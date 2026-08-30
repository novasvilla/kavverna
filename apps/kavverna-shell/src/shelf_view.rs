use crate::{settings, shelf_state};
use cxx_qt::Threading;
use cxx_qt_lib::{QByteArray, QList, QString, QStringList};
use shelf::{DragPayload, Dropped, ItemKind};
use std::sync::Mutex;

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
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, shelf_open)]
        #[qproperty(QList_i32, row_ids)]
        #[qproperty(QList_i32, row_pile_ids)]
        #[qproperty(QList_i32, row_pile_sizes)]
        #[qproperty(QStringList, row_kinds)]
        #[qproperty(QStringList, row_names)]
        #[qproperty(QStringList, row_details)]
        #[qproperty(QStringList, row_icons)]
        #[qproperty(QStringList, row_thumbs)]
        #[qproperty(QList_bool, row_alive)]
        #[qproperty(i32, item_count)]
        #[qproperty(QString, notice)]
        #[qproperty(QString, drag_uris)]
        #[qproperty(QString, drag_text)]
        #[qproperty(bool, drag_ok)]
        #[qproperty(bool, edge_strip)]
        #[qproperty(bool, keep_across_restarts)]
        #[qproperty(bool, remove_after_drop)]
        type ShelfView = super::ShelfViewRust;
    }

    impl cxx_qt::Threading for ShelfView {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut ShelfView>);
        #[qinvokable]
        fn set_open(self: Pin<&mut ShelfView>, open: bool);
        #[qinvokable]
        fn wanted_format(self: Pin<&mut ShelfView>, formats: &QString) -> QString;
        #[qinvokable]
        fn deposit(
            self: Pin<&mut ShelfView>,
            urls: &QString,
            text: &QString,
            moz_url: &QByteArray,
            image_bytes: &QByteArray,
            image_format: &QString,
        );
        #[qinvokable]
        fn prepare_drag(self: Pin<&mut ShelfView>, ids: &QString);
        #[qinvokable]
        fn taken(self: Pin<&mut ShelfView>, ids: &QString);
        #[qinvokable]
        fn remove(self: Pin<&mut ShelfView>, id: i32);
        #[qinvokable]
        fn clear(self: Pin<&mut ShelfView>);
        #[qinvokable]
        fn open_target(self: Pin<&mut ShelfView>, id: i32) -> QString;
        #[qinvokable]
        fn path_of(self: Pin<&mut ShelfView>, id: i32) -> QString;
        #[qinvokable]
        fn reveal(self: Pin<&mut ShelfView>, id: i32);
        #[qinvokable]
        fn choose_edge_strip(self: Pin<&mut ShelfView>, on: bool);
        #[qinvokable]
        fn choose_keep_across_restarts(self: Pin<&mut ShelfView>, keep: bool);
        #[qinvokable]
        fn choose_remove_after_drop(self: Pin<&mut ShelfView>, remove: bool);
    }
}

use core::pin::Pin;

static VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::ShelfView>>> = Mutex::new(None);

pub struct ShelfViewRust {
    shelf_open: bool,
    row_ids: QList<i32>,
    row_pile_ids: QList<i32>,
    row_pile_sizes: QList<i32>,
    row_kinds: QStringList,
    row_names: QStringList,
    row_details: QStringList,
    row_icons: QStringList,
    row_thumbs: QStringList,
    row_alive: QList<bool>,
    item_count: i32,
    notice: QString,
    drag_uris: QString,
    drag_text: QString,
    drag_ok: bool,
    edge_strip: bool,
    keep_across_restarts: bool,
    remove_after_drop: bool,
}

impl Default for ShelfViewRust {
    fn default() -> Self {
        Self {
            shelf_open: false,
            row_ids: QList::default(),
            row_pile_ids: QList::default(),
            row_pile_sizes: QList::default(),
            row_kinds: QStringList::default(),
            row_names: QStringList::default(),
            row_details: QStringList::default(),
            row_icons: QStringList::default(),
            row_thumbs: QStringList::default(),
            row_alive: QList::default(),
            item_count: 0,
            notice: QString::default(),
            drag_uris: QString::default(),
            drag_text: QString::default(),
            drag_ok: false,
            edge_strip: settings::bool_at(
                settings::SHELF_EDGE_STRIP,
                settings::SHELF_EDGE_STRIP_DEFAULT,
            ),
            keep_across_restarts: settings::bool_at(
                settings::SHELF_KEEP_ACROSS_RESTARTS,
                settings::SHELF_KEEP_ACROSS_RESTARTS_DEFAULT,
            ),
            remove_after_drop: settings::bool_at(
                settings::SHELF_REMOVE_AFTER_DROP,
                settings::SHELF_REMOVE_AFTER_DROP_DEFAULT,
            ),
        }
    }
}

