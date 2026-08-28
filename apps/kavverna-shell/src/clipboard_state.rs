use crate::settings;
use clipboard_history::{Command, Commands, History, Settings, Snapshot};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// Read by the panel and written by the history thread, neither of which shares an event loop
/// with the other.
static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);
static COMMANDS: Mutex<Option<Commands>> = Mutex::new(None);

const POLL: Duration = Duration::from_secs(1);

fn lock() -> MutexGuard<'static, Option<Snapshot>> {
    SNAPSHOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn get() -> Snapshot {
    lock().clone().unwrap_or_default()
}

pub fn is_running() -> bool {
    COMMANDS.lock().map(|held| held.is_some()).unwrap_or(false)
}

pub fn send(command: Command) {
    match COMMANDS.lock().ok().and_then(|held| held.clone()) {
        Some(commands) => commands.send(command),
        None => tracing::warn!("the clipboard history is not running, change dropped"),
    }
}

pub fn keeps_history() -> bool {
    settings::bool_at(settings::CLIPBOARD_ENABLED, settings::CLIPBOARD_ENABLED_DEFAULT)
}

pub fn clears_on_suspend() -> bool {
    settings::bool_at(settings::CLEAR_ON_SUSPEND, settings::CLEAR_ON_SUSPEND_DEFAULT)
}

pub fn clears_on_screen_lock() -> bool {
    settings::bool_at(settings::CLEAR_ON_SCREEN_LOCK, settings::CLEAR_ON_SCREEN_LOCK_DEFAULT)
}

/// Auto clear needs the connection as much as the history does, and it has to keep working with
/// the history switched off. With it off nothing is read: the compositor still reports that a
/// copy happened, which is all the timer needs.
pub fn wanted() -> bool {
    keeps_history()
        || clears_on_suspend()
        || clears_on_screen_lock()
        || clear_after().is_some()
}

fn clear_after() -> Option<Duration> {
    let seconds = settings::integer_at(
        settings::CLEAR_AFTER_SECONDS,
        settings::CLEAR_AFTER_SECONDS_DEFAULT,
    );
    (seconds > 0).then(|| Duration::from_secs(seconds.unsigned_abs()))
}

/// Starts and stops with the setting, so switching it off really does unbind the device.
pub fn run(on_change: impl Fn()) {
    let Some(root) = data_root() else {
        tracing::error!("no data directory, the clipboard history cannot be kept");
        return;
    };

    let mut running: Option<(History, Receiver<Snapshot>)> = None;
    let mut applied = read_settings();

    loop {
        match (wanted(), running.is_some()) {
            (true, false) => {
                applied = read_settings();
                match History::start(&root, applied) {
                    Ok((history, snapshots)) => {
                        set_commands(Some(history.commands()));
                        running = Some((history, snapshots));
                        tracing::info!(?applied, "the clipboard history is running");
                    }
                    Err(err) => {
                        tracing::error!(%err, "the clipboard history could not start");
                        std::thread::sleep(POLL * 5);
                    }
                }
            }
            (false, true) => {
                set_commands(None);
                running = None;
                *lock() = None;
                on_change();
                tracing::info!("the clipboard history stopped watching");
            }
            _ => {}
        }

        let Some((history, snapshots)) = running.as_ref() else {
            std::thread::sleep(POLL);
            continue;
        };

        let current = read_settings();
        if current != applied {
            applied = current;
            history.send(Command::Apply(current));
        }

        match snapshots.recv_timeout(POLL) {
            Ok(snapshot) => {
                *lock() = Some(snapshot);
                on_change();
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::error!("the clipboard thread stopped on its own");
                set_commands(None);
                running = None;
                *lock() = None;
                on_change();
            }
        }
    }
}

fn set_commands(commands: Option<Commands>) {
    if let Ok(mut held) = COMMANDS.lock() {
        *held = commands;
    }
}

fn read_settings() -> Settings {
    Settings {
        keep_history: keeps_history(),
        clear_after: clear_after(),
        limit: u32::try_from(settings::integer_at(
            settings::CLIPBOARD_LIMIT,
            settings::CLIPBOARD_LIMIT_DEFAULT,
        ))
        .unwrap_or(0),
        skip_sensitive: settings::bool_at(
            settings::CLIPBOARD_SKIP_SENSITIVE,
            settings::CLIPBOARD_SKIP_SENSITIVE_DEFAULT,
        ),
        images_and_files: settings::bool_at(
            settings::CLIPBOARD_IMAGES_AND_FILES,
            settings::CLIPBOARD_IMAGES_AND_FILES_DEFAULT,
        ),
    }
}

/// Content, not settings: a settings backup has no business carrying what was copied.
fn data_root() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("dev", "", "kavverna")
        .map(|dirs| dirs.data_dir().to_path_buf())
}
