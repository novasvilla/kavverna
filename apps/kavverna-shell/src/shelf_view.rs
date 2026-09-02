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
        #[qproperty(bool, strip_on_left)]
        #[qproperty(bool, placed)]
        #[qproperty(QString, shelf_screen)]
        #[qproperty(i32, shelf_left)]
        #[qproperty(i32, shelf_top)]
        #[qproperty(bool, ghost_visible)]
        #[qproperty(QString, ghost_screen)]
        #[qproperty(i32, ghost_left)]
        #[qproperty(i32, ghost_top)]
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
        fn glance_of(self: Pin<&mut ShelfView>, id: i32) -> QString;
        #[qinvokable]
        fn reveal(self: Pin<&mut ShelfView>, id: i32);
        #[qinvokable]
        fn choose_edge_strip(self: Pin<&mut ShelfView>, on: bool);
        #[qinvokable]
        fn choose_keep_across_restarts(self: Pin<&mut ShelfView>, keep: bool);
        #[qinvokable]
        fn choose_remove_after_drop(self: Pin<&mut ShelfView>, remove: bool);
        #[qinvokable]
        fn choose_strip_edge(self: Pin<&mut ShelfView>, left: bool);
        #[qinvokable]
        fn drag_begun(
            self: Pin<&mut ShelfView>,
            pointer_x: i32,
            pointer_y: i32,
            width: i32,
            height: i32,
        );
        #[qinvokable]
        fn drag_preview(
            self: Pin<&mut ShelfView>,
            pointer_x: i32,
            pointer_y: i32,
            width: i32,
            height: i32,
        );
        #[qinvokable]
        fn drag_commit(self: Pin<&mut ShelfView>, width: i32, height: i32);
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
    strip_on_left: bool,
    placed: bool,
    shelf_screen: QString,
    shelf_left: i32,
    shelf_top: i32,
    ghost_visible: bool,
    ghost_screen: QString,
    ghost_left: i32,
    ghost_top: i32,
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
            strip_on_left: settings::bool_at(
                settings::SHELF_STRIP_LEFT,
                settings::SHELF_STRIP_LEFT_DEFAULT,
            ),
            placed: false,
            shelf_screen: QString::default(),
            shelf_left: 0,
            shelf_top: 0,
            ghost_visible: false,
            ghost_screen: QString::default(),
            ghost_left: 0,
            ghost_top: 0,
        }
    }
}

/// The shelf window's width and height cap, restated from the interface's own numbers.
const SHELF_LARGEST: (i32, i32) = (240, 640);

/// Where a shelf drag started from: the surface's global top-left corner and the press in
/// surface coordinates, the panel's own arrangement. One drag at a time, on the Qt thread only.
struct ShelfDragStart {
    origin: (i32, i32),
    pressed: (i32, i32),
}

