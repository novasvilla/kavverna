use feature_catalog::Feature;
use keep_awake::{Hold, Scope};
use preferences::Preferences;
use std::sync::atomic::{AtomicU64, Ordering};
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
pub const CLIPBOARD_KEEP_DAYS: &str = "clipboard-history.keep-days";
pub const CLIPBOARD_IMAGES_AND_FILES: &str = "clipboard-history.images-and-files";
pub const CLIPBOARD_SKIP_SENSITIVE: &str = "clipboard-history.skip-sensitive";
pub const CLEAR_AFTER_SECONDS: &str = "clipboard-auto-clear.after-seconds";
pub const CLEAR_ON_SUSPEND: &str = "clipboard-auto-clear.on-suspend";
pub const CLEAR_ON_SCREEN_LOCK: &str = "clipboard-auto-clear.on-screen-lock";
pub const CLEAN_LINKS: &str = enable_key(Feature::CleanUrl);
/// Custom URL rules and disabled rule identities use `scope<TAB>parameter`. `*` is global.
pub const CLEAN_URL_ADDED_RULES: &str = "clean-url.added-rules";
pub const CLEAN_URL_DISABLED_RULES: &str = "clean-url.disabled-rules";
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
/// The thin landing strip on the screen edge, the one thing the shelf does at rest.
pub const SHELF_EDGE_STRIP: &str = enable_key(Feature::Shelf);
pub const SHELF_KEEP_ACROSS_RESTARTS: &str = "shelf.keep-across-restarts";
pub const SHELF_REMOVE_AFTER_DROP: &str = "shelf.remove-after-drop";
/// The strip and the shelf live on the right edge unless moved to the left.
pub const SHELF_STRIP_LEFT: &str = "shelf.strip-left";
/// Where the shelf was left, one entry per screen, same shape as the panel's.
pub const SHELF_POSITION: &str = "shelf.position";
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
/// Whether the panel floats over every window, fullscreen included, the way it always has.
/// Off, it sits on the ordinary panel layer and fullscreen applications cover it.
pub const PANEL_ON_TOP: &str = "panel.on-top";
pub const HOLD_UNTIL: &str = "keep-awake.hold-until";

pub const PANEL_ON_TOP_DEFAULT: bool = true;
pub const ALLOW_DISPLAY_SLEEP_DEFAULT: bool = true;
pub const RESTORE_ON_START_DEFAULT: bool = false;
pub const MOUSE_JIGGLE_DEFAULT: bool = false;
pub const JIGGLE_SHORTEST_DEFAULT: i64 = 2;
// One of the values the panel's choices can show, so a fresh install marks its current
// setting instead of lighting nothing.
pub const JIGGLE_LONGEST_DEFAULT: i64 = 5;
pub const JIGGLE_ACTIVITY_DEFAULT: i64 = 0;
pub const JIGGLE_KEYSTROKE_DEFAULT: i64 = 0;
/// Zero means indefinite, matching the duration picker's first entry.
pub const DEFAULT_MINUTES_DEFAULT: i64 = 0;
pub const MIDDLE_CLICK_TOGGLE_DEFAULT: bool = false;

/// Off until asked for. A history of everything copied is not something to start keeping on
/// somebody's behalf.
pub const CLIPBOARD_ENABLED_DEFAULT: bool = false;
pub const CLIPBOARD_LIMIT_DEFAULT: i64 = 50;
/// Zero keeps entries until the count limit pushes them out, which is how every earlier
/// release behaved.
pub const CLIPBOARD_KEEP_DAYS_DEFAULT: i64 = 0;
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

pub const SHELF_EDGE_STRIP_DEFAULT: bool = true;
/// Parked items are deliberate; a shelf that forgot them at logout would defeat the parking.
pub const SHELF_KEEP_ACROSS_RESTARTS_DEFAULT: bool = true;
/// A drag something accepted took the item along, so it leaves; a cancelled drag keeps it.
pub const SHELF_REMOVE_AFTER_DROP_DEFAULT: bool = true;
pub const SHELF_STRIP_LEFT_DEFAULT: bool = false;

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
    read(|prefs| is_installed_in(prefs, feature))
}

pub fn is_installed_in(prefs: &Preferences, feature: Feature) -> bool {
    feature.is_built() && prefs.bool(&feature.availability_key(), feature.installed_by_default())
}

/// A shared service belongs to more than one feature, so it runs while any of them is here.
pub fn any_installed(features: &[Feature]) -> bool {
    features.iter().copied().any(is_installed)
}

