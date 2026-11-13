use crate::{routes, settings, tray};
use sound_mixer::{DeviceRole, MixerCommand, MixerCommands, MixerSnapshot, StreamTarget};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// How long a burst of changes may gather before the interface hears of it. A volume drag
/// sends a snapshot per step, and each one rebuilds every list in the sound page; the drag
/// still feels live at this delay.
const SETTLE: Duration = Duration::from_millis(40);

static SNAPSHOT: Mutex<Option<MixerSnapshot>> = Mutex::new(None);
static COMMANDS: OnceLock<MixerCommands> = OnceLock::new();

fn lock() -> MutexGuard<'static, Option<MixerSnapshot>> {
    SNAPSHOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn get() -> MixerSnapshot {
    lock().clone().unwrap_or_default()
}

pub fn is_running() -> bool {
    COMMANDS.get().is_some()
}

pub fn send(command: MixerCommand) {
    match COMMANDS.get() {
        Some(commands) => commands.send(command),
        None => tracing::warn!("no mixer session, change dropped"),
    }
}

/// Moves to the next output in the cycle and reports what it landed on, so a shortcut, the tray
/// and the bus all switch outputs the same way.
pub fn cycle_output() -> Option<String> {
    let snapshot = get();
    let present: Vec<String> = snapshot.outputs.iter().map(|device| device.name.clone()).collect();
    let cycle = chosen_cycle(settings::texts_at(settings::OUTPUT_CYCLE).as_deref(), &present);
    let next = snapshot.next_in_cycle(&cycle)?.clone();
    send(MixerCommand::MakeDefaultOutput(next.clone()));
    Some(next)
}

/// What the switcher moves between: the chosen outputs that are actually plugged in, or every
/// output when nobody has chosen. Kept in the settings order rather than the device order, since
/// that is the order the person ticking them saw.
fn chosen_cycle(chosen: Option<&[String]>, present: &[String]) -> Vec<String> {
    match chosen {
        None => present.to_vec(),
        Some(chosen) => chosen.iter().filter(|name| present.contains(name)).cloned().collect(),
    }
}

pub fn mute_every_input(muted: bool) {
    let inputs: Vec<u32> = lock()
        .as_ref()
        .map(|snapshot| snapshot.inputs.iter().map(|device| device.node_id).collect())
        .unwrap_or_default();
    for node_id in inputs {
        send(MixerCommand::SetMute { node_id, muted });
    }
}

/// Answered under the lock rather than from a copy, since the tray asks every time it builds
/// its menu and a copy is the whole snapshot.
pub fn every_input_muted() -> bool {
    lock().as_ref().is_some_and(|snapshot| all_muted(snapshot.inputs.iter().map(|d| d.muted)))
}

/// True only when every input is muted, since a half muted set is not muted.
fn all_muted(inputs: impl IntoIterator<Item = bool>) -> bool {
    let mut any = false;
    for muted in inputs {
        if !muted {
            return false;
        }
        any = true;
    }
    any
}

/// Hands every change to `each` as it arrives and calls `settled` once a burst has gathered for
/// at most `window` from its first change. The deadline is not pushed back by later changes, so
/// a change that never stops still reaches the interface every `window`.
fn settle_in_bursts<T>(
    changes: &Receiver<T>,
    window: Duration,
    mut each: impl FnMut(T),
    mut settled: impl FnMut(),
) {
    let mut due: Option<Instant> = None;
    loop {
        // Checked before receiving, since a receive with a queued change returns it whatever
        // the timeout, and a steady stream would otherwise never settle.
        if due.is_some_and(|at| Instant::now() >= at) {
            due = None;
            settled();
            continue;
        }
        let next = match due {
            None => changes.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(at) => changes.recv_timeout(at.saturating_duration_since(Instant::now())),
        };
        match next {
            Ok(change) => {
                each(change);
                due.get_or_insert_with(|| Instant::now() + window);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if due.is_some() {
                    settled();
                }
                return;
            }
        }
    }
}

