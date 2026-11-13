use crate::settings;
use clipboard_history::{Command, Commands, History, Settings, Snapshot};
use feature_catalog::Feature;
use preferences::Preferences;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

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

pub fn send(command: Command) -> bool {
    match COMMANDS.lock().ok().and_then(|held| held.clone()) {
        Some(commands) => commands.send(command),
        None => {
            tracing::warn!("the clipboard history is not running, change dropped");
            false
        }
    }
}

// These read the switches as stored, which is what the clipboard page shows and writes back.
// Whether anything runs is decided by `wanted_in` and `settings_in`, which put each feature's
// availability above its switch.

pub fn keeps_history() -> bool {
    settings::bool_at(settings::CLIPBOARD_ENABLED, settings::CLIPBOARD_ENABLED_DEFAULT)
}

pub fn cleans_links() -> bool {
    settings::bool_at(settings::CLEAN_LINKS, settings::CLEAN_LINKS_DEFAULT)
}

pub fn clean_rules() -> link_cleaner::Rules {
    settings::read(rules_in)
}

pub fn clears_on_suspend() -> bool {
    settings::bool_at(settings::CLEAR_ON_SUSPEND, settings::CLEAR_ON_SUSPEND_DEFAULT)
}

pub fn clears_on_screen_lock() -> bool {
    settings::bool_at(settings::CLEAR_ON_SCREEN_LOCK, settings::CLEAR_ON_SCREEN_LOCK_DEFAULT)
}

fn rules_in(prefs: &Preferences) -> link_cleaner::Rules {
    link_cleaner::Rules::from_entries(
        &prefs.texts(settings::CLEAN_URL_ADDED_RULES).unwrap_or_default(),
        &prefs.texts(settings::CLEAN_URL_DISABLED_RULES).unwrap_or_default(),
    )
}

/// Each switch counts only while the feature it belongs to is installed. Removing a feature
/// leaves its switch untouched so that putting it back restores it, which means a stale `true`
/// is exactly what a removed feature leaves behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Switches {
    keep_history: bool,
    clean_links: bool,
    clear_on_suspend: bool,
    clear_on_screen_lock: bool,
    clear_after: Option<Duration>,
}

fn switches_in(prefs: &Preferences) -> Switches {
    let history = settings::is_installed_in(prefs, Feature::ClipboardHistory);
    let links = settings::is_installed_in(prefs, Feature::CleanUrl);
    let clearing = settings::is_installed_in(prefs, Feature::ClipboardAutoClear);
    let seconds =
        prefs.integer(settings::CLEAR_AFTER_SECONDS, settings::CLEAR_AFTER_SECONDS_DEFAULT);

    Switches {
        keep_history: history
            && prefs.bool(settings::CLIPBOARD_ENABLED, settings::CLIPBOARD_ENABLED_DEFAULT),
        clean_links: links && prefs.bool(settings::CLEAN_LINKS, settings::CLEAN_LINKS_DEFAULT),
        clear_on_suspend: clearing
            && prefs.bool(settings::CLEAR_ON_SUSPEND, settings::CLEAR_ON_SUSPEND_DEFAULT),
        clear_on_screen_lock: clearing
            && prefs.bool(settings::CLEAR_ON_SCREEN_LOCK, settings::CLEAR_ON_SCREEN_LOCK_DEFAULT),
        clear_after: (clearing && seconds > 0).then(|| Duration::from_secs(seconds.unsigned_abs())),
    }
}

/// Auto clear needs the connection as much as the history does, and it has to keep working with
/// the history switched off. With it off nothing is read: the compositor still reports that a
/// copy happened, which is all the timer needs.
///
/// Transform has no switch of its own, its buttons being the switch, so being installed is
/// enough. It reads the selection only when a button asks, and keeps nothing.
fn wanted_in(prefs: &Preferences) -> bool {
    let on = switches_in(prefs);
    on.keep_history
        || on.clean_links
        || on.clear_on_suspend
        || on.clear_on_screen_lock
        || on.clear_after.is_some()
        || settings::is_installed_in(prefs, Feature::ClipboardTransform)
}

