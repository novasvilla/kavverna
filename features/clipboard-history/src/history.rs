//! The clipboard history as a running feature.
//!
//! Selection changes and interface commands share one channel, so the store needs no lock and
//! a copy landing cannot race a button press.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::auto_clear::AutoClear;
use crate::entry::{self, Entry, Kind, MAX_FILES, MAX_IMAGE_BYTES, StoredImage};
use crate::selection::{
    CapturePolicy, Payload, Selection, SelectionEvent, SelectionWatcher, WatchError,
};
use crate::sensitivity::looks_sensitive;
use crate::store::{Captured, Store, StoreError, Summary, Window};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// With this off nothing is read at all: the compositor still says a copy happened, which
    /// is all auto clear needs, but the content never reaches this process.
    pub keep_history: bool,
    pub limit: u32,
    pub skip_sensitive: bool,
    pub images_and_files: bool,
    pub clear_after: Option<Duration>,
    pub clean_links: bool,
    pub rules: link_cleaner::Rules,
    /// Unpinned entries last copied more than this many days ago are forgotten. `None`, or
    /// zero from a hand-edited file, keeps them until the count limit pushes them out.
    pub keep_days: Option<u32>,
}

impl Settings {
    pub fn expiry_age(&self) -> Option<Duration> {
        const DAY: u64 = 24 * 60 * 60;
        self.keep_days.filter(|days| *days > 0).map(|days| Duration::from_secs(days as u64 * DAY))
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keep_history: true,
            limit: entry::DEFAULT_LIMIT,
            skip_sensitive: true,
            images_and_files: true,
            clear_after: None,
            clean_links: false,
            rules: link_cleaner::Rules::default(),
            keep_days: None,
        }
    }
}

#[derive(Debug)]
pub enum Command {
    /// Starts a new query and returns to the top of its results.
    Search(String),
    /// Moves the published rows to another slice of the same query.
    Page(Window),
    /// Answers with the whole entry, full text included, which the list never carries.
    Read {
        id: i64,
        answer: SyncSender<Option<Entry>>,
    },
    PutBack(i64),
    /// Puts the chosen text entries on the clipboard as one text, in the order given. The
    /// pieces stay where they are in the list and the joined text is not saved as an entry of
    /// its own: it is our own write, which the watcher does not report back as a copy.
    PutBackJoined {
        ids: Vec<i64>,
        separator: String,
    },
    Pin {
        id: i64,
        pinned: bool,
    },
    Move {
        id: i64,
        towards_top: bool,
    },
    /// Edits a text entry; `Store::rewrite` says what the entry keeps.
    Rewrite {
        id: i64,
        text: String,
    },
    Forget(i64),
    ClearUnpinned,
    ClearClipboard {
        finished: Option<SyncSender<bool>>,
    },
    /// Works the transformation out and shows the result; the clipboard is not touched.
    PreviewTransform(crate::transform::Transformation),
    /// Puts the last previewed result on the clipboard.
    ApplyTransform,
    DiscardTransform,
    AdoptKlipperHistory,
    Apply(Settings),
    Stop,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// The rows inside `window`, not the whole history.
    pub rows: Vec<Summary>,
    /// How many rows the query matches in all, of which `rows` is one window.
    pub total: usize,
    pub window: Window,
    pub query: String,
    pub pinned: usize,
    pub recent: usize,
    /// What the last transformation had to say, cleared by the next copy.
    pub notice: String,
    /// The parameters taken out of the last copied link, cleared by the next copy.
    pub cleaned: Vec<String>,
    /// The previewed result, elided for the panel: a copied novel should not travel to the
    /// interface whole. Empty when nothing is staged.
    pub preview: String,
    /// Whether the selection right now offers text, and whether it offers html, so the buttons
    /// can say why they are off instead of doing nothing.
    pub can_transform: bool,
    pub can_markdown: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Watch(#[from] WatchError),
    #[error("the clipboard thread would not start")]
    ThreadGone,
}

/// Dropping this stops the thread and unbinds from the compositor.
pub struct History {
    commands: Sender<Event>,
    thread: Option<JoinHandle<()>>,
}

/// Holding one of these does not keep the feature alive; only `History` does.
#[derive(Clone)]
pub struct Commands(Sender<Event>);

impl Commands {
    pub fn send(&self, command: Command) -> bool {
        self.0.send(Event::Asked(command)).is_ok()
    }

