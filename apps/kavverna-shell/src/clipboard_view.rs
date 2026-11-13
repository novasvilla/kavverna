use crate::{clipboard_state, settings};
use clipboard_history::entry::MAX_CHARACTERS;
use clipboard_history::{Command, Kind, Snapshot, Summary, Transformation, Window};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QList, QModelIndex, QString, QStringList, QUrl,
    QVariant, QVector,
};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
        type QList_bool = cxx_qt_lib::QList<bool>;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(bool, available)]
        #[qproperty(bool, enabled)]
        #[qproperty(QString, query)]
        #[qproperty(i32, total)]
        #[qproperty(i32, offset)]
        #[qproperty(bool, has_more)]
        #[qproperty(bool, has_earlier)]
        #[qproperty(i32, selected_count)]
        #[qproperty(QString, notice)]
        #[qproperty(i32, full_text_id)]
        #[qproperty(QString, full_text)]
        #[qproperty(bool, full_text_ready)]
        #[qproperty(i32, keep_days)]
        #[qproperty(QList_i32, keep_days_choices)]
        #[qproperty(i32, pinned_count)]
        #[qproperty(i32, recent_count)]
        #[qproperty(i32, limit)]
        #[qproperty(bool, images_and_files)]
        #[qproperty(bool, skip_sensitive)]
        #[qproperty(i32, klipper_waiting)]
        #[qproperty(i32, clear_after)]
        #[qproperty(bool, clear_on_suspend)]
        #[qproperty(bool, clear_on_screen_lock)]
        #[qproperty(bool, clean_links)]
        #[qproperty(QStringList, clean_rule_scopes)]
        #[qproperty(QStringList, clean_rule_parameters)]
        #[qproperty(QList_bool, clean_rule_enabled)]
        #[qproperty(QList_bool, clean_rule_custom)]
        #[qproperty(QString, clean_rule_notice)]
        #[qproperty(QString, clean_notice)]
        #[qproperty(QString, transform_notice)]
        #[qproperty(QString, transform_preview)]
        #[qproperty(bool, can_transform)]
        #[qproperty(bool, can_markdown)]
        type ClipboardView = super::ClipboardViewRust;
    }

    impl cxx_qt::Threading for ClipboardView {}

    // What the list needs of its base class to announce its own changes.
    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut ClipboardView>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut ClipboardView>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut ClipboardView>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut ClipboardView>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut ClipboardView>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut ClipboardView>);
        #[inherit]
        fn index(self: &ClipboardView, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;

        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn rows_changed(
            self: Pin<&mut ClipboardView>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
    }

    unsafe extern "RustQt" {
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn row_value(self: &ClipboardView, index: &QModelIndex, role: i32) -> QVariant;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &ClipboardView, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &ClipboardView) -> QHash_i32_QByteArray;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "canFetchMore"]
        fn can_fetch_more(self: &ClipboardView, parent: &QModelIndex) -> bool;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "fetchMore"]
        fn fetch_more(self: Pin<&mut ClipboardView>, parent: &QModelIndex);
    }

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn enable(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn search(self: Pin<&mut ClipboardView>, text: &QString);
        #[qinvokable]
        fn show_more(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn show_earlier(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn put_back(self: Pin<&mut ClipboardView>, id: i32);
        #[qinvokable]
        fn pin(self: Pin<&mut ClipboardView>, id: i32, pinned: bool);
        #[qinvokable]
        fn move_towards_top(self: Pin<&mut ClipboardView>, id: i32, towards_top: bool);
        #[qinvokable]
        fn forget(self: Pin<&mut ClipboardView>, id: i32);
        #[qinvokable]
        fn clear_unpinned(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn read_full(self: Pin<&mut ClipboardView>, id: i32);
        #[qinvokable]
        fn rewrite(self: Pin<&mut ClipboardView>, id: i32, text: &QString) -> QString;
        #[qinvokable]
        fn toggle_selected(self: Pin<&mut ClipboardView>, id: i32);
        #[qinvokable]
        fn clear_selection(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn paste_selection(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn choose_keep_days(self: Pin<&mut ClipboardView>, days: i32) -> bool;
        #[qinvokable]
        fn choose_limit(self: Pin<&mut ClipboardView>, rows: i32);
        #[qinvokable]
        fn choose_images_and_files(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn choose_skip_sensitive(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn adopt_klipper_history(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn transform(self: Pin<&mut ClipboardView>, wanted: i32);
        #[qinvokable]
        fn use_transform(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn discard_transform(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn choose_clear_after(self: Pin<&mut ClipboardView>, seconds: i32);
        #[qinvokable]
        fn choose_clear_on_suspend(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn choose_clear_on_screen_lock(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn choose_clean_links(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn toggle_clean_rule(self: Pin<&mut ClipboardView>, index: i32, enabled: bool);
        #[qinvokable]
        fn add_clean_rule(
            self: Pin<&mut ClipboardView>,
            domain: &QString,
            parameter: &QString,
        ) -> bool;
        #[qinvokable]
        fn remove_clean_rule(self: Pin<&mut ClipboardView>, index: i32);
    }
}

use core::pin::Pin;

static VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::ClipboardView>>> = Mutex::new(None);

/// Raised by every notice, so the timer of an older one cannot wipe a newer one early.
static NOTICE: AtomicU64 = AtomicU64::new(0);

/// Long enough to read three words, short enough not to linger over the next action.
const NOTICE_LASTS: Duration = Duration::from_secs(2);

const NOT_RUNNING: &str = "The clipboard history is not running.";

/// Qt::UserRole, where a model's own roles start.
const ROLE_BASE: i32 = 0x0100;

/// The order is the role numbers, so a role is only ever added at the end.
const ROLE_NAMES: [&str; 10] = [
    "entry_id",
    "preview",
    "kind",
    "pinned",
    "copied_at",
    "copied_at_text",
    "thumbnail",
    "thumbnail_width",
    "thumbnail_height",
    "selected",
];

const SELECTED_ROLE: i32 = ROLE_BASE + 9;

#[derive(Default)]
pub struct ClipboardViewRust {
    available: bool,
    enabled: bool,
    query: QString,
    total: i32,
    offset: i32,
    has_more: bool,
    has_earlier: bool,
    selected_count: i32,
    notice: QString,
    full_text_id: i32,
    full_text: QString,
    full_text_ready: bool,
    keep_days: i32,
    keep_days_choices: QList<i32>,
    pinned_count: i32,
    recent_count: i32,
    limit: i32,
    images_and_files: bool,
    skip_sensitive: bool,
    klipper_waiting: i32,
    clear_after: i32,
    clear_on_suspend: bool,
    clear_on_screen_lock: bool,
    clean_links: bool,
    clean_rule_scopes: QStringList,
    clean_rule_parameters: QStringList,
    clean_rule_enabled: QList<bool>,
    clean_rule_custom: QList<bool>,
    clean_rule_notice: QString,
    clean_notice: QString,
    transform_notice: QString,
    transform_preview: QString,
    can_transform: bool,
    can_markdown: bool,

    rows: Vec<Row>,
    /// The window the rows are, as the history thread last reported it. Never a copy kept
    /// here: the thread holds one window for the panel and every snapshot says which.
    window: Window,
    /// The query the rows answer, as applied, which trails what is typed by the debounce.
    shown_query: String,
    /// A page has been asked for and not yet arrived, so a scroll does not ask twice.
    asking: bool,
    /// In the order picked, which is the order they are joined in.
    selection: Vec<(i64, Kind)>,
}

/// One row as the list draws it.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: i64,
    kind: Kind,
    kind_name: &'static str,
    preview: String,
    pinned: bool,
    /// Milliseconds since the epoch, which is what a QML Date takes.
    copied_at: i64,
    copied_at_text: String,
    /// A local path, empty for none.
    thumbnail: String,
    thumbnail_width: i32,
    thumbnail_height: i32,
}

impl Row {
    fn from_summary(summary: &Summary, now: Option<&Moment>) -> Self {
        let copied_at = summary
            .copied_at
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_millis() as i64)
            .unwrap_or(0);
        let copied_at_text = match (now, local_moment(copied_at.div_euclid(1000))) {
            (Some(now), Some(then)) => copied_at_text(&then, now),
            _ => String::new(),
        };
        let (thumbnail_width, thumbnail_height) = summary
            .image
            .as_ref()
            .map(|image| image.thumbnail_size())
            .map_or((0, 0), |(width, height)| (width as i32, height as i32));
        Self {
            id: summary.id,
            kind: summary.kind,
            kind_name: clipboard_state::kind_name(summary.kind, &summary.preview),
            preview: summary.preview.clone(),
            pinned: summary.pinned,
            copied_at,
            copied_at_text,
            thumbnail: summary
                .thumbnail
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            thumbnail_width,
            thumbnail_height,
        }
    }
}

/// One step that turns the rows shown into the rows wanted. Each is announced to the view on
/// its own, so a `ListView` keeps its delegates and its place instead of starting over.
#[derive(Debug, PartialEq, Eq)]
enum Edit {
    Remove {
        at: usize,
        count: usize,
    },
    /// Rows `at..at + count` of the new list, landing at the same position.
    Insert {
        at: usize,
        count: usize,
    },
    Change {
        at: usize,
    },
}

/// Rows are matched by id. A row that moved, which pinning and putting back both do, goes as a
/// removal and an insertion; a row whose id stayed but whose content did not is a change.
fn edits(old: &[Row], new: &[Row]) -> Vec<Edit> {
    let wanted: HashSet<i64> = new.iter().map(|row| row.id).collect();
    let mut steps = Vec::new();
    let mut working: Vec<&Row> = old.iter().collect();

    // From the end, so a removal never shifts a position still to be looked at.
    let mut at = working.len();
    while at > 0 {
        if wanted.contains(&working[at - 1].id) {
            at -= 1;
            continue;
        }
        let end = at;
        while at > 0 && !wanted.contains(&working[at - 1].id) {
            at -= 1;
        }
        steps.push(Edit::Remove { at, count: end - at });
        working.drain(at..end);
    }

    let mut present: HashSet<i64> = working.iter().map(|row| row.id).collect();
    let mut at = 0;
    while at < new.len() {
        if working.get(at).is_some_and(|row| row.id == new[at].id) {
            if *working[at] != new[at] {
                steps.push(Edit::Change { at });
                working[at] = &new[at];
            }
            at += 1;
            continue;
        }
        if present.contains(&new[at].id) {
            if let Some(from) = working.iter().skip(at).position(|row| row.id == new[at].id) {
                steps.push(Edit::Remove { at: at + from, count: 1 });
                working.remove(at + from);
                steps.push(Edit::Insert { at, count: 1 });
                working.insert(at, &new[at]);
                at += 1;
                continue;
            }
        }
        let count = new[at..].iter().take_while(|row| !present.contains(&row.id)).count();
        steps.push(Edit::Insert { at, count });
        working.splice(at..at, new[at..at + count].iter());
        present.extend(new[at..at + count].iter().map(|row| row.id));
        at += count;
    }

    if working.len() > new.len() {
        steps.push(Edit::Remove { at: new.len(), count: working.len() - new.len() });
    }
    steps
}

fn replay(rows: &mut Vec<Row>, new: &[Row], edit: &Edit) {
    match *edit {
        Edit::Remove { at, count } => {
            rows.drain(at..at + count);
        }
        Edit::Insert { at, count } => {
            rows.splice(at..at, new[at..at + count].iter().cloned());
        }
        Edit::Change { at } => rows[at] = new[at].clone(),
    }
}

/// How many rows a scroll to the end asks for at a time.
const PAGE_STEP: usize = 100;

/// The window that shows more below. It grows until a snapshot would carry as many rows as the
/// history thread allows in one read, and then slides down instead.
fn further(window: Window, total: usize) -> Option<Window> {
    if window.offset + window.limit >= total {
        return None;
    }
    let grown = (window.limit + PAGE_STEP).min(Window::MAX_LIMIT);
    if grown > window.limit {
        return Some(Window { offset: window.offset, limit: grown });
    }
    let offset = (window.offset + PAGE_STEP).min(total - window.limit);
    Some(Window { offset, limit: window.limit })
}

/// The window that shows more above, after a slide down left the top behind.
fn earlier(window: Window) -> Option<Window> {
    (window.offset > 0)
        .then(|| Window { offset: window.offset.saturating_sub(PAGE_STEP), limit: window.limit })
}

/// After deletions a window can start past the end, showing nothing of a history that still
/// has rows. It steps back so its last row is the last entry.
fn step_back(window: Window, shown: usize, total: usize) -> Option<Window> {
    (shown == 0 && total > 0 && window.offset > 0)
        .then(|| Window { offset: total.saturating_sub(window.limit), limit: window.limit })
}

/// Why an edit cannot be made, in a sentence for the editor; `None` when it can.
fn rewrite_refusal(kind: Option<Kind>, text: &str) -> Option<&'static str> {
    match kind {
        None => Some("That entry is no longer in the history."),
        Some(Kind::Image | Kind::Files) => Some("Only text can be edited."),
        Some(Kind::Text) if text.trim().is_empty() => Some("An entry cannot be empty."),
        Some(Kind::Text) if text.trim().chars().count() > MAX_CHARACTERS => {
            Some("That is longer than an entry can hold.")
        }
        Some(Kind::Text) => None,
    }
}

/// The text entries of a selection in the order picked, and the sentence that says what
/// happened to them. Pictures and files have nothing to join.
fn joined(selection: &[(i64, Kind)]) -> (Vec<i64>, String) {
    let ids: Vec<i64> =
        selection.iter().filter(|(_, kind)| *kind == Kind::Text).map(|(id, _)| *id).collect();
    let left_out = selection.len() - ids.len();
    let sentence = match (ids.len(), left_out) {
        (0, _) => "Nothing selected is text to join.".to_owned(),
        (1, 0) => "Pasted 1 entry".to_owned(),
        (count, 0) => format!("Pasted {count} entries"),
        (1, _) => "Pasted 1 entry, without the pictures and files".to_owned(),
        (count, _) => format!("Pasted {count} entries, without the pictures and files"),
    };
    (ids, sentence)
}

/// The ages the panel offers, refusing anything else: a stray number here deletes history.
fn keep_days_choice(days: i32) -> Option<i64> {
    let days = i64::from(days);
    settings::CLIPBOARD_KEEP_DAYS_CHOICES.contains(&days).then_some(days)
}

/// A wall clock reading in the local time zone, as much of it as a row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Moment {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
}

fn local_moment(seconds: i64) -> Option<Moment> {
    let time = seconds as libc::time_t;
    // SAFETY: an all zero `tm` is a valid value of a plain C struct, and `localtime_r` writes
    // only into the one it is given, which lives for the whole call.
    let mut parts: libc::tm = unsafe { std::mem::zeroed() };
    let filled = unsafe { libc::localtime_r(&time, &mut parts) };
    if filled.is_null() {
        return None;
    }
    Some(Moment {
        year: i64::from(parts.tm_year) + 1900,
        month: u32::try_from(parts.tm_mon + 1).ok()?,
        day: u32::try_from(parts.tm_mday).ok()?,
        hour: u32::try_from(parts.tm_hour).ok()?,
        minute: u32::try_from(parts.tm_min).ok()?,
    })
}

/// Days since 1970-01-01 for a proleptic Gregorian date, after Howard Hinnant's algorithm.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let shifted_month = i64::from(if month > 2 { month - 3 } else { month + 9 });
    let of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + of_year;
    era * 146_097 + of_era - 719_468
}

const MONTHS: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The time alone today, the day before as yesterday, and a date further back, with the year
/// once it is not this one.
fn copied_at_text(then: &Moment, now: &Moment) -> String {
    let clock = format!("{:02}:{:02}", then.hour, then.minute);
    let days = days_from_civil(now.year, now.month, now.day)
        - days_from_civil(then.year, then.month, then.day);
    let month = MONTHS[(then.month as usize).clamp(1, 12) - 1];
    match days {
        ..=0 => clock,
        1 => format!("Yesterday {clock}"),
        _ if then.year == now.year => format!("{} {month}", then.day),
        _ => format!("{} {month} {}", then.day, then.year),
    }
}

fn now_moment() -> Option<Moment> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    local_moment(i64::try_from(seconds).ok()?)
}

impl qobject::ClipboardView {
    fn row_value(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = usize::try_from(index.row()).ok().and_then(|at| self.rust().rows.get(at))
        else {
            return QVariant::default();
        };
        match role - ROLE_BASE {
            // SQLite row ids start at one, so the narrowing is safe here.
            0 => QVariant::from(&(row.id as i32)),
            1 => QVariant::from(&QString::from(&row.preview)),
            2 => QVariant::from(&QString::from(row.kind_name)),
            3 => QVariant::from(&row.pinned),
            4 => QVariant::from(&row.copied_at),
            5 => QVariant::from(&QString::from(&row.copied_at_text)),
            6 if row.thumbnail.is_empty() => QVariant::from(&QUrl::default()),
            6 => QVariant::from(&QUrl::from_local_file(&QString::from(&row.thumbnail))),
            7 => QVariant::from(&row.thumbnail_width),
            8 => QVariant::from(&row.thumbnail_height),
            9 => QVariant::from(&self.rust().selection.iter().any(|(id, _)| *id == row.id)),
            _ => QVariant::default(),
        }
    }

    fn row_count(&self, parent: &QModelIndex) -> i32 {
        if parent.is_valid() { 0 } else { self.rust().rows.len() as i32 }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        for (at, name) in ROLE_NAMES.iter().enumerate() {
            roles.insert(ROLE_BASE + at as i32, QByteArray::from(*name));
        }
        roles
    }

    fn can_fetch_more(&self, parent: &QModelIndex) -> bool {
        let rust = self.rust();
        !parent.is_valid() && !rust.asking && further(rust.window, rust.total as usize).is_some()
    }

    fn fetch_more(self: Pin<&mut Self>, parent: &QModelIndex) {
        if !parent.is_valid() {
            self.show_more();
        }
    }

    fn show_more(self: Pin<&mut Self>) {
        let wanted = further(self.rust().window, self.rust().total as usize);
        self.ask_for(wanted);
    }

    fn show_earlier(self: Pin<&mut Self>) {
        let wanted = earlier(self.rust().window);
        self.ask_for(wanted);
    }

    fn ask_for(mut self: Pin<&mut Self>, window: Option<Window>) {
        let Some(window) = window else { return };
        if self.rust().asking {
            return;
        }
        if clipboard_state::send(Command::Page(window)) {
            self.as_mut().rust_mut().asking = true;
        }
    }

    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Ok(mut view) = VIEW.lock() {
            *view = Some(thread);
        }
        // Counting copies Plasma's database, so it is asked once.
        let waiting = clipboard_history::klipper::waiting() as i32;
        self.as_mut().set_klipper_waiting(waiting);
        let mut choices = QList::<i32>::default();
        for days in settings::CLIPBOARD_KEEP_DAYS_CHOICES {
            choices.append(days as i32);
        }
        self.as_mut().set_keep_days_choices(choices);
        self.apply(clipboard_state::get());
    }

    fn enable(mut self: Pin<&mut Self>, on: bool) {
        settings::put_bool(settings::CLIPBOARD_ENABLED, on);
        self.as_mut().set_enabled(on);
    }

    fn search(mut self: Pin<&mut Self>, text: &QString) {
        let wanted = text.to_string();
        self.as_mut().set_query(QString::from(&wanted));
        clipboard_state::search(wanted);
    }

    fn put_back(self: Pin<&mut Self>, id: i32) {
        let sent = clipboard_state::send(Command::PutBack(id as i64));
        self.announce(if sent { "Copied" } else { NOT_RUNNING });
    }

    fn pin(self: Pin<&mut Self>, id: i32, pinned: bool) {
        clipboard_state::send(Command::Pin { id: id as i64, pinned });
    }

    fn move_towards_top(self: Pin<&mut Self>, id: i32, towards_top: bool) {
        clipboard_state::send(Command::Move { id: id as i64, towards_top });
    }

    fn forget(self: Pin<&mut Self>, id: i32) {
        clipboard_state::send(Command::Forget(id as i64));
    }

    fn clear_unpinned(self: Pin<&mut Self>) {
        clipboard_state::send(Command::ClearUnpinned);
    }

    /// The history thread can take up to two seconds to answer, so the read runs on a thread of
    /// its own and the answer arrives through the properties. An answer for an entry nobody is
    /// looking at any more is dropped.
    fn read_full(mut self: Pin<&mut Self>, id: i32) {
        self.as_mut().set_full_text_id(id);
        self.as_mut().set_full_text(QString::default());
        self.as_mut().set_full_text_ready(false);
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let found =
                clipboard_state::read(id as i64).map(|entry| clipboard_state::full_text(&entry));
            let _ = thread.queue(move |mut view| {
                if *view.full_text_id() != id {
                    return;
                }
                match found {
                    Some(text) => view.as_mut().set_full_text(QString::from(&text)),
                    None => view.as_mut().announce("That entry is no longer in the history."),
                }
                view.as_mut().set_full_text_ready(true);
            });
        });
    }

    /// Answers with the reason it was refused, or an empty string once the edit is on its way.
    fn rewrite(mut self: Pin<&mut Self>, id: i32, text: &QString) -> QString {
        let text = text.to_string();
        let kind = self.rust().rows.iter().find(|row| row.id == id as i64).map(|row| row.kind);
        if let Some(refusal) = rewrite_refusal(kind, &text) {
            return QString::from(refusal);
        }
        if !clipboard_state::send(Command::Rewrite { id: id as i64, text: text.clone() }) {
            return QString::from(NOT_RUNNING);
        }
        if *self.full_text_id() == id {
            self.as_mut().set_full_text(QString::from(text.trim()));
        }
        self.announce("Edited");
        QString::default()
    }

    fn toggle_selected(mut self: Pin<&mut Self>, id: i32) {
        let id = id as i64;
        let Some(kind) = self.rust().rows.iter().find(|row| row.id == id).map(|row| row.kind)
        else {
            return;
        };
        {
            let mut rust = self.as_mut().rust_mut();
            match rust.selection.iter().position(|(picked, _)| *picked == id) {
                Some(at) => {
                    rust.selection.remove(at);
                }
                None => rust.selection.push((id, kind)),
            }
        }
        let count = self.rust().selection.len() as i32;
        self.as_mut().set_selected_count(count);
        self.selection_changed(&[id]);
    }

    fn clear_selection(mut self: Pin<&mut Self>) {
        let dropped: Vec<i64> = std::mem::take(&mut self.as_mut().rust_mut().selection)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        self.as_mut().set_selected_count(0);
        self.selection_changed(&dropped);
    }

    fn paste_selection(mut self: Pin<&mut Self>) {
        let (ids, sentence) = joined(&self.rust().selection);
        if ids.is_empty() {
            self.announce(&sentence);
            return;
        }
        if !clipboard_state::send(Command::PutBackJoined { ids, separator: "\n".into() }) {
            self.announce(NOT_RUNNING);
            return;
        }
        self.as_mut().announce(&sentence);
        self.clear_selection();
    }

    fn selection_changed(mut self: Pin<&mut Self>, ids: &[i64]) {
        let root = QModelIndex::default();
        let mut roles = QVector::<i32>::default();
        roles.append(SELECTED_ROLE);
        let positions: Vec<i32> = self
            .rust()
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| ids.contains(&row.id))
            .map(|(at, _)| at as i32)
            .collect();
        for at in positions {
            let index = self.index(at, 0, &root);
            self.as_mut().rows_changed(&index, &index, &roles);
        }
    }

    fn choose_keep_days(mut self: Pin<&mut Self>, days: i32) -> bool {
        let Some(days) = keep_days_choice(days) else {
            return false;
        };
        let saved = settings::put_integer(settings::CLIPBOARD_KEEP_DAYS, days);
        if saved {
            self.as_mut().set_keep_days(days as i32);
        }
        saved
    }

    /// Shown for a moment and then cleared, for an action whose result is otherwise invisible:
    /// the panel closes on a pick, and the clipboard does not show what it holds.
    fn announce(mut self: Pin<&mut Self>, text: &str) {
        let raised = NOTICE.fetch_add(1, Ordering::AcqRel) + 1;
        self.as_mut().set_notice(QString::from(text));
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            std::thread::sleep(NOTICE_LASTS);
            let _ = thread.queue(move |view| {
                if NOTICE.load(Ordering::Acquire) == raised {
                    view.set_notice(QString::default());
                }
            });
        });
    }

    fn choose_limit(mut self: Pin<&mut Self>, rows: i32) {
        settings::put_integer(settings::CLIPBOARD_LIMIT, rows as i64);
        self.as_mut().set_limit(rows);
    }

    fn choose_images_and_files(mut self: Pin<&mut Self>, on: bool) {
        settings::put_bool(settings::CLIPBOARD_IMAGES_AND_FILES, on);
        self.as_mut().set_images_and_files(on);
    }

    fn choose_skip_sensitive(mut self: Pin<&mut Self>, on: bool) {
        settings::put_bool(settings::CLIPBOARD_SKIP_SENSITIVE, on);
        self.as_mut().set_skip_sensitive(on);
    }

    fn choose_clear_after(mut self: Pin<&mut Self>, seconds: i32) {
        settings::put_integer(settings::CLEAR_AFTER_SECONDS, seconds as i64);
        self.as_mut().set_clear_after(seconds);
    }

    fn choose_clear_on_suspend(mut self: Pin<&mut Self>, on: bool) {
        settings::put_bool(settings::CLEAR_ON_SUSPEND, on);
        self.as_mut().set_clear_on_suspend(on);
    }

    fn choose_clear_on_screen_lock(mut self: Pin<&mut Self>, on: bool) {
        settings::put_bool(settings::CLEAR_ON_SCREEN_LOCK, on);
        self.as_mut().set_clear_on_screen_lock(on);
    }

    fn choose_clean_links(mut self: Pin<&mut Self>, on: bool) {
        settings::put_bool(settings::CLEAN_LINKS, on);
        self.as_mut().set_clean_links(on);
    }

    /// The rules live in settings and reach the clipboard thread the way every other clipboard
    /// setting does, on its next poll; nothing here talks to that thread directly.
    fn toggle_clean_rule(mut self: Pin<&mut Self>, index: i32, enabled: bool) {
        let mut rules = clipboard_state::clean_rules();
        let Some(rule) =
            usize::try_from(index).ok().and_then(|at| rules.catalogue().get(at).cloned())
        else {
            return;
        };
        rules.set_enabled(&rule.scope, &rule.parameter, enabled);
        settings::put_texts(settings::CLEAN_URL_DISABLED_RULES, &rules.disabled);
        self.as_mut().set_clean_rule_notice(QString::default());
        self.as_mut().apply_clean_rules();
    }

    fn add_clean_rule(mut self: Pin<&mut Self>, domain: &QString, parameter: &QString) -> bool {
        let mut rules = clipboard_state::clean_rules();
        let added = match rules.add(&domain.to_string(), &parameter.to_string()) {
            Ok(()) => {
                settings::put_texts(settings::CLEAN_URL_ADDED_RULES, &rules.added_entries());
                self.as_mut().set_clean_rule_notice(QString::default());
                true
            }
            Err(error) => {
                self.as_mut().set_clean_rule_notice(QString::from(&error.to_string()));
                false
            }
        };
        self.as_mut().apply_clean_rules();
        added
    }

    fn remove_clean_rule(mut self: Pin<&mut Self>, index: i32) {
        let mut rules = clipboard_state::clean_rules();
        let Some(rule) =
            usize::try_from(index).ok().and_then(|at| rules.catalogue().get(at).cloned())
        else {
            return;
        };
        if !rule.custom {
            return;
        }
        rules.remove(&rule.scope, &rule.parameter);
        settings::put_texts(settings::CLEAN_URL_ADDED_RULES, &rules.added_entries());
        settings::put_texts(settings::CLEAN_URL_DISABLED_RULES, &rules.disabled);
        self.as_mut().set_clean_rule_notice(QString::default());
        self.as_mut().apply_clean_rules();
    }

    fn adopt_klipper_history(mut self: Pin<&mut Self>) {
        clipboard_state::send(Command::AdoptKlipperHistory);
        self.as_mut().set_klipper_waiting(0);
    }

    /// Numbered the way the buttons are laid out, left to right. Only previews: the
    /// clipboard is written by use_transform, never by looking.
    fn transform(self: Pin<&mut Self>, wanted: i32) {
        let wanted = match wanted {
            0 => Transformation::Plain,
            1 => Transformation::Json,
            2 => Transformation::Markdown,
            _ => return,
        };
        clipboard_state::send(Command::PreviewTransform(wanted));
    }

    fn use_transform(self: Pin<&mut Self>) {
        clipboard_state::send(Command::ApplyTransform);
    }

    fn discard_transform(self: Pin<&mut Self>) {
        clipboard_state::send(Command::DiscardTransform);
    }

    fn apply_rows(mut self: Pin<&mut Self>, snapshot: &Snapshot) {
        if let Some(back) = step_back(snapshot.window, snapshot.rows.len(), snapshot.total) {
            // The rows in hand stay up until the window that has rows arrives.
            self.as_mut().rust_mut().asking = false;
            self.ask_for(Some(back));
            return;
        }

        let now = now_moment();
        let new: Vec<Row> =
            snapshot.rows.iter().map(|summary| Row::from_summary(summary, now.as_ref())).collect();

        if self.rust().shown_query != snapshot.query {
            // A different query is a different list; there is no place in it to keep.
            unsafe { self.as_mut().begin_reset_model() };
            {
                let mut rust = self.as_mut().rust_mut();
                rust.rows = new;
                rust.shown_query = snapshot.query.clone();
            }
            unsafe { self.as_mut().end_reset_model() };
        } else {
            // A row gone from the same window was forgotten or expired, not scrolled away.
            if self.rust().window == snapshot.window {
                let kept: HashSet<i64> = new.iter().map(|row| row.id).collect();
                let gone: HashSet<i64> = self
                    .rust()
                    .rows
                    .iter()
                    .map(|row| row.id)
                    .filter(|id| !kept.contains(id))
                    .collect();
                self.as_mut().rust_mut().selection.retain(|(id, _)| !gone.contains(id));
                let count = self.rust().selection.len() as i32;
                self.as_mut().set_selected_count(count);
            }
            for edit in edits(&self.rust().rows, &new) {
                self.as_mut().replay_announced(&new, &edit);
            }
        }

        let total = snapshot.total;
        {
            let mut rust = self.as_mut().rust_mut();
            rust.window = snapshot.window;
            rust.asking = false;
        }
        self.as_mut().set_total(total as i32);
        self.as_mut().set_offset(snapshot.window.offset as i32);
        self.as_mut().set_has_more(further(snapshot.window, total).is_some());
        self.as_mut().set_has_earlier(snapshot.window.offset > 0);
    }

    fn replay_announced(mut self: Pin<&mut Self>, new: &[Row], edit: &Edit) {
        let root = QModelIndex::default();
        match *edit {
            Edit::Remove { at, count } => {
                let (first, last) = (at as i32, (at + count - 1) as i32);
                unsafe { self.as_mut().begin_remove_rows(&root, first, last) };
                replay(&mut self.as_mut().rust_mut().rows, new, edit);
                unsafe { self.as_mut().end_remove_rows() };
            }
            Edit::Insert { at, count } => {
                let (first, last) = (at as i32, (at + count - 1) as i32);
                unsafe { self.as_mut().begin_insert_rows(&root, first, last) };
                replay(&mut self.as_mut().rust_mut().rows, new, edit);
                unsafe { self.as_mut().end_insert_rows() };
            }
            Edit::Change { at } => {
                replay(&mut self.as_mut().rust_mut().rows, new, edit);
                let index = self.index(at as i32, 0, &root);
                self.as_mut().rows_changed(&index, &index, &QVector::<i32>::default());
            }
        }
    }

    fn apply(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        self.as_mut().apply_rows(&snapshot);
        self.as_mut().set_transform_notice(QString::from(&snapshot.notice));
        self.as_mut().set_clean_notice(QString::from(&match snapshot.cleaned.as_slice() {
            [] => String::new(),
            [one] => format!("Took {one} out of the last link"),
            [most @ .., last] => {
                format!("Took {} and {last} out of the last link", most.join(", "))
            }
        }));
        self.as_mut().set_transform_preview(QString::from(&snapshot.preview));
        self.as_mut().set_can_transform(snapshot.can_transform);
        self.as_mut().set_can_markdown(snapshot.can_markdown);
        self.as_mut().set_pinned_count(snapshot.pinned as i32);
        self.as_mut().set_recent_count(snapshot.recent as i32);
        self.as_mut().set_available(clipboard_state::is_running());
        self.as_mut().set_enabled(clipboard_state::keeps_history());
        self.as_mut().set_limit(settings::integer_at(
            settings::CLIPBOARD_LIMIT,
            settings::CLIPBOARD_LIMIT_DEFAULT,
        ) as i32);
        self.as_mut().set_keep_days(settings::integer_at(
            settings::CLIPBOARD_KEEP_DAYS,
            settings::CLIPBOARD_KEEP_DAYS_DEFAULT,
        ) as i32);
        self.as_mut().set_images_and_files(settings::bool_at(
            settings::CLIPBOARD_IMAGES_AND_FILES,
            settings::CLIPBOARD_IMAGES_AND_FILES_DEFAULT,
        ));
        self.as_mut().set_skip_sensitive(settings::bool_at(
            settings::CLIPBOARD_SKIP_SENSITIVE,
            settings::CLIPBOARD_SKIP_SENSITIVE_DEFAULT,
        ));
        self.as_mut().set_clear_after(settings::integer_at(
            settings::CLEAR_AFTER_SECONDS,
            settings::CLEAR_AFTER_SECONDS_DEFAULT,
        ) as i32);
        self.as_mut().set_clear_on_suspend(clipboard_state::clears_on_suspend());
        self.as_mut().set_clear_on_screen_lock(clipboard_state::clears_on_screen_lock());
        self.as_mut().set_clean_links(clipboard_state::cleans_links());
        self.as_mut().apply_clean_rules();
    }

    fn apply_clean_rules(mut self: Pin<&mut Self>) {
        let mut scopes = QStringList::default();
        let mut parameters = QStringList::default();
        let mut enabled = QList::<bool>::default();
        let mut custom = QList::<bool>::default();
        for rule in clipboard_state::clean_rules().catalogue() {
            scopes.append(QString::from(if rule.scope.is_empty() {
                "Every site"
            } else {
                &rule.scope
            }));
            parameters.append(QString::from(&rule.parameter));
            enabled.append(rule.enabled);
            custom.append(rule.custom);
        }
        self.as_mut().set_clean_rule_scopes(scopes);
        self.as_mut().set_clean_rule_parameters(parameters);
        self.as_mut().set_clean_rule_enabled(enabled);
        self.as_mut().set_clean_rule_custom(custom);
    }
}

