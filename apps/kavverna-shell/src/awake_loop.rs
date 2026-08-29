use crate::command::Command;
use crate::tray::{StatusIcon, TrayIcon};
use crate::{awake_state, jiggle_state, panel, settings};
use keep_awake::{Activity, Hold, KeepAwake, Keystroke, MouseJiggle, Scope};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

const TICK: Duration = Duration::from_secs(1);

/// Runs on its own thread with a current-thread runtime, so neither Qt's event loop nor
/// ksni's has to accommodate it.
pub fn run(commands: Receiver<Command>, tray: TrayIcon) {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!(%err, "keep awake unavailable: no runtime");
            return;
        }
    };

    let mut keep_awake = match runtime.block_on(KeepAwake::connect()) {
        Ok(keep_awake) => keep_awake,
        Err(err) => {
            tracing::error!(%err, "keep awake unavailable: no session bus");
            return;
        }
    };

    let (shortest, longest) = jiggle_range();
    let mut jiggle = MouseJiggle::between(shortest, longest);

    if settings::bool_at(settings::RESTORE_ON_START, settings::RESTORE_ON_START_DEFAULT) {
        match remembered_hold(now_in_seconds()) {
            Some(hold) => {
                if let Err(err) = runtime.block_on(keep_awake.engage(hold, scope())) {
                    tracing::error!(%err, "could not restore keep awake on start");
                } else {
                    tracing::info!(?hold, "keep awake restored on start");
                }
            }
            None => tracing::info!("nothing was being held, so nothing to restore"),
        }
    }

    loop {
        match commands.recv_timeout(TICK) {
            Ok(Command::Engage(hold, requested)) => {
                if let Err(err) = runtime.block_on(keep_awake.engage(hold, requested)) {
                    tracing::error!(%err, "could not engage keep awake");
                }
                remember(&keep_awake);
            }
            Ok(Command::Extend(extra)) => {
                if keep_awake.extend(extra) {
                    remember(&keep_awake);
                } else {
                    tracing::info!("nothing to extend: no timed hold running");
                }
            }
            Ok(Command::Release) => {
                runtime.block_on(keep_awake.release());
                remember(&keep_awake);
            }
            Ok(Command::NudgeNow) => {
                jiggle.set_screen(jiggle_state::screen());
                let (activity, keystroke) = jiggle_activity();
                jiggle.set_activity(activity, keystroke);
                jiggle.nudge_now();
            }
            Err(RecvTimeoutError::Timeout) => {
                if runtime.block_on(keep_awake.expire_if_due()) {
                    remember(&keep_awake);
                    announce_expiry();
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                runtime.block_on(keep_awake.release());
                return;
            }
        }

        // A tool in its own right: it runs whenever it is switched on, whether or not sleep
        // is being held off.
        let jiggling = settings::bool_at(settings::MOUSE_JIGGLE, settings::MOUSE_JIGGLE_DEFAULT);
        if jiggling {
            let (shortest, longest) = jiggle_range();
            jiggle.set_range(shortest, longest);
            jiggle.set_screen(jiggle_state::screen());
            let (activity, keystroke) = jiggle_activity();
            jiggle.set_activity(activity, keystroke);
            jiggle.tick();
        } else {
            jiggle.rest();
        }

        jiggle_state::set(jiggle_state::JiggleState {
            running: jiggling,
            nudges: jiggle.nudges(),
            seconds_until_next: jiggle.until_next().map(|left| left.as_secs()),
            waiting_seconds: jiggle.next_interval().as_secs(),
        });

        let active = keep_awake.is_active();
        let remaining = keep_awake.remaining();

        awake_state::set_hold(active, remaining);
        panel::publish_awake(active, remaining);

        if let Some(tray) = tray.lock().ok().and_then(|held| held.clone()) {
            tray.update(move |icon: &mut StatusIcon| {
                icon.awake = active;
                icon.remaining = remaining;
            });
        }
    }
}

fn now_in_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or(0)
}