    /// The whole entry, asked of the history thread. `None` for an entry that is gone, and
    /// also for a thread that does not answer in time, so a caller on the interface thread is
    /// never held for long.
    pub fn read(&self, id: i64) -> Option<Entry> {
        let (answer, reply) = sync_channel(1);
        if !self.send(Command::Read { id, answer }) {
            return None;
        }
        reply.recv_timeout(READ_PATIENCE).ok().flatten()
    }
}

const READ_PATIENCE: Duration = Duration::from_secs(2);

impl History {
    pub fn commands(&self) -> Commands {
        Commands(self.commands.clone())
    }

    pub fn start(
        root: &Path,
        settings: Settings,
    ) -> Result<(Self, Receiver<Snapshot>), StartError> {
        let (events_out, events_in) = channel();
        let (snapshots_out, snapshots_in) = channel();
        let (ready_out, ready_in) = sync_channel(1);

        let root = root.to_path_buf();
        let commands = events_out.clone();
        let thread = std::thread::Builder::new()
            .name("kavverna-history".into())
            .spawn(move || run(root, settings, events_out, events_in, snapshots_out, ready_out))
            .map_err(|_| StartError::ThreadGone)?;

        match ready_in.recv() {
            Ok(Ok(())) => Ok((Self { commands, thread: Some(thread) }, snapshots_in)),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(StartError::ThreadGone),
        }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(Event::Asked(command));
    }

