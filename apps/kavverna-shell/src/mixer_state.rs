use sound_mixer::{MixerCommand, MixerCommands, MixerSnapshot};
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
    let cycle: Vec<String> = snapshot.outputs.iter().map(|device| device.name.clone()).collect();
    let next = snapshot.next_in_cycle(&cycle)?.clone();
    send(MixerCommand::MakeDefaultOutput(next.clone()));
    Some(next)
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

    while let Ok(snapshot) = changes.recv() {
        *lock() = Some(snapshot);
        on_change();
    }

    tracing::info!("mixer session ended");
}