static SHELF_DRAG_FROM: Mutex<Option<ShelfDragStart>> = Mutex::new(None);

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

        // The shelf reopens where it was left, while that screen is still connected. The
        // stored point was clamped when it was committed; a later resolution change is
        // corrected by the clamp of the next drag rather than guessed at here, where the
        // window's size is not yet known.
        let screens = crate::panel::screens();
        let entries = settings::texts_at(settings::SHELF_POSITION).unwrap_or_default();
        match crate::panel_anchor::last_remembered(&entries, &screens) {
            Some(((x, y), screen)) => {
                self.as_mut().set_shelf_left(x);
                self.as_mut().set_shelf_top(y);
                self.as_mut().set_shelf_screen(QString::from(&screen.name));
                self.as_mut().set_placed(true);
            }
            None => {
                self.as_mut().set_shelf_screen(QString::from(
                    screens.first().map(|screen| screen.name.as_str()).unwrap_or(""),
                ));
                self.as_mut().set_placed(false);
            }
        }

        self.as_mut().reconcile_screens();
        self.apply();
    }

    fn choose_strip_edge(mut self: Pin<&mut Self>, left: bool) {
        settings::put_bool(settings::SHELF_STRIP_LEFT, left);
        self.as_mut().set_strip_on_left(left);
    }

    /// The same ghost gesture the panel uses, for the same reason: the surface cannot move
    /// under the pointer without poisoning the pointer's own readings.
    fn drag_begun(
        mut self: Pin<&mut Self>,
        pointer_x: i32,
        pointer_y: i32,
        width: i32,
        height: i32,
    ) {
        let screens = crate::panel::screens();
        let name = self.shelf_screen().to_string();
        let Some(screen) =
            screens.iter().find(|screen| screen.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let worked = crate::panel::worked(screen);
        let from = if *self.placed() {
            (*self.shelf_left(), *self.shelf_top())
        } else {
            let gap = crate::panel::gap();
            let x = if *self.strip_on_left() { gap } else { worked.width - width - gap };
            (x, (worked.height - height) / 2)
        };
        if let Ok(mut held) = SHELF_DRAG_FROM.lock() {
            *held = Some(ShelfDragStart {
                origin: (screen.x + from.0, screen.y + from.1),
                pressed: (pointer_x, pointer_y),
            });
        }
        self.as_mut().set_ghost_screen(QString::from(&screen.name));
        self.as_mut().set_ghost_left(from.0);
        self.as_mut().set_ghost_top(from.1);
    }

    fn drag_preview(
        mut self: Pin<&mut Self>,
        pointer_x: i32,
        pointer_y: i32,
        width: i32,
        height: i32,
    ) {
        let Some((origin, pressed)) = SHELF_DRAG_FROM
            .lock()
            .ok()
            .and_then(|held| held.as_ref().map(|drag| (drag.origin, drag.pressed)))
        else {
            return;
        };
        if !*self.ghost_visible()
            && (pointer_x - pressed.0).abs() + (pointer_y - pressed.1).abs() < 8
        {
            return;
        }
        let screens = crate::panel::screens();
        let name = self.ghost_screen().to_string();
        let Some(screen) =
            screens.iter().find(|screen| screen.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let local =
            crate::panel_anchor::dragged_local(origin, pressed, (pointer_x, pointer_y), screen);
        let at = crate::panel_anchor::pinned(
            local,
            &crate::panel::worked(screen),
            (width, height),
            crate::panel::gap(),
        );
        self.as_mut().set_ghost_left(at.left);
        self.as_mut().set_ghost_top(at.top);
        self.as_mut().set_ghost_visible(true);
    }

    fn drag_commit(mut self: Pin<&mut Self>, width: i32, height: i32) {
        let began = SHELF_DRAG_FROM.lock().ok().and_then(|mut held| held.take());
        if began.is_none() || !*self.ghost_visible() {
            self.as_mut().set_ghost_visible(false);
            return;
        }
        self.as_mut().set_ghost_visible(false);

        let screens = crate::panel::screens();
        let name = self.ghost_screen().to_string();
        let Some(screen) =
            screens.iter().find(|screen| screen.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let landed = crate::panel_anchor::pinned(
            (*self.ghost_left(), *self.ghost_top()),
            &crate::panel::worked(screen),
            (width, height),
            crate::panel::gap(),
        );
        self.as_mut().set_shelf_left(landed.left);
        self.as_mut().set_shelf_top(landed.top);
        self.as_mut().set_shelf_screen(QString::from(&screen.name));
        self.as_mut().set_placed(true);

        // A shelf stays where it was put; there is no mode to ask first.
        let entries = settings::texts_at(settings::SHELF_POSITION).unwrap_or_default();
        let name = screen.name.clone();
        settings::put_texts(
            settings::SHELF_POSITION,
            &crate::panel_anchor::with_position(entries, &name, landed.left, landed.top),
        );
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

    fn glance_of(self: Pin<&mut Self>, id: i32) -> QString {
        let Ok(id) = u64::try_from(id) else {
            return QString::default();
        };
        shelf_state::with(|shelf| shelf.item(id).map(|item| QString::from(&item.glance())))
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

    /// A shelf whose screen left moves to the first one still here, pinned by its largest
    /// size so it stays on that screen however much it later grows. The spot remembered for
    /// the screen that left is kept for its return.
    fn reconcile_screens(mut self: Pin<&mut Self>) {
        let screens = crate::panel::screens();
        let current = self.shelf_screen().to_string();
        if !screens.is_empty() && !screens.iter().any(|screen| screen.name == current) {
            let screen = &screens[0];
            self.as_mut().set_shelf_screen(QString::from(&screen.name));
            let landed = crate::panel_anchor::pinned(
                (*self.shelf_left(), *self.shelf_top()),
                &crate::panel::worked(screen),
                SHELF_LARGEST,
                crate::panel::gap(),
            );
            self.as_mut().set_shelf_left(landed.left);
            self.as_mut().set_shelf_top(landed.top);
        }
    }
}

pub fn publish() {
    queue(|view| view.apply());
}

pub fn screens_changed() {
    queue(|view| view.reconcile_screens());
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
