use crate::command::Command;
use crate::tray::{StatusIcon, TrayIcon};
use crate::{awake_state, jiggle_state, panel, settings};
use keep_awake::{Activity, Hold, KeepAwake, Keystroke, MouseJiggle, Scope, Trigger};
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
        let hold = default_hold();
        if let Err(err) = runtime.block_on(keep_awake.engage(hold, scope(), Trigger::Manual)) {
            tracing::error!(%err, "could not restore keep awake on start");
        } else {
            tracing::info!(?hold, "keep awake restored on start");
        }
    }

    loop {
        match commands.recv_timeout(TICK) {
            Ok(Command::Engage(hold, requested)) => {
                if let Err(err) =
                    runtime.block_on(keep_awake.engage(hold, requested, Trigger::Manual))
                {
                    tracing::error!(%err, "could not engage keep awake");
                }
            }
            Ok(Command::Extend(extra)) => {
                if !keep_awake.extend(extra) {
                    tracing::info!("nothing to extend: no timed hold running");
                }
            }
            Ok(Command::Release) => runtime.block_on(keep_awake.release()),
            Ok(Command::NudgeNow) => {
                jiggle.set_screen(jiggle_state::screen());
                let (activity, keystroke) = jiggle_activity();
                jiggle.set_activity(activity, keystroke);
                jiggle.nudge_now();
            }
            Err(RecvTimeoutError::Timeout) => {
                if runtime.block_on(keep_awake.expire_if_due()) {
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

fn scope() -> Scope {
    if settings::bool_at(settings::ALLOW_DISPLAY_SLEEP, settings::ALLOW_DISPLAY_SLEEP_DEFAULT) {
        Scope::SystemOnly
    } else {
        Scope::SystemAndDisplay
    }
}

fn default_hold() -> Hold {
    match settings::integer_at(settings::DEFAULT_MINUTES, settings::DEFAULT_MINUTES_DEFAULT) {
        0 => Hold::Indefinite,
        minutes => Hold::For(Duration::from_secs(minutes.unsigned_abs() * 60)),
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
