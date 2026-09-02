use crate::{clipboard_state, settings};
use clipboard_history::{Command, Snapshot, Transformation};
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
        self.as_mut().set_enabled(clipboard_state::wanted());
        self.as_mut().set_limit(settings::integer_at(
            settings::CLIPBOARD_LIMIT,
            settings::CLIPBOARD_LIMIT_DEFAULT,
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
