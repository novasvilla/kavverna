use crate::{clipboard_state, settings};
use clipboard_history::{Command, Snapshot};
use cxx_qt::Threading;
use cxx_qt_lib::{QList, QString, QStringList};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
        type QList_i64 = cxx_qt_lib::QList<i64>;
        type QList_bool = cxx_qt_lib::QList<bool>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, available)]
        #[qproperty(bool, enabled)]
        #[qproperty(QString, query)]
        #[qproperty(QList_i32, row_ids)]
        #[qproperty(QStringList, row_previews)]
        #[qproperty(QStringList, row_kinds)]
        #[qproperty(QList_bool, row_pinned)]
        #[qproperty(QList_i64, row_times)]
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
        type ClipboardView = super::ClipboardViewRust;
    }

    impl cxx_qt::Threading for ClipboardView {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn enable(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn search(self: Pin<&mut ClipboardView>, text: &QString);
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
        fn choose_limit(self: Pin<&mut ClipboardView>, rows: i32);
        #[qinvokable]
        fn choose_images_and_files(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn choose_skip_sensitive(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn adopt_klipper_history(self: Pin<&mut ClipboardView>);
        #[qinvokable]
        fn choose_clear_after(self: Pin<&mut ClipboardView>, seconds: i32);
        #[qinvokable]
        fn choose_clear_on_suspend(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn choose_clear_on_screen_lock(self: Pin<&mut ClipboardView>, on: bool);
        #[qinvokable]
        fn choose_clean_links(self: Pin<&mut ClipboardView>, on: bool);
    }
}

use core::pin::Pin;

static VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::ClipboardView>>> = Mutex::new(None);

#[derive(Default)]
pub struct ClipboardViewRust {
    available: bool,
    enabled: bool,
    query: QString,
    row_ids: QList<i32>,
    row_previews: QStringList,
    row_kinds: QStringList,
    row_pinned: QList<bool>,
    row_times: QList<i64>,
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
}

impl qobject::ClipboardView {
    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Ok(mut view) = VIEW.lock() {
            *view = Some(thread);
        }
        // Counting copies Plasma's database, so it is asked once.
        let waiting = clipboard_history::klipper::waiting() as i32;
        self.as_mut().set_klipper_waiting(waiting);
        self.apply(clipboard_state::get());
    }

    fn enable(mut self: Pin<&mut Self>, on: bool) {
        settings::put_bool(settings::CLIPBOARD_ENABLED, on);
        self.as_mut().set_enabled(on);
    }

    fn search(mut self: Pin<&mut Self>, text: &QString) {
        let wanted = text.to_string();
        self.as_mut().set_query(QString::from(&wanted));
        clipboard_state::send(Command::Search(wanted));
    }

    fn put_back(self: Pin<&mut Self>, id: i32) {
        clipboard_state::send(Command::PutBack(id as i64));
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

    fn adopt_klipper_history(mut self: Pin<&mut Self>) {
        clipboard_state::send(Command::AdoptKlipperHistory);
        self.as_mut().set_klipper_waiting(0);
    }

    fn apply(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        let mut ids = QList::<i32>::default();
        let mut previews = QStringList::default();
        let mut kinds = QStringList::default();
        let mut pinned = QList::<bool>::default();
        let mut times = QList::<i64>::default();

        for row in &snapshot.rows {
            // SQLite row ids start at one, so the narrowing is safe here.
            ids.append(row.id as i32);
            previews.append(QString::from(&row.preview));
            kinds.append(QString::from(row.kind.as_str()));
            pinned.append(row.pinned);
            times.append(
                row.copied_at
                    .duration_since(UNIX_EPOCH)
                    .map(|since| since.as_millis() as i64)
                    .unwrap_or(0),
            );
        }

        self.as_mut().set_row_ids(ids);
        self.as_mut().set_row_previews(previews);
        self.as_mut().set_row_kinds(kinds);
        self.as_mut().set_row_pinned(pinned);
        self.as_mut().set_row_times(times);
        self.as_mut().set_pinned_count(snapshot.pinned as i32);
        self.as_mut().set_recent_count(snapshot.recent as i32);
        self.as_mut().set_available(clipboard_state::is_running());
        self.as_mut().set_enabled(clipboard_state::wanted());
        self.as_mut().set_limit(
            settings::integer_at(settings::CLIPBOARD_LIMIT, settings::CLIPBOARD_LIMIT_DEFAULT)
                as i32,
        );
        self.as_mut().set_images_and_files(settings::bool_at(
            settings::CLIPBOARD_IMAGES_AND_FILES,
            settings::CLIPBOARD_IMAGES_AND_FILES_DEFAULT,
        ));
        self.as_mut().set_skip_sensitive(settings::bool_at(
            settings::CLIPBOARD_SKIP_SENSITIVE,
            settings::CLIPBOARD_SKIP_SENSITIVE_DEFAULT,
        ));
        self.as_mut().set_clear_after(
            settings::integer_at(
                settings::CLEAR_AFTER_SECONDS,
                settings::CLEAR_AFTER_SECONDS_DEFAULT,
            ) as i32,
        );
        self.as_mut().set_clear_on_suspend(clipboard_state::clears_on_suspend());
        self.as_mut().set_clear_on_screen_lock(clipboard_state::clears_on_screen_lock());
        self.as_mut().set_clean_links(clipboard_state::cleans_links());
    }
}

/// Called from the history thread, which has no access to the Qt event loop.
pub fn publish() {
    let Ok(view) = VIEW.lock() else {
        return;
    };
    if let Some(thread) = view.as_ref() {
        let _ = thread.queue(|view| view.apply(clipboard_state::get()));
    }
}
