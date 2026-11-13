//! The "Apps using …" folds and the ports list, as two list models.
//!
//! Both are real models rather than the parallel string lists the other views hand QML. A fold
//! updates every two seconds, and replacing a list rebuilds every delegate, which would throw
//! away an End confirmation half way through being read and the hover that holds the order
//! still. Here a row keeps its delegate for as long as it is listed: figures arrive as
//! `dataChanged`, and a row that moves is moved, not rebuilt.
//!
//! The contract, as QML sees it:
//!
//! `ProcessView`, one per window, `attach()` once.
//! - `open_fold` (string, assign to open): `""`, `"cpu"`, `"memory"`, `"gpu"` or `"disk"`.
//!   Only that fold is measured, only while the panel shows; hiding the panel sets it back
//!   to `""`.
//! - `card_name` (string, assign): which card the `"gpu"` fold reads, by the name `VitalsView`
//!   lists in `card_names`. Empty means the discrete card.
//! - `expanded` (bool, assign): 15 rows instead of 8. Opening another fold resets it.
//! - `has_more` (bool): there are more rows than 8 to expand to.
//! - `phase`: `"closed"`, `"measuring"`, `"ready"`, `"idle"` or `"unavailable"`.
//! - `status_text`: the sentence for every phase but ready: "Measuring…", "No app is using
//!   the GPU now", and so on.
//! - `summary_text`: what the rows leave of the processor's load, empty elsewhere.
//! - `count`: rows in the model.
//! - `hold_order(bool)`: true while the pointer is inside the fold. The rows keep their places
//!   and only their figures change until it is false again.
//! - `end_application(key)`, `force_end_application(key)`: with the `key` role of a row.
//! - Roles: `key`, `name`, `icon`, `value`, `secondary`, `pid`, `processes`, `present`,
//!   `feedback`, `feedback_kind`.
//!
//! `PortsView`, `attach()` once, `refresh()` whenever the list is shown or asked to.
//! - `refreshing` (bool), `status_text`, `count`.
//! - `end_port(key)`, `force_end_port(key)`.
//! - Roles: `key`, `port`, `transport`, `address`, `exposure`, `exposure_text`, `name`, `icon`,
//!   `pid`, `url`, `endable`, `confirm_text`, `open`, `feedback`, `feedback_kind`.