/// Starts and stops with the setting, so switching it off really does unbind the device.
pub fn run(on_change: impl Fn()) {
    let Some(root) = data_root() else {
        tracing::error!("no data directory, the clipboard history cannot be kept");
        return;
    };

    let mut running: Option<(History, Receiver<Snapshot>)> = None;
    let mut applied = Settings::default();
    // Building the link rules is the expensive part of reading the settings, so everything is
    // read again only when a write has moved them since the last look.
    let mut seen: Option<u64> = None;
    let mut wants = false;

    loop {
        let generation = settings::generation();
        let moved = seen != Some(generation);
        if moved {
            seen = Some(generation);
            wants = settings::read(wanted_in);
        }

        match (wants, running.is_some()) {
            (true, false) => {
                applied = settings::read(settings_in);
                match History::start(&root, applied.clone()) {
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

        if moved {
            let current = settings::read(settings_in);
            if current != applied {
                history.send(Command::Apply(current.clone()));
                applied = current;
            }
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

fn settings_in(prefs: &Preferences) -> Settings {
    let on = switches_in(prefs);
    Settings {
        keep_history: on.keep_history,
        clean_links: on.clean_links,
        rules: rules_in(prefs),
        clear_after: on.clear_after,
        limit: u32::try_from(
            prefs.integer(settings::CLIPBOARD_LIMIT, settings::CLIPBOARD_LIMIT_DEFAULT),
        )
        .unwrap_or(0),
        keep_days: u32::try_from(
            prefs.integer(settings::CLIPBOARD_KEEP_DAYS, settings::CLIPBOARD_KEEP_DAYS_DEFAULT),
        )
        .ok()
        .filter(|days| *days > 0),
        skip_sensitive: prefs
            .bool(settings::CLIPBOARD_SKIP_SENSITIVE, settings::CLIPBOARD_SKIP_SENSITIVE_DEFAULT),
        images_and_files: prefs.bool(
            settings::CLIPBOARD_IMAGES_AND_FILES,
            settings::CLIPBOARD_IMAGES_AND_FILES_DEFAULT,
        ),
    }
}

/// Content, not settings: a settings backup has no business carrying what was copied.
fn data_root() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("dev", "", "kavverna").map(|dirs| dirs.data_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Never saved: the path sits under a directory that does not exist.
    fn blank() -> Preferences {
        Preferences::load_from("/nonexistent/kavverna-test/settings.json")
    }

    fn removed(prefs: &mut Preferences, feature: Feature) {
        prefs.set_bool(&feature.availability_key(), false);
    }

    /// Transform arrives installed and needs no switch, so a fresh install holds the
    /// connection for it. The cases about the other features take it away first.
    fn without_transform() -> Preferences {
        let mut prefs = blank();
        removed(&mut prefs, Feature::ClipboardTransform);
        prefs
    }

    #[test]
    fn nothing_switched_on_wants_nothing() {
        assert!(!wanted_in(&without_transform()));
    }

    /// The case that was broken: with Transform alone nothing opened the connection, and its
    /// buttons were dropped.
    #[test]
    fn transform_alone_opens_the_connection_and_records_nothing() {
        let mut prefs = blank();
        for feature in [Feature::ClipboardHistory, Feature::CleanUrl, Feature::ClipboardAutoClear] {
            removed(&mut prefs, feature);
        }
        // Switches left on from before the removals, which must count for nothing.
        prefs.set_bool(settings::CLIPBOARD_ENABLED, true);
        prefs.set_bool(settings::CLEAN_LINKS, true);
        prefs.set_integer(settings::CLEAR_AFTER_SECONDS, 30);

        assert!(wanted_in(&prefs), "Transform needs the connection");
        let applied = settings_in(&prefs);
        assert!(!applied.keep_history, "nothing may be recorded");
        assert!(!applied.clean_links);
        assert_eq!(applied.clear_after, None);

        removed(&mut prefs, Feature::ClipboardTransform);
        assert!(!wanted_in(&prefs), "with Transform gone too, nothing is left to hold it open");
    }

    #[test]
    fn a_removed_history_stops_recording_while_clean_url_stays() {
        let mut prefs = without_transform();
        prefs.set_bool(settings::CLIPBOARD_ENABLED, true);
        assert!(wanted_in(&prefs));

        removed(&mut prefs, Feature::ClipboardHistory);

        assert!(!wanted_in(&prefs), "a stale switch must not keep the connection open");
        assert!(!settings_in(&prefs).keep_history, "nothing may be recorded");
    }

    /// The case that was broken: Clean URL keeps the connection open, and a stale history
    /// switch then went on recording through it.
    #[test]
    fn a_removed_history_records_nothing_while_clean_url_keeps_cleaning() {
        let mut prefs = blank();
        prefs.set_bool(settings::CLIPBOARD_ENABLED, true);
        prefs.set_bool(settings::CLEAN_LINKS, true);

        removed(&mut prefs, Feature::ClipboardHistory);

        assert!(wanted_in(&prefs), "Clean URL still needs the connection");
        let applied = settings_in(&prefs);
        assert!(!applied.keep_history, "nothing may be recorded");
        assert!(applied.clean_links);
    }

    #[test]
    fn a_removed_clean_url_stops_cleaning_while_the_history_keeps_recording() {
        let mut prefs = blank();
        prefs.set_bool(settings::CLIPBOARD_ENABLED, true);
        prefs.set_bool(settings::CLEAN_LINKS, true);

        removed(&mut prefs, Feature::CleanUrl);

        let applied = settings_in(&prefs);
        assert!(applied.keep_history);
        assert!(!applied.clean_links);
    }

    #[test]
    fn a_removed_auto_clear_no_longer_holds_the_connection_open() {
        let mut prefs = without_transform();
        prefs.set_integer(settings::CLEAR_AFTER_SECONDS, 30);
        prefs.set_bool(settings::CLEAR_ON_SUSPEND, true);
        prefs.set_bool(settings::CLEAR_ON_SCREEN_LOCK, true);
        assert!(wanted_in(&prefs));

        removed(&mut prefs, Feature::ClipboardAutoClear);

        assert!(!wanted_in(&prefs));
        assert_eq!(settings_in(&prefs).clear_after, None);
    }

    /// Removing a feature never writes its switch, so putting it back restores what it was.
    #[test]
    fn putting_a_feature_back_restores_its_switch() {
        let mut prefs = blank();
        prefs.set_bool(settings::CLIPBOARD_ENABLED, true);
        removed(&mut prefs, Feature::ClipboardHistory);

        prefs.set_bool(&Feature::ClipboardHistory.availability_key(), true);

        assert!(settings_in(&prefs).keep_history);
    }
}
