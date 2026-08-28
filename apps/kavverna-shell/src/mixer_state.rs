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

/// Publishes every snapshot the session produces until it stops.
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