use crate::process_state::{self, PortRow, Resource, Row};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QList, QModelIndex, QString, QVariant,
};
use std::sync::Mutex;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(QString, open_fold)]
        #[qproperty(QString, card_name)]
        #[qproperty(bool, expanded)]
        #[qproperty(bool, has_more)]
        #[qproperty(QString, phase)]
        #[qproperty(QString, status_text)]
        #[qproperty(QString, summary_text)]
        #[qproperty(i32, count)]
        type ProcessView = super::ProcessViewRust;

        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(bool, refreshing)]
        #[qproperty(QString, status_text)]
        #[qproperty(i32, count)]
        type PortsView = super::PortsViewRust;
    }

    impl cxx_qt::Threading for ProcessView {}
    impl cxx_qt::Threading for PortsView {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut ProcessView>);
        #[qinvokable]
        fn hold_order(self: Pin<&mut ProcessView>, holding: bool);
        #[qinvokable]
        fn end_application(self: Pin<&mut ProcessView>, key: &QString);
        #[qinvokable]
        fn force_end_application(self: Pin<&mut ProcessView>, key: &QString);

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn role_value(self: &ProcessView, index: &QModelIndex, role: i32) -> QVariant;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &ProcessView, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &ProcessView) -> QHash_i32_QByteArray;

        #[inherit]
        fn index(self: &ProcessView, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(
            self: Pin<&mut ProcessView>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut ProcessView>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        fn begin_remove_rows(
            self: Pin<&mut ProcessView>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        fn end_remove_rows(self: Pin<&mut ProcessView>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        fn begin_move_rows(
            self: Pin<&mut ProcessView>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        fn end_move_rows(self: Pin<&mut ProcessView>);
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut ProcessView>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut PortsView>);
        #[qinvokable]
        fn refresh(self: Pin<&mut PortsView>);
        #[qinvokable]
        fn end_port(self: Pin<&mut PortsView>, key: &QString);
        #[qinvokable]
        fn force_end_port(self: Pin<&mut PortsView>, key: &QString);

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn role_value(self: &PortsView, index: &QModelIndex, role: i32) -> QVariant;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &PortsView, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &PortsView) -> QHash_i32_QByteArray;

        #[inherit]
        fn index(self: &PortsView, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        fn begin_insert_rows(
            self: Pin<&mut PortsView>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        fn end_insert_rows(self: Pin<&mut PortsView>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        fn begin_remove_rows(
            self: Pin<&mut PortsView>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        fn end_remove_rows(self: Pin<&mut PortsView>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        fn begin_move_rows(
            self: Pin<&mut PortsView>,
            source_parent: &QModelIndex,
            source_first: i32,
            source_last: i32,
            destination_parent: &QModelIndex,
            destination_child: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        fn end_move_rows(self: Pin<&mut PortsView>);
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut PortsView>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }
}

use core::pin::Pin;

static VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::ProcessView>>> = Mutex::new(None);
static PORTS_VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::PortsView>>> = Mutex::new(None);

/// Qt's first role free for a model's own.
const USER_ROLE: i32 = 256;

const ROW_ROLES: [&str; 10] = [
    "key",
    "name",
    "icon",
    "value",
    "secondary",
    "pid",
    "processes",
    "present",
    "feedback",
    "feedback_kind",
];

const PORT_ROLES: [&str; 15] = [
    "key",
    "port",
    "transport",
    "address",
    "exposure",
    "exposure_text",
    "name",
    "icon",
    "pid",
    "url",
    "endable",
    "confirm_text",
    "open",
    "feedback",
    "feedback_kind",
];

#[derive(Default)]
pub struct ProcessViewRust {
    open_fold: QString,
    card_name: QString,
    expanded: bool,
    has_more: bool,
    phase: QString,
    status_text: QString,
    summary_text: QString,
    count: i32,
    rows: Vec<Row>,
}

#[derive(Default)]
pub struct PortsViewRust {
    refreshing: bool,
    status_text: QString,
    count: i32,
    rows: Vec<PortRow>,
}

/// One step that turns a list of keys into another, in the order the model must be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    Remove(usize),
    Insert(usize),
    /// Always upward, so `to` is also where Qt's destination row goes.
    Move {
        from: usize,
        to: usize,
    },
}

/// The fewest plain steps from `old` to `new` that keep every surviving row's delegate:
/// removals bottom up, then each place filled in turn by moving the row up to it or inserting.
pub fn edits(old: &[String], new: &[String]) -> Vec<Edit> {
    let mut steps = Vec::new();
    let mut current = old.to_vec();
    for at in (0..current.len()).rev() {
        if !new.contains(&current[at]) {
            steps.push(Edit::Remove(at));
            current.remove(at);
        }
    }
    for (at, key) in new.iter().enumerate() {
        if current.get(at) == Some(key) {
            continue;
        }
        match current.iter().position(|held| held == key) {
            Some(from) => {
                steps.push(Edit::Move { from, to: at });
                let moved = current.remove(from);
                current.insert(at, moved);
            }
            None => {
                steps.push(Edit::Insert(at));
                current.insert(at, key.clone());
            }
        }
    }
    steps
}

fn text(value: &str) -> QVariant {
    QVariant::from(&QString::from(value))
}

fn number(value: impl TryInto<i32>) -> QVariant {
    QVariant::from(&value.try_into().unwrap_or(i32::MAX))
}

fn roles(names: &[&str]) -> QHash<QHashPair_i32_QByteArray> {
    let mut hash = QHash::<QHashPair_i32_QByteArray>::default();
    for (at, name) in names.iter().enumerate() {
        hash.insert(USER_ROLE + at as i32, QByteArray::from(*name));
    }
    hash
}

fn feedback_parts(feedback: &Option<process_state::Feedback>) -> (&str, &str) {
    feedback.as_ref().map_or(("", ""), |said| (said.text.as_str(), said.kind.name()))
}

/// What both models need from Qt to apply a list of edits to their rows.
trait Model {
    type Item: Clone;

    fn key(item: &Self::Item) -> &str;
    fn items(self: Pin<&mut Self>) -> &mut Vec<Self::Item>;
    fn insert_rows(self: Pin<&mut Self>, first: i32, last: i32, begin: bool);
    fn remove_rows(self: Pin<&mut Self>, first: i32, last: i32, begin: bool);
    fn move_row(self: Pin<&mut Self>, from: i32, to: i32, begin: bool);
    fn changed(self: Pin<&mut Self>, first: i32, last: i32);
    fn publish_count(self: Pin<&mut Self>, count: i32);
}

fn sync<M: Model>(mut model: Pin<&mut M>, next: Vec<M::Item>) {
    let old: Vec<String> =
        model.as_mut().items().iter().map(|item| M::key(item).to_owned()).collect();
    let new: Vec<String> = next.iter().map(|item| M::key(item).to_owned()).collect();
    for step in edits(&old, &new) {
        match step {
            Edit::Remove(at) => {
                model.as_mut().remove_rows(at as i32, at as i32, true);
                model.as_mut().items().remove(at);
                model.as_mut().remove_rows(at as i32, at as i32, false);
            }
            Edit::Insert(at) => {
                model.as_mut().insert_rows(at as i32, at as i32, true);
                model.as_mut().items().insert(at, next[at].clone());
                model.as_mut().insert_rows(at as i32, at as i32, false);
            }
            Edit::Move { from, to } => {
                model.as_mut().move_row(from as i32, to as i32, true);
                let moved = model.as_mut().items().remove(from);
                model.as_mut().items().insert(to, moved);
                model.as_mut().move_row(from as i32, to as i32, false);
            }
        }
    }
    let count = next.len();
    *model.as_mut().items() = next;
    if count > 0 {
        model.as_mut().changed(0, count as i32 - 1);
    }
    model.publish_count(count as i32);
}

macro_rules! list_model {
    ($view:ty, $item:ty) => {
        impl Model for $view {
            type Item = $item;

            fn key(item: &$item) -> &str {
                &item.key
            }

            fn items(self: Pin<&mut Self>) -> &mut Vec<$item> {
                &mut self.rust_mut().get_mut().rows
            }

            fn insert_rows(self: Pin<&mut Self>, first: i32, last: i32, begin: bool) {
                if begin {
                    self.begin_insert_rows(&QModelIndex::default(), first, last);
                } else {
                    self.end_insert_rows();
                }
            }

            fn remove_rows(self: Pin<&mut Self>, first: i32, last: i32, begin: bool) {
                if begin {
                    self.begin_remove_rows(&QModelIndex::default(), first, last);
                } else {
                    self.end_remove_rows();
                }
            }

            fn move_row(self: Pin<&mut Self>, from: i32, to: i32, begin: bool) {
                if begin {
                    let root = QModelIndex::default();
                    self.begin_move_rows(&root, from, from, &root, to);
                } else {
                    self.end_move_rows();
                }
            }

            fn changed(self: Pin<&mut Self>, first: i32, last: i32) {
                let root = QModelIndex::default();
                let top = self.index(first, 0, &root);
                let bottom = self.index(last, 0, &root);
                self.data_changed(&top, &bottom, &QList::<i32>::default());
            }

            fn publish_count(self: Pin<&mut Self>, count: i32) {
                if *self.count() != count {
                    self.set_count(count);
                }
            }
        }
    };
}

list_model!(qobject::ProcessView, Row);
list_model!(qobject::PortsView, PortRow);

impl qobject::ProcessView {
    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Ok(mut view) = VIEW.lock() {
            *view = Some(thread);
        }
        // The worker learns of the fold before anything else reacts to the change: an apply
        // run first would find a fold open that nothing has asked for and close it again.
        self.as_mut()
            .on_open_fold_changed(|mut view| {
                view.as_mut().ask();
                view.as_mut().set_expanded(false);
            })
            .release();
        self.as_mut().on_card_name_changed(|view| view.ask()).release();
        self.as_mut().on_expanded_changed(|view| view.apply(process_state::get())).release();
        self.apply(process_state::get());
    }

    /// Hands the worker what QML asked for, and shows that fold as measuring until the
    /// worker's first reading of it arrives.
    fn ask(self: Pin<&mut Self>) {
        let resource = Resource::from_name(&self.open_fold().to_string());
        process_state::want(resource, &self.card_name().to_string());
        self.apply(process_state::get());
    }

    fn hold_order(self: Pin<&mut Self>, holding: bool) {
        process_state::hold(holding);
    }

    fn end_application(self: Pin<&mut Self>, key: &QString) {
        process_state::end(&key.to_string(), false);
        self.apply(process_state::get());
    }

    fn force_end_application(self: Pin<&mut Self>, key: &QString) {
        process_state::end(&key.to_string(), true);
        self.apply(process_state::get());
    }

    fn apply(mut self: Pin<&mut Self>, shown: process_state::Shown) {
        // The worker closes the fold when the panel hides; everything else about which fold is
        // open is QML's to say.
        if process_state::wanted_resource().is_none() && !self.open_fold().is_empty() {
            self.as_mut().set_open_fold(QString::default());
        }
        let asked = Resource::from_name(&self.open_fold().to_string());
        let (phase, status, summary, rows) = match asked {
            None => (process_state::Phase::Closed, String::new(), String::new(), Vec::new()),
            Some(resource)
                if shown.resource != Some(resource)
                    || (resource == Resource::Graphics
                        && shown.card != self.card_name().to_string()) =>
            {
                let phase = process_state::Phase::Measuring;
                (phase, process_state::status_text(resource, phase), String::new(), Vec::new())
            }
            Some(_) => (shown.phase, shown.status_text, shown.summary_text, shown.rows),
        };
        let shown_rows =
            if *self.expanded() { process_state::SHOWN_EXPANDED } else { process_state::SHOWN };
        let has_more = rows.len() > process_state::SHOWN;
        self.as_mut().set_phase(QString::from(phase.name()));
        self.as_mut().set_status_text(QString::from(&status));
        self.as_mut().set_summary_text(QString::from(&summary));
        self.as_mut().set_has_more(has_more);
        sync(self, rows.into_iter().take(shown_rows).collect());
    }

    fn role_value(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = usize::try_from(index.row()).ok().and_then(|at| self.rust().rows.get(at))
        else {
            return QVariant::default();
        };
        let (feedback, kind) = feedback_parts(&row.feedback);
        match ROW_ROLES.get(usize::try_from(role - USER_ROLE).unwrap_or(usize::MAX)) {
            Some(&"key") => text(&row.key),
            Some(&"name") => text(&row.name),
            Some(&"icon") => text(&row.icon),
            Some(&"value") => text(&row.value),
            Some(&"secondary") => text(&row.secondary),
            Some(&"pid") => number(row.pid),
            Some(&"processes") => number(row.processes),
            Some(&"present") => QVariant::from(&row.present),
            Some(&"feedback") => text(feedback),
            Some(&"feedback_kind") => text(kind),
            _ => QVariant::default(),
        }
    }

    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().rows.len() as i32
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        roles(&ROW_ROLES)
    }
}

impl qobject::PortsView {
    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Ok(mut view) = PORTS_VIEW.lock() {
            *view = Some(thread);
        }
        self.apply(process_state::ports());
    }

    fn refresh(self: Pin<&mut Self>) {
        process_state::refresh_ports(publish_ports);
    }

    fn end_port(self: Pin<&mut Self>, key: &QString) {
        process_state::end_port(&key.to_string(), false, publish_ports);
    }

    fn force_end_port(self: Pin<&mut Self>, key: &QString) {
        process_state::end_port(&key.to_string(), true, publish_ports);
    }

    fn apply(mut self: Pin<&mut Self>, shown: process_state::PortsShown) {
        self.as_mut().set_refreshing(shown.refreshing);
        self.as_mut().set_status_text(QString::from(shown.status_text()));
        sync(self, shown.rows);
    }

    fn role_value(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = usize::try_from(index.row()).ok().and_then(|at| self.rust().rows.get(at))
        else {
            return QVariant::default();
        };
        let (feedback, kind) = feedback_parts(&row.feedback);
        match PORT_ROLES.get(usize::try_from(role - USER_ROLE).unwrap_or(usize::MAX)) {
            Some(&"key") => text(&row.key),
            Some(&"port") => number(row.port),
            Some(&"transport") => text(row.transport),
            Some(&"address") => text(&row.address),
            Some(&"exposure") => text(row.exposure),
            Some(&"exposure_text") => text(&row.exposure_text),
            Some(&"name") => text(&row.name),
            Some(&"icon") => text(&row.icon),
            Some(&"pid") => number(row.pid),
            Some(&"url") => text(&row.url),
            Some(&"endable") => QVariant::from(&row.endable),
            Some(&"confirm_text") => text(&row.confirm_text),
            Some(&"open") => QVariant::from(&row.open),
            Some(&"feedback") => text(feedback),
            Some(&"feedback_kind") => text(kind),
            _ => QVariant::default(),
        }
    }

    fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().rows.len() as i32
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        roles(&PORT_ROLES)
    }
}

pub fn publish() {
    let Ok(view) = VIEW.lock() else {
        return;
    };
    if let Some(thread) = view.as_ref() {
        let _ = thread.queue(|view| view.apply(process_state::get()));
    }
}

pub fn publish_ports() {
    let Ok(view) = PORTS_VIEW.lock() else {
        return;
    };
    if let Some(thread) = view.as_ref() {
        let _ = thread.queue(|view| view.apply(process_state::ports()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn replay(old: &[String], steps: &[Edit], new: &[String]) -> Vec<String> {
        let mut current = old.to_vec();
        for step in steps {
            match *step {
                Edit::Remove(at) => {
                    current.remove(at);
                }
                Edit::Insert(at) => current.insert(at, new[at].clone()),
                Edit::Move { from, to } => {
                    assert!(to < from, "a move is always upward");
                    let moved = current.remove(from);
                    current.insert(to, moved);
                }
            }
        }
        current
    }

    #[test]
    fn unchanged_keys_need_no_edit_at_all() {
        let order = keys(&["a", "b", "c"]);
        assert!(edits(&order, &order).is_empty(), "figures alone arrive as dataChanged");
    }

    #[test]
    fn a_swap_is_one_move_and_keeps_every_row() {
        let steps = edits(&keys(&["a", "b", "c"]), &keys(&["b", "a", "c"]));
        assert_eq!(steps, vec![Edit::Move { from: 1, to: 0 }]);
    }

    #[test]
    fn any_reordering_replays_to_the_new_list() {
        let cases = [
            (vec!["a", "b", "c", "d"], vec!["d", "c", "b", "a"]),
            (vec!["a", "b", "c"], vec!["x", "c", "a"]),
            (vec![], vec!["a", "b"]),
            (vec!["a", "b"], vec![]),
            (vec!["a", "b", "c", "d", "e"], vec!["e", "a", "y", "c"]),
        ];
        for (old, new) in cases {
            let (old, new) = (keys(&old), keys(&new));
            let steps = edits(&old, &new);
            assert_eq!(replay(&old, &steps, &new), new, "{old:?} to {new:?}");
        }
    }
}
