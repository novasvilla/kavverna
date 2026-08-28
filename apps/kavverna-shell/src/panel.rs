use crate::awake_state;
use crate::command::{self, Command};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use feature_catalog::Feature;
use keep_awake::{Hold, Scope, format_duration};
use std::sync::Mutex;
use std::time::Duration;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, panel_open)]
        #[qproperty(bool, awake)]
        #[qproperty(QString, awake_summary)]
        #[qproperty(bool, allow_display_sleep)]
        #[qproperty(QStringList, section_titles)]
        #[qproperty(QStringList, section_summaries)]
        type KavvernaPanel = super::KavvernaPanelRust;
    }

    impl cxx_qt::Threading for KavvernaPanel {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut KavvernaPanel>);
        #[qinvokable]
        fn toggle_awake(self: Pin<&mut KavvernaPanel>);
        #[qinvokable]
        fn keep_awake_minutes(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn release_awake(self: Pin<&mut KavvernaPanel>);
        #[qinvokable]
        fn set_display_sleep(self: Pin<&mut KavvernaPanel>, allow: bool);
        #[qinvokable]
        fn close_panel(self: Pin<&mut KavvernaPanel>);
    }
}

use core::pin::Pin;

/// Set once QML has built the panel, so the tray thread can raise it.
static PANEL: Mutex<Option<cxx_qt::CxxQtThread<qobject::KavvernaPanel>>> = Mutex::new(None);

pub struct KavvernaPanelRust {
    panel_open: bool,
    awake: bool,
    awake_summary: QString,
    allow_display_sleep: bool,
    section_titles: QStringList,
    section_summaries: QStringList,
}

impl Default for KavvernaPanelRust {
    fn default() -> Self {
        let mut section_titles = QStringList::default();
        let mut section_summaries = QStringList::default();

        for feature in implemented() {
            let descriptor = feature.describe();
            section_titles.append(QString::from(descriptor.title));
            section_summaries.append(QString::from(descriptor.summary));
        }

        Self {
            panel_open: true,
            awake: false,
            awake_summary: QString::from("Sleep allowed"),
            allow_display_sleep: true,
            section_titles,
            section_summaries,
        }
    }
}

/// Only features with working controls. A section for something unbuilt would be a lie.
fn implemented() -> Vec<Feature> {
    vec![Feature::KeepAwake]
}

fn summarise(active: bool, remaining: Option<Duration>) -> String {
    match (active, remaining) {
        (false, _) => "Sleep allowed".into(),
        (true, Some(left)) => format!("Awake for {}", format_duration(left)),
        (true, None) => "Awake until switched off".into(),
    }
}

impl qobject::KavvernaPanel {
    fn attach(self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        if let Ok(mut panel) = PANEL.lock() {
            *panel = Some(thread);
        }
    }

    fn scope(&self) -> Scope {
        if *self.allow_display_sleep() { Scope::SystemOnly } else { Scope::SystemAndDisplay }
    }

    fn toggle_awake(self: Pin<&mut Self>) {
        if *self.awake() {
            command::send(Command::Release);
        } else {
            command::send(Command::Engage(Hold::Indefinite, self.scope()));
        }
    }

    fn keep_awake_minutes(self: Pin<&mut Self>, minutes: i32) {
        let seconds = u64::try_from(minutes).unwrap_or(0).saturating_mul(60);
        command::send(Command::Engage(
            Hold::For(Duration::from_secs(seconds)),
            self.scope(),
        ));
    }

    fn release_awake(self: Pin<&mut Self>) {
        command::send(Command::Release);
    }

    fn set_display_sleep(mut self: Pin<&mut Self>, allow: bool) {
        awake_state::set_allow_display_sleep(allow);
        self.as_mut().set_allow_display_sleep(allow);

        if *self.awake() {
            let hold = awake_state::get().remaining.map_or(Hold::Indefinite, Hold::For);
            command::send(Command::Engage(hold, self.scope()));
        }
    }

    fn close_panel(mut self: Pin<&mut Self>) {
        self.as_mut().set_panel_open(false);
    }

    fn apply(mut self: Pin<&mut Self>, active: bool, remaining: Option<Duration>) {
        self.as_mut().set_awake(active);
        self.as_mut()
            .set_awake_summary(QString::from(&summarise(active, remaining)));
    }
}

/// Called from threads that have no access to the Qt event loop.
fn with_panel(action: impl FnOnce(Pin<&mut qobject::KavvernaPanel>) + Send + 'static) {
    let Ok(panel) = PANEL.lock() else {
        return;
    };
    if let Some(thread) = panel.as_ref() {
        let _ = thread.queue(action);
    }
}

pub fn publish_awake(active: bool, remaining: Option<Duration>) {
    with_panel(move |panel| panel.apply(active, remaining));
}

pub fn open() {
    with_panel(|mut panel| panel.as_mut().set_panel_open(true));
}