pub fn run(on_change: impl Fn()) {
    let (commands, changes) = match sound_mixer::start() {
        Ok(parts) => parts,
        Err(err) => {
            tracing::error!(%err, "sound mixer unavailable");
            return;
        }
    };

    tracing::info!("sound mixer connected");

    if COMMANDS.set(commands).is_err() {
        tracing::error!("a mixer session was already running");
        return;
    }

    let mut inputs_before: Vec<String> = Vec::new();
    let mut outputs_before: Vec<String> = Vec::new();
    let mut stream_ids_before: Vec<u32> = Vec::new();

    // The tray caches its menu between updates, and the microphone item there says which way
    // it will switch, so the tray is redrawn whenever that answer flips and once at the start.
    let mut tray_says_muted: Option<bool> = None;
    let settled = || {
        on_change();
        let muted = every_input_muted();
        // Remembered only once a tray took it: the host can take half a minute to answer.
        if tray_says_muted != Some(muted) && tray::refresh() {
            tray_says_muted = Some(muted);
        }
    };

    // Routes and the preferred input are worked out on every snapshot, since they compare one
    // with the last; only telling the interface is gathered into bursts.
    let each = |snapshot: MixerSnapshot| {
        let inputs_now: Vec<String> =
            snapshot.inputs.iter().map(|device| device.name.clone()).collect();
        let outputs_now: Vec<String> =
            snapshot.outputs.iter().map(|device| device.name.clone()).collect();
        let stream_ids_now: Vec<u32> =
            snapshot.streams.iter().map(|stream| stream.node_id).collect();
        let already_default =
            snapshot.inputs.iter().any(|device| device.is_default && device.name == preferred());

        // The half WirePlumber's own store does not do: pin a stream back when its chosen
        // device returns, and pin a newcomer whose identity only our key recognises.
        let due = {
            let side = |role, devices_before: &[String], devices_now: &[String], key| {
                let at: Vec<routes::StreamAt> = snapshot
                    .streams
                    .iter()
                    .filter(|s| s.role == role && !s.taps_playback)
                    .map(|s| routes::StreamAt {
                        node_id: s.node_id,
                        app: s.key.as_str().to_owned(),
                        movable: s.anchored.is_none(),
                    })
                    .collect();
                let entries = settings::texts_at(key).unwrap_or_default();
                routes::moves_due(&entries, &at, &stream_ids_before, devices_before, devices_now)
            };
            let mut due =
                side(DeviceRole::Output, &outputs_before, &outputs_now, settings::PLAYBACK_ROUTES);
            due.extend(side(
                DeviceRole::Input,
                &inputs_before,
                &inputs_now,
                settings::RECORDING_ROUTES,
            ));
            due
        };

        *lock() = Some(snapshot);

        for (node_id, device) in due {
            tracing::info!(node_id, device = %device, "re-applying a stored route");
            send(MixerCommand::MoveStream { node_id, to: StreamTarget::Device(device) });
        }

        if !already_default && has_just_arrived(&preferred(), &inputs_before, &inputs_now) {
            tracing::info!(input = %preferred(), "the preferred input is back, making it default");
            send(MixerCommand::MakeDefaultInput(preferred()));
        }
        inputs_before = inputs_now;
        outputs_before = outputs_now;
        stream_ids_before = stream_ids_now;
    };

    settle_in_bursts(&changes, SETTLE, each, settled);

    tracing::info!("mixer session ended");
}

fn preferred() -> String {
    settings::text_at(settings::PREFERRED_INPUT, "")
}

/// A pin is applied when the device turns up, not on every snapshot. Reasserting it constantly
/// would make choosing anything else impossible for as long as the headset stayed plugged in,
/// which is a pin nobody asked for.
fn has_just_arrived(preferred: &str, before: &[String], now: &[String]) -> bool {
    !preferred.is_empty()
        && now.iter().any(|name| name == preferred)
        && !before.iter().any(|name| name == preferred)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADSET: &str = "alsa_input.usb-headset";

    #[test]
    fn a_burst_reaches_the_interface_once_with_every_change_handled() {
        let (sender, changes) = std::sync::mpsc::channel();
        for step in 0..5 {
            sender.send(step).unwrap();
        }
        drop(sender);

        let mut handled = Vec::new();
        let mut settled = 0;
        settle_in_bursts(
            &changes,
            Duration::from_secs(5),
            |step| handled.push(step),
            || settled += 1,
        );

        assert_eq!(handled, vec![0, 1, 2, 3, 4]);
        assert_eq!(settled, 1);
    }

    #[test]
    fn a_change_that_never_stops_still_settles_within_the_window() {
        let (sender, changes) = std::sync::mpsc::channel();
        let feeding = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_millis(300);
            while Instant::now() < until {
                sender.send(()).unwrap();
                std::thread::sleep(Duration::from_millis(2));
            }
        });

        let mut settled = 0;
        settle_in_bursts(&changes, Duration::from_millis(20), |()| {}, || settled += 1);
        feeding.join().unwrap();

        assert!(settled >= 3, "a steady stream settled only {settled} times");
    }

    #[test]
    fn a_half_muted_set_of_inputs_is_not_muted() {
        assert!(all_muted([true, true]));
        assert!(!all_muted([true, false]));
        assert!(!all_muted([]), "no inputs is not a muted set");
    }

    #[test]
    fn plugging_the_preferred_input_back_in_claims_it() {
        assert!(has_just_arrived(HEADSET, &[], &[HEADSET.into()]));
    }

    #[test]
    fn a_preferred_input_that_never_left_is_left_alone() {
        assert!(!has_just_arrived(HEADSET, &[HEADSET.into()], &[HEADSET.into()]));
    }

    #[test]
    fn nothing_pinned_claims_nothing() {
        assert!(!has_just_arrived("", &[], &["anything".into()]));
    }

    #[test]
    fn an_unchosen_cycle_is_every_output() {
        let present = vec!["analog".to_owned(), "hdmi".to_owned()];

        assert_eq!(chosen_cycle(None, &present), present);
    }

    /// Unplugging one of the chosen outputs must not strand the cycle on a name that is gone.
    #[test]
    fn a_chosen_output_that_is_not_plugged_in_is_skipped() {
        let chosen = vec!["analog".to_owned(), "hdmi".to_owned()];
        let present = vec!["analog".to_owned()];

        assert_eq!(chosen_cycle(Some(&chosen), &present), vec!["analog".to_owned()]);
    }

    #[test]
    fn choosing_nothing_leaves_nothing_to_cycle_through() {
        let present = vec!["analog".to_owned()];

        assert!(chosen_cycle(Some(&[]), &present).is_empty());
    }
}