    pub fn read(&self, id: i64) -> Option<Entry> {
        self.commands().read(id)
    }
}

impl Drop for History {
    fn drop(&mut self) {
        self.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

enum Event {
    Copied(SelectionEvent),
    Asked(Command),
}

fn run(
    root: PathBuf,
    settings: Settings,
    events_out: Sender<Event>,
    events_in: Receiver<Event>,
    snapshots: Sender<Snapshot>,
    ready: SyncSender<Result<(), StartError>>,
) {
    let mut store = match Store::open(&root) {
        Ok(store) => store,
        Err(err) => {
            let _ = ready.send(Err(err.into()));
            return;
        }
    };

    let policy = Arc::new(CapturePolicy::default());
    apply_policy(&policy, &settings);

    let report = events_out.clone();
    let watcher = match SelectionWatcher::start(
        Arc::clone(&policy),
        Box::new(move |event| {
            let _ = report.send(Event::Copied(event));
        }),
    ) {
        Ok(watcher) => watcher,
        Err(err) => {
            let _ = ready.send(Err(err.into()));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    let mut settings = settings;
    let mut view = View::default();
    let mut notice = String::new();
    // The previewed transformation, waiting for Use it. A new copy discards it: the thing it
    // was made from is gone.
    let mut staged: Option<String> = None;
    let mut cleaned = Vec::new();
    let mut clearing = AutoClear::default();
    clearing.set_delay(settings.clear_after);
    let mut due = Upkeep::starting(Instant::now());
    keep_up(&mut store, &settings, &mut due, Instant::now());
    publish(&store, &watcher, &view, &notice, &cleaned, staged.as_deref(), &snapshots);

    loop {
        let changed = match events_in.recv_timeout(TICK) {
            Ok(Event::Copied(SelectionEvent::Copied {
                selection: Selection::Clipboard,
                payload,
                plain_only,
            })) => {
                clearing.noticed_copy(Instant::now());
                let (payload, removed) = tidy(&watcher, payload, plain_only, &settings);
                let was_cleaned = !removed.is_empty();
                cleaned = removed;
                notice.clear();
                staged = None;
                save(&mut store, payload, &settings) || settings.keep_history || was_cleaned
            }
            Ok(Event::Copied(SelectionEvent::Changed(Selection::Clipboard))) => {
                clearing.noticed_copy(Instant::now());
                false
            }
            // Emptying the clipboard is not a reason to forget what was copied before it, and
            // there is nothing left to clear.
            Ok(Event::Copied(SelectionEvent::Emptied(_))) => {
                clearing.forget();
                false
            }
            Ok(Event::Copied(_)) => false,
            Ok(Event::Asked(Command::Stop)) => break,
            Ok(Event::Asked(Command::ClearClipboard { finished })) => {
                empty(&watcher, &mut clearing, finished);
                false
            }
            Ok(Event::Asked(Command::PreviewTransform(wanted))) => {
                (staged, notice) = preview(&watcher, wanted);
                true
            }
            Ok(Event::Asked(Command::ApplyTransform)) => {
                match staged.take() {
                    Some(text) => {
                        watcher.offer(Selection::Clipboard, Payload::Text(text));
                        notice = "On the clipboard; the next paste is the result.".into();
                    }
                    None => notice.clear(),
                }
                true
            }
            Ok(Event::Asked(Command::DiscardTransform)) => {
                staged = None;
                notice.clear();
                true
            }
            Ok(Event::Asked(Command::PutBackJoined { ids, separator })) => {
                notice = put_back_joined(&store, &watcher, &ids, &separator);
                true
            }
            Ok(Event::Asked(command)) => {
                if matches!(command, Command::Apply(_) | Command::AdoptKlipperHistory) {
                    // A shorter age, or entries adopted with their old times, should not wait
                    // an hour to take effect.
                    due.expire_now();
                }
                let changed = act(&mut store, &watcher, &mut settings, &mut view, command);
                clearing.set_delay(settings.clear_after);
                apply_policy(&policy, &settings);
                changed
            }
            Err(RecvTimeoutError::Timeout) => {
                if clearing.due(Instant::now()) {
                    empty(&watcher, &mut clearing, None);
                }
                false
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };

        let kept_up = keep_up(&mut store, &settings, &mut due, Instant::now());
        if changed || kept_up {
            publish(&store, &watcher, &view, &notice, &cleaned, staged.as_deref(), &snapshots);
        }
    }
}

/// What the list shows: the query and which slice of its results.
#[derive(Default)]
struct View {
    query: String,
    window: Window,
}

/// Work the thread does on its own schedule rather than in answer to anything.
struct Upkeep {
    expire_at: Instant,
    thumbnails_owed: bool,
}

/// Expiry is measured in days, so an hour late is nothing, and checking once an hour keeps
/// the delete off every copy.
const EXPIRY_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Decoding a screenshot takes tens of milliseconds; a few per pass keeps a copy that lands
/// meanwhile from waiting behind a whole backlog.
const THUMBNAILS_PER_PASS: usize = 4;

impl Upkeep {
    fn starting(now: Instant) -> Self {
        Self { expire_at: now, thumbnails_owed: true }
    }

    fn expire_now(&mut self) {
        self.expire_at = Instant::now();
    }
}

/// Returns whether anything the list shows changed.
fn keep_up(store: &mut Store, settings: &Settings, due: &mut Upkeep, now: Instant) -> bool {
    let mut changed = false;
    if now >= due.expire_at {
        due.expire_at = now + EXPIRY_INTERVAL;
        if let Some(age) = settings.expiry_age() {
            match store.expire(age, std::time::SystemTime::now()) {
                Ok(0) => {}
                Ok(removed) => {
                    tracing::info!(removed, "forgot clipboard entries past their age");
                    changed = true;
                }
                Err(err) => tracing::error!(%err, "could not forget old clipboard entries"),
            }
        }
    }
    if due.thumbnails_owed {
        match store.backfill_thumbnails(THUMBNAILS_PER_PASS) {
            Ok(0) => due.thumbnails_owed = false,
            Ok(_) => changed = true,
            Err(err) => {
                tracing::error!(%err, "could not make the missing thumbnails");
                due.thumbnails_owed = false;
            }
        }
    }
    changed
}

/// Short enough that a five second delay is not noticeably late, and idle the rest of the time.
const TICK: Duration = Duration::from_secs(1);

/// Replaces a copied link with its cleaned version before anything else sees it, so what is
/// pasted and what is saved are the same thing. Our own write carries the marker the watcher
/// ignores, so this does not come back around.
fn tidy(
    watcher: &SelectionWatcher,
    payload: Payload,
    plain_only: bool,
    settings: &Settings,
) -> (Payload, Vec<String>) {
    if !settings.clean_links || !plain_only {
        return (payload, Vec::new());
    }
    let Payload::Text(text) = &payload else {
        return (payload, Vec::new());
    };
    let Some(cleaned) = link_cleaner::clean(text, &settings.rules) else {
        return (payload, Vec::new());
    };

    tracing::info!(removed = ?cleaned.removed, "took the tracking out of a copied link");
    watcher.offer(Selection::Clipboard, Payload::Text(cleaned.link.clone()));
    (Payload::Text(cleaned.link), cleaned.removed)
}

fn apply_policy(policy: &CapturePolicy, settings: &Settings) {
    policy.read_content.store(settings.keep_history || settings.clean_links, Ordering::Relaxed);
    policy.images_and_files.store(settings.images_and_files, Ordering::Relaxed);
}

fn empty(watcher: &SelectionWatcher, clearing: &mut AutoClear, finished: Option<SyncSender<bool>>) {
    watcher.clear(Selection::Clipboard, finished);
    clearing.forget();
}

fn act(
    store: &mut Store,
    watcher: &SelectionWatcher,
    settings: &mut Settings,
    view: &mut View,
    command: Command,
) -> bool {
    match command {
        Command::PutBack(id) => put_back(store, watcher, id),
        other => change(store, settings, view, other),
    }
}

/// The commands that need only the store, kept apart from the clipboard so they can be tested
/// without a compositor. Returns whether the list should be published again.
fn change(store: &mut Store, settings: &mut Settings, view: &mut View, command: Command) -> bool {
    let outcome = match command {
        Command::Search(wanted) => {
            view.query = wanted;
            view.window.offset = 0;
            return true;
        }
        Command::Page(window) => {
            view.window = window.clamped();
            return true;
        }
        Command::Read { id, answer } => {
            let found = store.entry(id).unwrap_or_else(|err| {
                tracing::error!(%err, "could not read a clipboard entry");
                None
            });
            let _ = answer.try_send(found);
            return false;
        }
        Command::Pin { id, pinned } => store.set_pinned(id, pinned),
        Command::Move { id, towards_top } => store.move_entry(id, towards_top).map(|_| ()),
        Command::Rewrite { id, text } => store.rewrite(id, &text).map(|done| {
            if !done {
                tracing::info!(id, "left an entry as it was: gone, not text, or blank");
            }
        }),
        Command::Forget(id) => store.forget(id),
        Command::ClearUnpinned => store.clear_unpinned(),
        Command::AdoptKlipperHistory => {
            crate::klipper::import_into(store, settings.skip_sensitive, settings.limit).map(|_| ())
        }
        Command::Apply(wanted) => {
            let limit = wanted.limit;
            *settings = wanted;
            store.trim_to(entry::sanitized_limit(limit))
        }
        Command::Stop
        | Command::PutBack(_)
        | Command::PutBackJoined { .. }
        | Command::ClearClipboard { .. }
        | Command::PreviewTransform(_)
        | Command::ApplyTransform
        | Command::DiscardTransform => return false,
    };

    if let Err(err) = outcome {
        tracing::error!(%err, "the clipboard history could not be changed");
        return false;
    }
    true
}

/// Answers with the sentence the panel shows, since a selection of pictures alone joins into
/// nothing and a button that silently does nothing looks broken.
fn put_back_joined(
    store: &Store,
    watcher: &SelectionWatcher,
    ids: &[i64],
    separator: &str,
) -> String {
    match store.joined(ids, separator) {
        Ok(Some(text)) => {
            watcher.offer(Selection::Clipboard, Payload::Text(text));
            "On the clipboard; the next paste is the selection joined.".into()
        }
        Ok(None) => "Nothing in that selection is text to join.".into(),
        Err(err) => {
            tracing::error!(%err, "could not join the selection");
            "The selection could not be read.".into()
        }
    }
}

/// A stale entry leaves the clipboard untouched rather than emptying it.
fn put_back(store: &mut Store, watcher: &SelectionWatcher, id: i64) -> bool {
    let Ok(Some(entry)) = store.entry(id) else {
        return false;
    };
    let Some(payload) = payload_for(store, &entry) else {
        tracing::info!(id, "that entry no longer has anything to paste");
        return false;
    };

    watcher.offer(Selection::Clipboard, payload);
    if let Err(err) = store.touch(id) {
        tracing::error!(%err, "could not record the paste");
    }
    true
}

fn payload_for(store: &Store, entry: &Entry) -> Option<Payload> {
    match entry.kind {
        Kind::Text => Some(Payload::Text(entry.text.clone())),
        Kind::Image => {
            let image = entry.image.as_ref()?;
            std::fs::read(store.image_path(&image.digest)).ok().map(Payload::Image)
        }
        Kind::Files => {
            let alive: Vec<PathBuf> =
                entry.file_paths.iter().filter(|path| path.exists()).cloned().collect();
            (!alive.is_empty()).then_some(Payload::Files(alive))
        }
    }
}

fn save(store: &mut Store, payload: Payload, settings: &Settings) -> bool {
    // Link cleaning needs the content read to rewrite it, which is not permission to keep it.
    // The gate belongs on the write as well as on the read, or switching the history off while
    // anything else is on quietly fills the database.
    if !settings.keep_history {
        return false;
    }
    let Some(captured) = worth_keeping(payload, settings) else {
        return false;
    };
    match store
        .remember(captured)
        .and_then(|_| store.trim_to(entry::sanitized_limit(settings.limit)))
    {
        Ok(()) => true,
        Err(err) => {
            tracing::error!(%err, "could not save what was copied");
            false
        }
    }
}

fn worth_keeping(payload: Payload, settings: &Settings) -> Option<Captured> {
    match payload {
        Payload::Text(raw) => {
            let text = entry::storable_text(&raw)?;
            if settings.skip_sensitive && looks_sensitive(&text) {
                tracing::debug!("a copy that looks like a secret was not saved");
                return None;
            }
            Some(Captured { kind: Kind::Text, text, file_paths: Vec::new(), image: None })
        }
        Payload::Image(png) => {
            if png.len() as u64 > MAX_IMAGE_BYTES {
                tracing::debug!(bytes = png.len(), "a copied image was too large to keep");
                return None;
            }
            let (width, height) = measure(&png)?;
            let digest = blake3::hash(&png).to_hex().to_string();
            Some(Captured {
                kind: Kind::Image,
                text: String::new(),
                file_paths: Vec::new(),
                image: Some((StoredImage { digest, width, height }, png)),
            })
        }
        Payload::Files(paths) => {
            if paths.is_empty() || paths.len() > MAX_FILES {
                return None;
            }
            Some(Captured {
                kind: Kind::Files,
                text: String::new(),
                file_paths: paths,
                image: None,
            })
        }
    }
}

/// Reads the header only: a copied screenshot can be tens of megabytes.
fn measure(png: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(png)).with_guessed_format().ok()?.into_dimensions().ok()
}

fn publish(
    store: &Store,
    watcher: &SelectionWatcher,
    view: &View,
    notice: &str,
    cleaned: &[String],
    staged: Option<&str>,
    snapshots: &Sender<Snapshot>,
) {
    let page = match store.page(&view.query, view.window) {
        Ok(page) => page,
        Err(err) => {
            tracing::error!(%err, "could not read the clipboard history");
            return;
        }
    };
    let (pinned, recent) = store.counts().unwrap_or_default();
    let offered = watcher.ask_current(None).types;
    let _ = snapshots.send(Snapshot {
        rows: page.rows,
        total: page.total,
        window: view.window,
        query: view.query.clone(),
        pinned,
        recent,
        notice: notice.to_string(),
        cleaned: cleaned.to_vec(),
        preview: staged.map(elided_for_panel).unwrap_or_default(),
        can_transform: crate::selection::preferred_text(&offered).is_some(),
        can_markdown: offered.iter().any(|mime| mime == "text/html"),
    });
}

/// Enough to judge the result by; the full text only ever moves on Use it.
fn elided_for_panel(text: &str) -> String {
    const SHOWN: usize = 1200;
    let mut shown: String = text.chars().take(SHOWN).collect();
    if shown.len() < text.len() {
        shown.push('…');
    }
    shown
}

/// One sentence saying what is ready, with a measure of it, so the panel reports what would
/// change rather than only that something would.
fn ready_sentence(wanted: crate::transform::Transformation, result: &str) -> String {
    use crate::transform::Transformation;
    match wanted {
        Transformation::Plain => {
            format!("Ready: the text alone, {} characters.", result.chars().count())
        }
        Transformation::Json => {
            format!("Ready: JSON laid out over {} lines.", result.lines().count())
        }
        Transformation::Markdown => {
            format!("Ready: Markdown, {} characters from this copy's HTML.", result.chars().count())
        }
    }
}

/// Reads the selection as it stands and works the transformation out, touching nothing: the
/// result waits for Use it. The answer is a sentence for the panel either way, because a
/// button that quietly does nothing looks dead rather than refused.
fn preview(
    watcher: &SelectionWatcher,
    wanted: crate::transform::Transformation,
) -> (Option<String>, String) {
    use crate::transform::Transformation;

    let current_text = || {
        let offered = watcher.ask_current(None).types;
        let mime = crate::selection::preferred_text(&offered)?;
        let bytes = watcher.ask_current(Some(mime)).content?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        (!text.is_empty()).then_some(text)
    };

    let made = match wanted {
        Transformation::Plain => match current_text() {
            None => return (None, "Nothing on the clipboard to make plain.".into()),
            Some(text) => text,
        },
        Transformation::Json => match current_text() {
            None => return (None, "Nothing on the clipboard to lay out.".into()),
            Some(text) => match crate::transform::pretty_json(&text) {
                Ok(json) => json,
                Err(refusal) => return (None, format!("Left alone: {refusal}.")),
            },
        },
        Transformation::Markdown => match watcher.ask_current(Some("text/html")).content {
            None => return (None, "This copy offers no HTML to turn into Markdown.".into()),
            Some(bytes) => crate::transform::markdown_from_html(&String::from_utf8_lossy(&bytes)),
        },
    };

    let said = ready_sentence(wanted, &made);
    (Some(made), said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ready_sentence_measures_what_it_offers() {
        use crate::transform::Transformation;
        assert_eq!(
            ready_sentence(Transformation::Plain, "hello"),
            "Ready: the text alone, 5 characters."
        );
        assert_eq!(
            ready_sentence(Transformation::Json, "{\n  \"a\": 1\n}"),
            "Ready: JSON laid out over 3 lines."
        );
        assert_eq!(
            ready_sentence(Transformation::Markdown, "# hi"),
            "Ready: Markdown, 4 characters from this copy's HTML."
        );
    }

    #[test]
    fn a_novel_is_elided_for_the_panel_and_a_note_is_not() {
        let long = "x".repeat(5000);
        let shown = elided_for_panel(&long);
        assert!(shown.chars().count() == 1201 && shown.ends_with('…'));

        assert_eq!(elided_for_panel("short"), "short");
    }

    #[test]
    fn a_secret_shaped_copy_is_not_kept_unless_the_setting_is_off() {
        let secret = || Payload::Text("sk-live-4f9a2b71c8e0d3a6f5b2".into());
        let mut settings = Settings::default();
        assert!(worth_keeping(secret(), &settings).is_none());

        settings.skip_sensitive = false;
        assert!(worth_keeping(secret(), &settings).is_some());
    }

    /// Link cleaning has to read the content to rewrite it. Reading is not permission to keep,
    /// and the two settings are independent rows on the same page.
    #[test]
    fn with_the_history_off_nothing_is_kept_however_it_was_read() {
        let room = tempfile::tempdir().expect("a temporary directory");
        let mut store = Store::open(room.path()).unwrap();
        let settings = Settings { keep_history: false, clean_links: true, ..Settings::default() };

        assert!(!save(&mut store, Payload::Text("a copy nobody asked to keep".into()), &settings));
        assert_eq!(store.counts().unwrap(), (0, 0));
    }

    #[test]
    fn blank_text_is_never_kept() {
        assert!(worth_keeping(Payload::Text("   \n".into()), &Settings::default()).is_none());
    }

    #[test]
    fn a_copy_of_too_many_files_is_a_folder_operation() {
        let many = (0..MAX_FILES + 1).map(|n| PathBuf::from(format!("/tmp/{n}"))).collect();
        assert!(worth_keeping(Payload::Files(many), &Settings::default()).is_none());
        assert!(worth_keeping(Payload::Files(Vec::new()), &Settings::default()).is_none());
    }

    #[test]
    fn something_that_is_not_a_picture_is_not_stored_as_one() {
        assert!(
            worth_keeping(Payload::Image(b"not a png".to_vec()), &Settings::default()).is_none()
        );
    }

    #[test]
    fn the_same_picture_gets_the_same_name_twice() {
        let png = tiny_png();
        let first = worth_keeping(Payload::Image(png.clone()), &Settings::default()).unwrap();
        let again = worth_keeping(Payload::Image(png), &Settings::default()).unwrap();

        let name = |captured: &Captured| captured.image.as_ref().unwrap().0.digest.clone();
        assert_eq!(name(&first), name(&again));
        assert_eq!(first.image.unwrap().0.width, 2);
    }

    #[test]
    fn an_identifier_is_kept_and_a_key_is_still_skipped() {
        let kept = |copied: &str| worth_keeping(Payload::Text(copied.into()), &Settings::default());
        assert!(kept("6ba7b810-9dad-11d1-80b4-00c04fd430c8").is_some());
        assert!(kept("  urn:uuid:6BA7B810-9DAD-11D1-80B4-00C04FD430C8\n").is_some());
        assert!(kept("ghp_9fT2kLq8Zx4mN7pR1sV6wY3bC5dE0aH2jK4").is_none());
    }

    #[test]
    fn zero_days_and_no_days_both_keep_forever() {
        let with = |keep_days| Settings { keep_days, ..Settings::default() };
        assert_eq!(with(None).expiry_age(), None);
        assert_eq!(with(Some(0)).expiry_age(), None);
        assert_eq!(with(Some(2)).expiry_age(), Some(Duration::from_secs(2 * 24 * 60 * 60)));
    }

    fn text(body: &str) -> Captured {
        Captured { kind: Kind::Text, text: body.into(), file_paths: Vec::new(), image: None }
    }

    #[test]
    fn upkeep_forgets_by_age_on_its_first_pass_and_then_waits() {
        let room = tempfile::tempdir().expect("a temporary directory");
        let mut store = Store::open(room.path()).unwrap();
        let now = std::time::SystemTime::now();
        let old = store.remember(text("from last month")).unwrap();
        store.backdate(old, now - Duration::from_secs(40 * 24 * 60 * 60)).unwrap();
        let pinned = store.remember(text("pinned last year")).unwrap();
        store.backdate(pinned, now - Duration::from_secs(400 * 24 * 60 * 60)).unwrap();
        store.set_pinned(pinned, true).unwrap();

        let settings = Settings { keep_days: Some(30), ..Settings::default() };
        let started = Instant::now();
        let mut due = Upkeep::starting(started);
        assert!(keep_up(&mut store, &settings, &mut due, started));
        assert_eq!(store.counts().unwrap(), (1, 0));
        assert!(due.expire_at >= started + EXPIRY_INTERVAL);

        let again = store.remember(text("also old")).unwrap();
        store.backdate(again, now - Duration::from_secs(40 * 24 * 60 * 60)).unwrap();
        assert!(!keep_up(&mut store, &settings, &mut due, started + TICK), "not due yet");
        assert_eq!(store.counts().unwrap(), (1, 1));
    }

    /// The path the panel's edit button takes, short of the clipboard itself.
    #[test]
    fn an_edit_sent_as_a_command_lands_in_the_store_and_the_index() {
        let room = tempfile::tempdir().expect("a temporary directory");
        let mut store = Store::open(room.path()).unwrap();
        let mut settings = Settings::default();
        let mut view = View::default();
        let id = store.remember(text("teh typo")).unwrap();
        store.remember(text("something newer")).unwrap();

        let rewrite = Command::Rewrite { id, text: "the fix".into() };
        assert!(change(&mut store, &mut settings, &mut view, rewrite));

        let (answer, reply) = sync_channel(1);
        assert!(!change(&mut store, &mut settings, &mut view, Command::Read { id, answer }));
        assert_eq!(reply.recv().unwrap().unwrap().text, "the fix");

        change(&mut store, &mut settings, &mut view, Command::Search("fix".into()));
        let page = store.page(&view.query, view.window).unwrap();
        assert_eq!(page.rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![id]);
        assert!(store.page("typo", view.window).unwrap().rows.is_empty());
    }

    #[test]
    fn a_new_search_starts_at_the_top_of_its_results() {
        let room = tempfile::tempdir().expect("a temporary directory");
        let mut store = Store::open(room.path()).unwrap();
        let mut settings = Settings::default();
        let mut view = View::default();

        let further = Command::Page(Window { offset: 40, limit: 20 });
        change(&mut store, &mut settings, &mut view, further);
        assert_eq!(view.window, Window { offset: 40, limit: 20 });

        change(&mut store, &mut settings, &mut view, Command::Search("x".into()));
        assert_eq!(view.window, Window { offset: 0, limit: 20 });
    }

    fn tiny_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        let picture = image::RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 255]));
        picture
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("a picture we just made should encode");
        bytes
    }
}