pub fn set_installed(feature: Feature, installed: bool) -> bool {
    put_bool(&feature.availability_key(), installed)
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

/// Raised by every write that reached the disk. A thread that polls can then rebuild what it
/// derives from the settings only when they moved, rather than once a tick.
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Several reads under one lock, for a decision that has to see one consistent state. The
/// closure must not call back into this module, since the lock is held while it runs.
pub fn read<T>(look: impl FnOnce(&Preferences) -> T) -> T {
    look(&*store().get_or_insert_with(Preferences::load))
}

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

pub fn can_save() -> bool {
    store().as_ref().is_some_and(Preferences::can_save)
}

pub fn integer_at(key: &str, fallback: i64) -> i64 {
    store().as_ref().map_or(fallback, |prefs| prefs.integer(key, fallback))
}

pub fn put_bool(key: &str, value: bool) -> bool {
    update(|prefs| holds_bool(prefs, key, value), |prefs| prefs.set_bool(key, value))
}

pub fn put_integer(key: &str, value: i64) -> bool {
    update(|prefs| holds_integer(prefs, key, value), |prefs| prefs.set_integer(key, value))
}

pub fn text_at(key: &str, fallback: &str) -> String {
    store().as_ref().map_or_else(|| fallback.to_owned(), |prefs| prefs.text(key, fallback))
}

pub fn put_text(key: &str, value: &str) -> bool {
    update(|prefs| holds_text(prefs, key, value), |prefs| prefs.set_text(key, value))
}

pub fn texts_at(key: &str) -> Option<Vec<String>> {
    store().as_ref().and_then(|prefs| prefs.texts(key))
}

pub fn put_texts(key: &str, values: &[String]) -> bool {
    update(|prefs| holds_texts(prefs, key, values), |prefs| prefs.set_texts(key, values))
}

// Each probe asks under two different fallbacks, since an absent key answers with whichever
// fallback it is given and must still be written: absent means "never chosen", which is not
// the same as having chosen the default.

fn holds_bool(prefs: &Preferences, key: &str, value: bool) -> bool {
    prefs.bool(key, !value) == value
}

fn holds_integer(prefs: &Preferences, key: &str, value: i64) -> bool {
    prefs.integer(key, i64::MIN) == value && prefs.integer(key, i64::MAX) == value
}

fn holds_text(prefs: &Preferences, key: &str, value: &str) -> bool {
    prefs.text(key, "") == value && prefs.text(key, "\u{0}") == value
}

fn holds_texts(prefs: &Preferences, key: &str, values: &[String]) -> bool {
    prefs.texts(key).is_some_and(|held| held == values)
}

/// A write that would change nothing is skipped and counts as saved. Most of these arrive on
/// the Qt thread, and rewriting and syncing the whole file there for nothing stalls the
/// interface.
fn update(holds: impl FnOnce(&Preferences) -> bool, change: impl FnOnce(&mut Preferences)) -> bool {
    let mut guard = store();
    let Some(current) = guard.as_ref() else { return false };
    if holds(current) {
        return true;
    }
    let mut candidate = current.clone();
    if !candidate.can_save() {
        tracing::error!(path = %candidate.path().display(), "settings cannot be saved");
        return false;
    }
    change(&mut candidate);
    if let Err(err) = candidate.save() {
        tracing::error!(%err, path = %candidate.path().display(), "settings not saved");
        return false;
    }
    *guard = Some(candidate);
    GENERATION.fetch_add(1, Ordering::Release);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Never saved: the path sits under a directory that does not exist.
    fn blank() -> Preferences {
        Preferences::load_from("/nonexistent/kavverna-test/settings.json")
    }

    #[test]
    fn an_absent_key_is_never_taken_for_its_default() {
        let prefs = blank();

        assert!(!holds_bool(&prefs, "k", false));
        assert!(!holds_bool(&prefs, "k", true));
        assert!(!holds_integer(&prefs, "k", 0));
        assert!(!holds_text(&prefs, "k", ""));
        assert!(!holds_texts(&prefs, "k", &[]));
    }

    #[test]
    fn a_stored_value_holds_and_a_different_one_does_not() {
        let mut prefs = blank();
        prefs.set_bool("b", false);
        prefs.set_integer("i", i64::MIN);
        prefs.set_text("t", "");
        prefs.set_texts("l", &[]);

        assert!(holds_bool(&prefs, "b", false));
        assert!(!holds_bool(&prefs, "b", true));
        assert!(holds_integer(&prefs, "i", i64::MIN));
        assert!(!holds_integer(&prefs, "i", 0));
        assert!(holds_text(&prefs, "t", ""));
        assert!(!holds_text(&prefs, "t", "x"));
        assert!(holds_texts(&prefs, "l", &[]));
        assert!(!holds_texts(&prefs, "l", &["x".to_owned()]));
    }

    #[test]
    fn a_removed_feature_is_not_installed_whatever_its_switch_says() {
        let mut prefs = blank();
        assert!(is_installed_in(&prefs, Feature::ClipboardHistory));

        prefs.set_bool(&Feature::ClipboardHistory.availability_key(), false);

        assert!(!is_installed_in(&prefs, Feature::ClipboardHistory));
    }
}
