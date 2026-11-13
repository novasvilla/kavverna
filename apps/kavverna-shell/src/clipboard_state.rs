use crate::settings;
use clipboard_history::{
    Command, Commands, Entry, History, Kind, Settings, Snapshot, Summary, Window,
};
use feature_catalog::Feature;
use preferences::Preferences;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

fn commands() -> Option<Commands> {
    COMMANDS.lock().ok().and_then(|held| held.clone())
}

pub fn send(command: Command) -> bool {
    match commands() {
        Some(commands) => commands.send(command),
        None => {
            tracing::warn!("the clipboard history is not running, change dropped");
            false
        }
    }
}

/// Blocks for up to two seconds, so never on the Qt thread.
pub fn read(id: i64) -> Option<Entry> {
    commands()?.read(id)
}

/// Waits this long after the last keystroke, so typing a word runs one query rather than one
/// per letter.
const SEARCH_QUIET: Duration = Duration::from_millis(150);

/// What was typed, held until the typing stops.
#[derive(Debug)]
struct Debounce {
    quiet: Duration,
    pending: Option<(String, Instant)>,
}

impl Debounce {
    fn new(quiet: Duration) -> Self {
        Self { quiet, pending: None }
    }

    fn typed(&mut self, text: String, now: Instant) {
        self.pending = Some((text, now + self.quiet));
    }

    /// How long until something is due. `None` when nothing is waiting.
    fn wait(&self, now: Instant) -> Option<Duration> {
        self.pending.as_ref().map(|(_, due)| due.saturating_duration_since(now))
    }

    fn due(&mut self, now: Instant) -> Option<String> {
        match &self.pending {
            Some((_, due)) if *due <= now => self.pending.take().map(|(text, _)| text),
            _ => None,
        }
    }
}

/// Searches once the typing settles. The thread starts on the first keystroke and then lives
/// as long as the process, idle on its channel.
pub fn search(text: String) {
    static TYPED: OnceLock<Option<Sender<String>>> = OnceLock::new();
    let typed = TYPED.get_or_init(|| {
        let (typed, keystrokes) = channel();
        std::thread::Builder::new()
            .name("kavverna-search".into())
            .spawn(move || settle_searches(keystrokes))
            .map_err(|err| tracing::error!(%err, "no thread to settle searches on"))
            .ok()?;
        Some(typed)
    });
    match typed {
        Some(typed) => {
            let _ = typed.send(text);
        }
        None => {
            send(Command::Search(text));
        }
    }
}

fn settle_searches(keystrokes: Receiver<String>) {
    let mut debounce = Debounce::new(SEARCH_QUIET);
    loop {
        let next = match debounce.wait(Instant::now()) {
            None => keystrokes.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(wait) => keystrokes.recv_timeout(wait),
        };
        match next {
            Ok(text) => debounce.typed(text, Instant::now()),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if let Some(text) = debounce.due(Instant::now()) {
            send(Command::Search(text));
        }
    }
}

/// The kind a person would name: a text that is one web address is a link. Judged on the
/// preview, which keeps the start of the text, and a link longer than the cut is still one
/// address with no spaces in it.
pub fn kind_name(kind: Kind, preview: &str) -> &'static str {
    let address = preview.trim().trim_end_matches('\u{2026}');
    let link = (address.starts_with("https://") || address.starts_with("http://"))
        && !address.contains(char::is_whitespace);
    if kind == Kind::Text && link { "link" } else { kind.as_str() }
}

/// What reading an entry whole gives back: the text itself, a list of files one path a line,
/// and for a picture its size, having no text of its own.
pub fn full_text(entry: &Entry) -> String {
    match entry.kind {
        Kind::Text => entry.text.clone(),
        Kind::Files => entry
            .file_paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n"),
        Kind::Image => entry.preview(),
    }
}

/// Scripts and agents read the history only while it is something the person chose to keep.
/// The store stays open for Clean URL or Transform with the history removed, and what it holds
/// from before is not theirs to hand out then.
fn readable() -> Result<Commands, String> {
    if !settings::is_installed(Feature::ClipboardHistory) || !keeps_history() {
        return Err("the clipboard history is switched off".into());
    }
    commands().ok_or_else(|| "the clipboard history is not running".into())
}