/// "1,7,9" from the interface back into numbers; anything else is dropped rather than guessed.
fn parse_ids(joined: &QString) -> Vec<u64> {
    joined.to_string().split(',').filter_map(|part| part.trim().parse().ok()).collect()
}

impl qobject::ShelfView {
    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Ok(mut view) = VIEW.lock() {
            *view = Some(thread);
        }
        self.apply();
    }

    fn set_open(mut self: Pin<&mut Self>, open: bool) {
        self.as_mut().set_shelf_open(open);
    }

    /// Which raw image flavour the drop should pull bytes for, if any. Decided in Rust so
    /// the preference order lives beside its tests.
    fn wanted_format(self: Pin<&mut Self>, formats: &QString) -> QString {
        let offered: Vec<String> = formats.to_string().lines().map(str::to_owned).collect();
        QString::from(shelf::wanted_image_format(&offered).unwrap_or(""))
    }

    fn deposit(
        mut self: Pin<&mut Self>,
        urls: &QString,
        text: &QString,
        moz_url: &QByteArray,
        image_bytes: &QByteArray,
        image_format: &QString,
    ) {
        let dropped = Dropped {
            urls: urls.to_string().lines().filter(|l| !l.is_empty()).map(str::to_owned).collect(),
            text: text.to_string(),
            moz_url: moz_url.as_slice().to_vec(),
            image_bytes: image_bytes.as_slice().to_vec(),
            image_format: image_format.to_string(),
        };
        shelf_state::with(|shelf| shelf.deposit(&dropped));
        self.as_mut().apply();
    }

    /// Fills the drag properties synchronously: the interface must have the mime data in
    /// hand before it starts the drag, and a drag cannot wait on a channel.
    fn prepare_drag(mut self: Pin<&mut Self>, ids: &QString) {
        let ids = parse_ids(ids);
        let payload = shelf_state::with(|shelf| shelf.payload_for(&ids));
        match payload {
            Some(DragPayload::Mime { uris, text }) => {
                self.as_mut().set_drag_uris(QString::from(&uris.join("\r\n")));
                self.as_mut().set_drag_text(QString::from(&text));
                self.as_mut().set_drag_ok(true);
            }
            Some(DragPayload::AllDead { notice }) => {
                self.as_mut().set_notice(QString::from(&notice));
                self.as_mut().set_drag_ok(false);
            }
            None => self.as_mut().set_drag_ok(false),
        }
    }

    fn taken(mut self: Pin<&mut Self>, ids: &QString) {
        if !settings::bool_at(
            settings::SHELF_REMOVE_AFTER_DROP,
            settings::SHELF_REMOVE_AFTER_DROP_DEFAULT,
        ) {
            return;
        }
        let ids = parse_ids(ids);
        shelf_state::with(|shelf| shelf.taken(&ids));
        self.as_mut().apply();
    }

    fn remove(mut self: Pin<&mut Self>, id: i32) {
        if let Ok(id) = u64::try_from(id) {
            shelf_state::with(|shelf| shelf.remove(id));
        }
        self.as_mut().apply();
    }

    fn clear(mut self: Pin<&mut Self>) {
        shelf_state::with(shelf::Shelf::clear);
        self.as_mut().apply();
    }

    /// What `Qt.openUrlExternally` should be handed: the link itself, or the file as a uri.
    fn open_target(self: Pin<&mut Self>, id: i32) -> QString {
        let Ok(id) = u64::try_from(id) else {
            return QString::default();
        };
        shelf_state::with(|shelf| {
            shelf.item(id).map(|item| match &item.kind {
                ItemKind::Link { url, .. } => QString::from(url),
                _ => item
                    .lives_at()
                    .map(shelf::file_uri)
                    .map(|u| QString::from(&u))
                    .unwrap_or_default(),
            })
        })
        .flatten()
        .unwrap_or_default()
    }

    fn path_of(self: Pin<&mut Self>, id: i32) -> QString {
        let Ok(id) = u64::try_from(id) else {
            return QString::default();
        };
        shelf_state::with(|shelf| {
            shelf.item(id).map(|item| match &item.kind {
                ItemKind::Link { url, .. } => QString::from(url),
                _ => item
                    .lives_at()
                    .map(|path| QString::from(&path.display().to_string()))
                    .unwrap_or_default(),
            })
        })
        .flatten()
        .unwrap_or_default()
    }

    /// The file manager's own D-Bus door, on a thread of its own: activating one can take a
    /// moment and the interface should not wait on it.
    fn reveal(self: Pin<&mut Self>, id: i32) {
        let Ok(id) = u64::try_from(id) else {
            return;
        };
        let uri = shelf_state::with(|shelf| {
            shelf.item(id).and_then(|item| item.lives_at()).map(shelf::file_uri)
        })
        .flatten();
        let Some(uri) = uri else {
            return;
        };

        std::thread::spawn(move || {
            let shown = zbus::blocking::Connection::session().and_then(|bus| {
                bus.call_method(
                    Some("org.freedesktop.FileManager1"),
                    "/org/freedesktop/FileManager1",
                    Some("org.freedesktop.FileManager1"),
                    "ShowItems",
                    &(vec![uri.as_str()], ""),
                )
            });
            if let Err(err) = shown {
                tracing::warn!(%err, "no file manager answered ShowItems");
            }
        });
    }

    fn choose_edge_strip(mut self: Pin<&mut Self>, on: bool) {
        settings::put_bool(settings::SHELF_EDGE_STRIP, on);
        self.as_mut().set_edge_strip(on);
    }

    fn choose_keep_across_restarts(mut self: Pin<&mut Self>, keep: bool) {
        settings::put_bool(settings::SHELF_KEEP_ACROSS_RESTARTS, keep);
        self.as_mut().set_keep_across_restarts(keep);
    }

    fn choose_remove_after_drop(mut self: Pin<&mut Self>, remove: bool) {
        settings::put_bool(settings::SHELF_REMOVE_AFTER_DROP, remove);
        self.as_mut().set_remove_after_drop(remove);
    }

    fn apply(mut self: Pin<&mut Self>) {
        let mut ids = QList::<i32>::default();
        let mut pile_ids = QList::<i32>::default();
        let mut pile_sizes = QList::<i32>::default();
        let mut kinds = QStringList::default();
        let mut names = QStringList::default();
        let mut details = QStringList::default();
        let mut icons = QStringList::default();
        let mut thumbs = QStringList::default();
        let mut alive = QList::<bool>::default();
        let mut count = 0;
        let mut notice = String::new();

        shelf_state::with(|shelf| {
            notice = shelf.notice.clone();
            for pile in shelf.piles() {
                for item in &pile.items {
                    ids.append(item.id as i32);
                    pile_ids.append(pile.id as i32);
                    pile_sizes.append(pile.items.len() as i32);
                    kinds.append(QString::from(match &item.kind {
                        ItemKind::File { .. } => "file",
                        ItemKind::Text { .. } => "text",
                        ItemKind::Link { .. } => "link",
                    }));
                    names.append(QString::from(&item.name()));
                    details.append(QString::from(&item.detail()));
                    icons.append(QString::from(shelf::icon_for(item)));
                    let thumb = match (&item.image, item.lives_at()) {
                        (Some(_), Some(path)) if item.alive() => shelf::file_uri(path),
                        _ => String::new(),
                    };
                    thumbs.append(QString::from(&thumb));
                    alive.append(item.alive());
                    count += 1;
                }
            }
        });

        self.as_mut().set_row_ids(ids);
        self.as_mut().set_row_pile_ids(pile_ids);
        self.as_mut().set_row_pile_sizes(pile_sizes);
        self.as_mut().set_row_kinds(kinds);
        self.as_mut().set_row_names(names);
        self.as_mut().set_row_details(details);
        self.as_mut().set_row_icons(icons);
        self.as_mut().set_row_thumbs(thumbs);
        self.as_mut().set_row_alive(alive);
        self.as_mut().set_item_count(count);
        self.as_mut().set_notice(QString::from(&notice));
    }
}

pub fn publish() {
    queue(|view| view.apply());
}

/// The shortcut and the tray reach the shelf through here, from their own threads.
pub fn toggle() {
    queue(|mut view| {
        let open = *view.shelf_open();
        view.as_mut().set_shelf_open(!open);
    });
}

fn queue(action: impl FnOnce(Pin<&mut qobject::ShelfView>) + Send + 'static) {
    let Ok(view) = VIEW.lock() else {
        return;
    };
    if let Some(thread) = view.as_ref() {
        let _ = thread.queue(action);
    }
}
