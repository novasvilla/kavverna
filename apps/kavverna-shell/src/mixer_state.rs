use crate::{routes, settings};
use sound_mixer::{DeviceRole, MixerCommand, MixerCommands, MixerSnapshot, StreamTarget};
use std::sync::{Mutex, MutexGuard, OnceLock};

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
    for device in get().inputs {
        send(MixerCommand::SetMute { node_id: device.node_id, muted });
    }
}

/// True only when every input is muted, since a half muted set is not muted.
pub fn every_input_muted() -> bool {
    let inputs = get().inputs;
    !inputs.is_empty() && inputs.iter().all(|device| device.muted)
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

    while let Ok(snapshot) = changes.recv() {
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

        on_change();
    }

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
