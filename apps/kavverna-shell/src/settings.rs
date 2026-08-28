use feature_catalog::Feature;
use preferences::Preferences;
use std::sync::{Mutex, MutexGuard};

/// A feature's switch is named once, in the catalogue, so the key that decides whether it runs
/// and the key the settings page writes cannot drift apart.
const fn enable_key(feature: Feature) -> &'static str {
    feature.describe().enable_keys[0]
}

pub const ALLOW_DISPLAY_SLEEP: &str = "keep-awake.allow-display-sleep";
pub const RESTORE_ON_START: &str = "keep-awake.restore-on-start";
pub const MOUSE_JIGGLE: &str = enable_key(Feature::MouseJiggle);
pub const JIGGLE_SHORTEST: &str = "mouse-jiggle.shortest-minutes";
pub const JIGGLE_LONGEST: &str = "mouse-jiggle.longest-minutes";
pub const JIGGLE_ACTIVITY: &str = "mouse-jiggle.activity";
pub const JIGGLE_KEYSTROKE: &str = "mouse-jiggle.keystroke";
pub const DEFAULT_MINUTES: &str = "keep-awake.default-minutes";
pub const MIDDLE_CLICK_TOGGLE: &str = "keep-awake.middle-click-toggle";
pub const CLIPBOARD_ENABLED: &str = enable_key(Feature::ClipboardHistory);
pub const CLIPBOARD_LIMIT: &str = "clipboard-history.limit";
pub const CLIPBOARD_IMAGES_AND_FILES: &str = "clipboard-history.images-and-files";
pub const CLIPBOARD_SKIP_SENSITIVE: &str = "clipboard-history.skip-sensitive";
pub const CLEAR_AFTER_SECONDS: &str = "clipboard-auto-clear.after-seconds";
pub const CLEAR_ON_SUSPEND: &str = "clipboard-auto-clear.on-suspend";
pub const CLEAR_ON_SCREEN_LOCK: &str = "clipboard-auto-clear.on-screen-lock";
pub const CLEAN_LINKS: &str = enable_key(Feature::CleanUrl);

pub const ALLOW_DISPLAY_SLEEP_DEFAULT: bool = true;
pub const RESTORE_ON_START_DEFAULT: bool = false;
pub const MOUSE_JIGGLE_DEFAULT: bool = false;
pub const JIGGLE_SHORTEST_DEFAULT: i64 = 2;
pub const JIGGLE_LONGEST_DEFAULT: i64 = 7;
pub const JIGGLE_ACTIVITY_DEFAULT: i64 = 0;
pub const JIGGLE_KEYSTROKE_DEFAULT: i64 = 0;
/// Zero means indefinite, matching the duration picker's first entry.
pub const DEFAULT_MINUTES_DEFAULT: i64 = 0;
pub const MIDDLE_CLICK_TOGGLE_DEFAULT: bool = false;

/// Off until asked for. A history of everything copied is not something to start keeping on
/// somebody's behalf.
pub const CLIPBOARD_ENABLED_DEFAULT: bool = false;
pub const CLIPBOARD_LIMIT_DEFAULT: i64 = 50;
pub const CLIPBOARD_IMAGES_AND_FILES_DEFAULT: bool = true;
pub const CLIPBOARD_SKIP_SENSITIVE_DEFAULT: bool = true;

/// Zero means never. One setting rather than a switch and a number, because a delay of nothing
/// and a delay switched off are the same thing to whoever is reading the page.
pub const CLEAR_AFTER_SECONDS_DEFAULT: i64 = 0;
pub const CLEAR_ON_SUSPEND_DEFAULT: bool = false;
pub const CLEAR_ON_SCREEN_LOCK_DEFAULT: bool = false;

/// Off until asked for: it rewrites what somebody copied, and doing that unasked is rude.
pub const CLEAN_LINKS_DEFAULT: bool = false;

static STORE: Mutex<Option<Preferences>> = Mutex::new(None);

fn store() -> MutexGuard<'static, Option<Preferences>> {
    let mut guard = STORE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.is_none() {
        *guard = Some(Preferences::load());
    }
    guard
}

pub fn bool_at(key: &str, fallback: bool) -> bool {
    store().as_ref().map_or(fallback, |prefs| prefs.bool(key, fallback))
}

pub fn integer_at(key: &str, fallback: i64) -> i64 {
    store().as_ref().map_or(fallback, |prefs| prefs.integer(key, fallback))
}

pub fn put_bool(key: &str, value: bool) {
    let mut guard = store();
    if let Some(prefs) = guard.as_mut() {
        prefs.set_bool(key, value);
        persist(prefs);
    }
}

pub fn put_integer(key: &str, value: i64) {
    let mut guard = store();
    if let Some(prefs) = guard.as_mut() {
        prefs.set_integer(key, value);
        persist(prefs);
    }
}

fn persist(prefs: &Preferences) {
    if let Err(err) = prefs.save() {
        tracing::error!(%err, path = %prefs.path().display(), "settings not saved");
    }
}