/// One window of the history as JSON, for `dev.kavverna.Shell.ClipboardEntries`. Blocks.
pub fn entries_json(offset: u32, limit: u32, query: &str) -> Result<String, String> {
    let window = Window { offset: offset as usize, limit: limit as usize }.clamped();
    let page = readable()?.query(query, window).ok_or("the clipboard history did not answer")?;
    Ok(page_json(&page.rows, page.total, window).to_string())
}

fn page_json(rows: &[Summary], total: usize, window: Window) -> serde_json::Value {
    let entries: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.id,
                "kind": kind_name(row.kind, &row.preview),
                "preview": row.preview,
                "pinned": row.pinned,
                "copied_at": iso_utc(row.copied_at),
            })
        })
        .collect();
    serde_json::json!({ "total": total, "offset": window.offset, "entries": entries })
}

/// One entry whole, for `dev.kavverna.Shell.ClipboardEntry`. Blocks.
pub fn entry_text(id: i64) -> Result<String, String> {
    readable()?.read(id).map(|entry| full_text(&entry)).ok_or_else(|| format!("no entry {id}"))
}

/// RFC 3339 in UTC, which every language parses without a time zone database.
fn iso_utc(at: SystemTime) -> String {
    let seconds = at.duration_since(UNIX_EPOCH).map(|since| since.as_secs() as i64).unwrap_or(0);
    let (days, of_day) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date, after Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let of_era = z.rem_euclid(146_097);
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * of_year + 2) / 153;
    let day = (of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
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
    fn typing_settles_into_one_search_for_the_last_text() {
        let start = Instant::now();
        let at = |millis| start + Duration::from_millis(millis);
        let mut debounce = Debounce::new(SEARCH_QUIET);
        assert_eq!(debounce.wait(start), None);

        debounce.typed("a".into(), start);
        debounce.typed("ap".into(), at(100));
        assert_eq!(debounce.due(at(200)), None, "still within the quiet after the last key");
        assert_eq!(debounce.wait(at(200)), Some(Duration::from_millis(50)));

        assert_eq!(debounce.due(at(250)).as_deref(), Some("ap"));
        assert_eq!(debounce.due(at(400)), None, "asked once, not once per key");
        assert_eq!(debounce.wait(at(400)), None);
    }

    #[test]
    fn a_link_is_one_address_and_nothing_else() {
        assert_eq!(kind_name(Kind::Text, "https://example.org/a?b=c"), "link");
        assert_eq!(kind_name(Kind::Text, "  http://example.org "), "link");
        let long = format!("https://example.org/{}\u{2026}", "x".repeat(200));
        assert_eq!(kind_name(Kind::Text, &long), "link");
        assert_eq!(kind_name(Kind::Text, "see https://example.org"), "text");
        assert_eq!(kind_name(Kind::Text, "https://example.org and more"), "text");
        assert_eq!(kind_name(Kind::Files, "https://example.org"), "files");
        assert_eq!(kind_name(Kind::Image, "1920\u{d7}1080"), "image");
    }

    #[test]
    fn times_for_scripts_are_written_in_utc() {
        assert_eq!(iso_utc(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let leap_day = UNIX_EPOCH + Duration::from_secs(1_709_210_096);
        assert_eq!(iso_utc(leap_day), "2024-02-29T12:34:56Z");
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn a_page_for_scripts_says_where_it_is_and_what_each_entry_is() {
        let row = Summary {
            id: 7,
            kind: Kind::Text,
            preview: "say \"hi\"".into(),
            pinned: true,
            copied_at: UNIX_EPOCH,
            image: None,
            thumbnail: None,
            file_count: 0,
        };
        let json = page_json(&[row], 12, Window { offset: 5, limit: 1 });
        assert_eq!(
            json.to_string(),
            r#"{"entries":[{"copied_at":"1970-01-01T00:00:00Z","id":7,"kind":"text","pinned":true,"preview":"say \"hi\""}],"offset":5,"total":12}"#
        );
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