pub fn publish() {
    let Ok(view) = VIEW.lock() else {
        return;
    };
    if let Some(thread) = view.as_ref() {
        let _ = thread.queue(|view| view.apply(clipboard_state::get()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, preview: &str) -> Row {
        Row {
            id,
            kind: Kind::Text,
            kind_name: "text",
            preview: preview.into(),
            pinned: false,
            copied_at: 0,
            copied_at_text: String::new(),
            thumbnail: String::new(),
            thumbnail_width: 0,
            thumbnail_height: 0,
        }
    }

    fn rows(ids: &[i64]) -> Vec<Row> {
        ids.iter().map(|id| row(*id, &format!("entry {id}"))).collect()
    }

    /// Replays the steps the way the view does and checks they land on the new list.
    fn replayed(old: &[Row], new: &[Row]) -> Vec<Edit> {
        let steps = edits(old, new);
        let mut shown = old.to_vec();
        for step in &steps {
            replay(&mut shown, new, step);
        }
        assert_eq!(shown, new, "the steps {steps:?} did not arrive at the new list");
        steps
    }

    #[test]
    fn a_new_copy_is_one_insert_at_the_top() {
        let steps = replayed(&rows(&[3, 2, 1]), &rows(&[4, 3, 2, 1]));
        assert_eq!(steps, vec![Edit::Insert { at: 0, count: 1 }]);
    }

    #[test]
    fn a_forgotten_entry_is_one_removal() {
        let steps = replayed(&rows(&[5, 4, 3, 2, 1]), &rows(&[5, 4, 2, 1]));
        assert_eq!(steps, vec![Edit::Remove { at: 2, count: 1 }]);
    }

    #[test]
    fn clearing_runs_together_into_few_removals() {
        let steps = replayed(&rows(&[6, 5, 4, 3, 2, 1]), &rows(&[6, 3]));
        assert_eq!(steps, vec![Edit::Remove { at: 4, count: 2 }, Edit::Remove { at: 1, count: 2 }]);
    }

    #[test]
    fn an_edited_or_pinned_row_in_place_is_a_change() {
        let mut new = rows(&[3, 2, 1]);
        new[1].preview = "fixed".into();
        new[2].pinned = true;
        let steps = replayed(&rows(&[3, 2, 1]), &new);
        assert_eq!(steps, vec![Edit::Change { at: 1 }, Edit::Change { at: 2 }]);
    }

    #[test]
    fn a_row_put_back_moves_to_the_top_without_a_reset() {
        let steps = replayed(&rows(&[4, 3, 2, 1]), &rows(&[2, 4, 3, 1]));
        assert_eq!(steps, vec![Edit::Remove { at: 2, count: 1 }, Edit::Insert { at: 0, count: 1 }]);
    }

    #[test]
    fn nothing_moved_is_nothing_to_say() {
        assert!(replayed(&rows(&[3, 2, 1]), &rows(&[3, 2, 1])).is_empty());
    }

    #[test]
    fn a_window_sliding_down_drops_the_top_and_adds_the_bottom() {
        let old: Vec<i64> = (0..10).collect();
        let new: Vec<i64> = (3..13).collect();
        let steps = replayed(&rows(&old), &rows(&new));
        assert_eq!(steps, vec![Edit::Remove { at: 0, count: 3 }, Edit::Insert { at: 7, count: 3 }]);
    }

    #[test]
    fn any_shuffle_still_arrives() {
        let old = rows(&[1, 2, 3, 4, 5, 6, 7, 8]);
        for new in [
            vec![8, 7, 6, 5, 4, 3, 2, 1],
            vec![9, 1, 10, 3, 11, 5],
            vec![2, 1],
            vec![],
            vec![12, 13],
            vec![5, 6, 7, 8, 1, 2, 3, 4],
        ] {
            replayed(&old, &rows(&new));
        }
        replayed(&[], &rows(&[1, 2, 3]));
    }

    #[test]
    fn scrolling_grows_the_window_and_then_slides_it() {
        let first = Window::default();
        assert_eq!(further(first, 80), None, "everything is already shown");
        assert_eq!(further(first, 3000), Some(Window { offset: 0, limit: 200 }));

        let full = Window { offset: 0, limit: Window::MAX_LIMIT };
        assert_eq!(further(full, 3000), Some(Window { offset: 100, limit: Window::MAX_LIMIT }));
        assert_eq!(
            further(Window { offset: 2450, limit: Window::MAX_LIMIT }, 3000),
            Some(Window { offset: 2500, limit: Window::MAX_LIMIT }),
            "the last slide stops at the end"
        );
        assert_eq!(further(Window { offset: 2500, limit: Window::MAX_LIMIT }, 3000), None);

        assert_eq!(earlier(first), None);
        assert_eq!(
            earlier(Window { offset: 50, limit: 500 }),
            Some(Window { offset: 0, limit: 500 })
        );
    }

    #[test]
    fn a_window_left_past_the_end_steps_back() {
        let past = Window { offset: 300, limit: 100 };
        assert_eq!(step_back(past, 0, 250), Some(Window { offset: 150, limit: 100 }));
        assert_eq!(step_back(past, 0, 40), Some(Window { offset: 0, limit: 100 }));
        assert_eq!(step_back(past, 0, 0), None, "an empty history has nowhere to go back to");
        assert_eq!(step_back(past, 5, 305), None, "rows are showing");
        assert_eq!(step_back(Window::default(), 0, 10), None, "already at the top");
    }

    #[test]
    fn an_edit_is_refused_in_a_sentence() {
        assert_eq!(rewrite_refusal(Some(Kind::Text), "fixed"), None);
        assert!(rewrite_refusal(Some(Kind::Text), "  \n ").is_some());
        assert!(rewrite_refusal(Some(Kind::Text), &"x".repeat(MAX_CHARACTERS + 1)).is_some());
        assert!(rewrite_refusal(Some(Kind::Image), "words").is_some());
        assert!(rewrite_refusal(Some(Kind::Files), "words").is_some());
        assert!(rewrite_refusal(None, "words").is_some());
    }

    #[test]
    fn a_selection_joins_its_text_in_the_order_picked() {
        let picked = [(5, Kind::Text), (2, Kind::Image), (9, Kind::Text)];
        assert_eq!(
            joined(&picked),
            (vec![5, 9], "Pasted 2 entries, without the pictures and files".into())
        );
        assert_eq!(joined(&[(1, Kind::Text)]), (vec![1], "Pasted 1 entry".into()));
        assert_eq!(joined(&[(1, Kind::Files)]).0, Vec::<i64>::new());
        assert_eq!(joined(&[]).1, "Nothing selected is text to join.");
    }

    #[test]
    fn only_the_offered_ages_can_be_chosen() {
        for days in [0, 1, 7, 30, 90] {
            assert_eq!(keep_days_choice(days), Some(i64::from(days)));
        }
        for days in [-1, 2, 14, 365] {
            assert_eq!(keep_days_choice(days), None);
        }
        assert!(
            settings::CLIPBOARD_KEEP_DAYS_CHOICES.contains(&settings::CLIPBOARD_KEEP_DAYS_DEFAULT)
        );
    }

    #[test]
    fn a_row_says_when_in_the_fewest_words() {
        let now = Moment { year: 2026, month: 3, day: 1, hour: 9, minute: 5 };
        let at = |year, month, day| Moment { year, month, day, hour: 14, minute: 7 };
        assert_eq!(copied_at_text(&at(2026, 3, 1), &now), "14:07");
        assert_eq!(copied_at_text(&at(2026, 2, 28), &now), "Yesterday 14:07");
        assert_eq!(copied_at_text(&at(2026, 2, 27), &now), "27 Feb");
        assert_eq!(copied_at_text(&at(2025, 12, 31), &now), "31 Dec 2025");
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2024, 2, 29), 19_782);
    }
}
