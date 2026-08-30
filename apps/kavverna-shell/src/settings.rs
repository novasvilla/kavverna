use feature_catalog::Feature;
use keep_awake::{Hold, Scope};
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
/// `node.name` of the input to come back to, which is what survives a restart. Empty for none.
pub const PREFERRED_INPUT: &str = "microphone-tools.preferred-input";
/// The outputs the switcher moves between. Absent means every output, which is what somebody
/// who has never opened this expects.
pub const OUTPUT_CYCLE: &str = "output-switcher.cycle";
/// Where each application plays: "app key<TAB>device node.name<TAB>description", one entry per
/// routed application. Absent from the list means follow the default.
pub const PLAYBACK_ROUTES: &str = "volume-mixer.routes";
/// The same shape for where each application records from.
pub const RECORDING_ROUTES: &str = "microphone-tools.routes";
pub const APPEARANCE: &str = "appearance";
/// Which palette dresses the panel. Doubles as the themes feature's enable key: removing the
/// feature applies the torch and leaves the choice waiting here.
pub const THEME: &str = enable_key(Feature::Themes);
/// Where the panel opens: 0 beside the tray icon, 1 where it was left, 2 the bottom right
/// corner the panel has always used.
pub const PLACEMENT: &str = "placement";
/// The gap between the panel and whatever it hangs off, in logical pixels. Config file only.
pub const PLACEMENT_GAP: &str = "placement.gap";
/// The last tray click that carried real coordinates, so a shortcut or a script opens the
/// panel where the icon was last seen. "screen name<TAB>x<TAB>y".
pub const PLACEMENT_ANCHOR: &str = "placement.anchor";
/// Where the panel was left, one entry per screen, most recent last. Same shape as the anchor.
pub const PLACEMENT_REMEMBERED: &str = "placement.remembered";
pub const HOLD_UNTIL: &str = "keep-awake.hold-until";

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

/// 0 follows the desktop, 1 is the cavern, 2 is its mouth. Following is the default because
/// somebody who switched their desktop to light meant it.
pub const APPEARANCE_DEFAULT: i64 = 0;

/// The torch is the look Kavverna has always had, so an absent key changes nothing.
pub const THEME_DEFAULT: &str = "torch";

/// Beside the icon, because the panel belongs to the icon that opened it. Panel bars sit on
/// different edges on different machines, and a corner that suits one is across the desk on
/// another.
pub const PLACEMENT_DEFAULT: i64 = 0;
/// Twelve is the margin the panel kept before placement existed, so the corner mode is the
/// old behaviour exactly.
pub const PLACEMENT_GAP_DEFAULT: i64 = 12;

/// What was being held when Kavverna last had a say. Zero for nothing, negative for a hold with
/// no end, and otherwise the wall clock second the hold runs out at. Wall clock rather than the
/// monotonic one the hold itself uses, because this has to survive the machine being off, and
/// a deadline that passed while it was off is a hold that is over.
pub const HOLD_UNTIL_NOTHING: i64 = 0;
pub const HOLD_UNTIL_INDEFINITE: i64 = -1;

/// Sits above a feature's own switch. Removing one hides it everywhere and stops it starting
/// next time, and never touches its enable keys, so putting it back restores what it was set to
/// do rather than a fresh default.
pub fn is_installed(feature: Feature) -> bool {
    feature.is_built() && bool_at(&feature.availability_key(), feature.installed_by_default())
}

/// A shared service belongs to more than one feature, so it runs while any of them is here.
pub fn any_installed(features: &[Feature]) -> bool {
    features.iter().copied().any(is_installed)
}

pub fn set_installed(feature: Feature, installed: bool) {
    put_bool(&feature.availability_key(), installed);
}

/// The setting resolved into a hold. Read from here by the panel switch, the tray menu and
/// anything else that starts a hold without being told how long for, so the three cannot
/// disagree about what the default duration means.
pub fn default_hold() -> Hold {
    match integer_at(DEFAULT_MINUTES, DEFAULT_MINUTES_DEFAULT) {
        0 => Hold::Indefinite,
        minutes => Hold::For(std::time::Duration::from_secs(minutes.unsigned_abs() * 60)),
    }
}

/// The setting resolved into how much a hold covers. Everything that starts one reads it from
/// here rather than from a copy of its own: the panel used to answer from a property it held,
/// which is a second source of the same truth waiting to disagree.
pub fn scope() -> Scope {
    if bool_at(ALLOW_DISPLAY_SLEEP, ALLOW_DISPLAY_SLEEP_DEFAULT) {
        Scope::SystemOnly
    } else {
        Scope::SystemAndDisplay
    }
}

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

pub fn text_at(key: &str, fallback: &str) -> String {
    store().as_ref().map_or_else(|| fallback.to_owned(), |prefs| prefs.text(key, fallback))
}

pub fn put_text(key: &str, value: &str) {
    let mut guard = store();
    if let Some(prefs) = guard.as_mut() {
        prefs.set_text(key, value);
        persist(prefs);
    }
}

pub fn texts_at(key: &str) -> Option<Vec<String>> {
    store().as_ref().and_then(|prefs| prefs.texts(key))
}

pub fn put_texts(key: &str, values: &[String]) {
    let mut guard = store();
    if let Some(prefs) = guard.as_mut() {
        prefs.set_texts(key, values);
        persist(prefs);
    }
}

fn persist(prefs: &Preferences) {
    if let Err(err) = prefs.save() {
        tracing::error!(%err, path = %prefs.path().display(), "settings not saved");
    }
}