/// Written whenever the hold changes rather than on every tick, since each write is a file
/// rewritten and a timed hold would otherwise rewrite it once a second for hours.
fn remember(keep_awake: &KeepAwake) {
    let record = record_for(keep_awake.is_active(), keep_awake.remaining(), now_in_seconds());
    settings::put_integer(settings::HOLD_UNTIL, record);
}

fn record_for(active: bool, remaining: Option<Duration>, now: i64) -> i64 {
    match (active, remaining) {
        (false, _) => settings::HOLD_UNTIL_NOTHING,
        (true, None) => settings::HOLD_UNTIL_INDEFINITE,
        (true, Some(left)) => now + left.as_secs() as i64,
    }
}

fn remembered_hold(now: i64) -> Option<Hold> {
    hold_from_record(settings::integer_at(settings::HOLD_UNTIL, settings::HOLD_UNTIL_NOTHING), now)
}

/// What to put back, given what was remembered and what time it is now. A deadline that has
/// already passed is a hold that ended while Kavverna was closed, so nothing is put back: coming
/// home to a machine that stayed awake all night because of a thirty minute hold is worse than
/// losing the hold.
fn hold_from_record(record: i64, now: i64) -> Option<Hold> {
    match record {
        settings::HOLD_UNTIL_NOTHING => None,
        deadline if deadline < 0 => Some(Hold::Indefinite),
        deadline if deadline > now => Some(Hold::For(Duration::from_secs((deadline - now) as u64))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn a_timed_hold_comes_back_with_what_was_left_of_it() {
        let record = record_for(true, Some(Duration::from_secs(1800)), NOW);

        assert_eq!(hold_from_record(record, NOW + 120), Some(Hold::For(Duration::from_secs(1680))));
    }

    #[test]
    fn a_hold_with_no_end_comes_back_the_same_way() {
        let record = record_for(true, None, NOW);

        assert_eq!(hold_from_record(record, NOW + 90_000), Some(Hold::Indefinite));
    }

    #[test]
    fn nothing_held_puts_nothing_back() {
        assert_eq!(hold_from_record(record_for(false, None, NOW), NOW), None);
    }

    /// The case that matters most: a machine left off overnight must not come back holding a
    /// thirty minute hold that ended hours ago.
    #[test]
    fn a_hold_that_ran_out_while_it_was_closed_is_over() {
        let record = record_for(true, Some(Duration::from_secs(1800)), NOW);

        assert_eq!(hold_from_record(record, NOW + 7200), None);
    }
}

fn scope() -> Scope {
    if settings::bool_at(settings::ALLOW_DISPLAY_SLEEP, settings::ALLOW_DISPLAY_SLEEP_DEFAULT) {
        Scope::SystemOnly
    } else {
        Scope::SystemAndDisplay
    }
}

fn jiggle_activity() -> (Activity, Keystroke) {
    let read = |key, fallback| i32::try_from(settings::integer_at(key, fallback)).unwrap_or(0);

    (
        Activity::from_id(read(settings::JIGGLE_ACTIVITY, settings::JIGGLE_ACTIVITY_DEFAULT)),
        Keystroke::from_id(read(settings::JIGGLE_KEYSTROKE, settings::JIGGLE_KEYSTROKE_DEFAULT)),
    )
}

fn jiggle_range() -> (Duration, Duration) {
    let minutes = |key, fallback| {
        Duration::from_secs(settings::integer_at(key, fallback).max(1).unsigned_abs() * 60)
    };

    (
        minutes(settings::JIGGLE_SHORTEST, settings::JIGGLE_SHORTEST_DEFAULT),
        minutes(settings::JIGGLE_LONGEST, settings::JIGGLE_LONGEST_DEFAULT),
    )
}

fn announce_expiry() {
    let outcome = notify_rust::Notification::new()
        .summary("Keep awake ended")
        .body("Time is up. The machine will sleep normally again.")
        .icon("preferences-system-power-management")
        .appname("Kavverna")
        .show();

    if let Err(err) = outcome {
        tracing::warn!(%err, "could not post the expiry notification");
    }
}
