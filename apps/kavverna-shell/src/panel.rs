use crate::command::{self, Command};
use crate::{launch_at_login, settings};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use keep_awake::{Hold, Scope, format_duration};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, panel_open)]
        #[qproperty(bool, showing_settings)]
        #[qproperty(bool, awake)]
        #[qproperty(QString, awake_summary)]
        #[qproperty(bool, allow_display_sleep)]
        #[qproperty(bool, restore_on_start)]
        #[qproperty(bool, mouse_jiggle)]
        #[qproperty(i32, jiggle_minutes)]
        #[qproperty(i32, default_minutes)]
        #[qproperty(bool, right_click_toggle)]
        #[qproperty(bool, timed)]
        #[qproperty(bool, jiggle_available)]
        #[qproperty(bool, launch_at_login)]
        #[qproperty(QString, settings_path)]
        type KavvernaPanel = super::KavvernaPanelRust;
    }

    impl cxx_qt::Threading for KavvernaPanel {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut KavvernaPanel>);
        #[qinvokable]
        fn dismiss(self: Pin<&mut KavvernaPanel>);
        #[qinvokable]
        fn show_settings(self: Pin<&mut KavvernaPanel>, showing: bool);
        #[qinvokable]
        fn toggle_awake(self: Pin<&mut KavvernaPanel>);
        #[qinvokable]
        fn keep_awake_minutes(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn choose_display_sleep(self: Pin<&mut KavvernaPanel>, allow: bool);
        #[qinvokable]
        fn choose_restore_on_start(self: Pin<&mut KavvernaPanel>, restore: bool);
        #[qinvokable]
        fn choose_mouse_jiggle(self: Pin<&mut KavvernaPanel>, jiggle: bool);
        #[qinvokable]
        fn choose_jiggle_minutes(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn choose_default_minutes(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn choose_right_click_toggle(self: Pin<&mut KavvernaPanel>, toggle: bool);
        #[qinvokable]
        fn extend_minutes(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn choose_launch_at_login(self: Pin<&mut KavvernaPanel>, launch: bool);
    }
}

use core::pin::Pin;

static PANEL: Mutex<Option<cxx_qt::CxxQtThread<qobject::KavvernaPanel>>> = Mutex::new(None);

/// Clicking the tray while the panel is focused makes it lose focus first, so an unguarded
/// open would immediately undo the dismissal and the icon would never close the panel.
static DISMISSED_AT: Mutex<Option<Instant>> = Mutex::new(None);
const DISMISS_GRACE: Duration = Duration::from_millis(400);

pub struct KavvernaPanelRust {
    panel_open: bool,
    showing_settings: bool,
    awake: bool,
    awake_summary: QString,
    allow_display_sleep: bool,
    restore_on_start: bool,
    mouse_jiggle: bool,
    jiggle_minutes: i32,
    default_minutes: i32,
    right_click_toggle: bool,
    timed: bool,
    jiggle_available: bool,
    launch_at_login: bool,
    settings_path: QString,
}

impl Default for KavvernaPanelRust {
    fn default() -> Self {
        let allow_display_sleep = settings::bool_at(
            settings::ALLOW_DISPLAY_SLEEP,
            settings::ALLOW_DISPLAY_SLEEP_DEFAULT,
        );
        let restore_on_start =
            settings::bool_at(settings::RESTORE_ON_START, settings::RESTORE_ON_START_DEFAULT);
        Self {
            panel_open: false,
            showing_settings: false,
            awake: false,
            awake_summary: QString::from("Sleep allowed"),
            allow_display_sleep,
            restore_on_start,
            mouse_jiggle: settings::bool_at(settings::MOUSE_JIGGLE, settings::MOUSE_JIGGLE_DEFAULT),
            jiggle_minutes: as_i32(
                settings::integer_at(settings::JIGGLE_MINUTES, settings::JIGGLE_MINUTES_DEFAULT),
                5,
            ),
            default_minutes: as_i32(
                settings::integer_at(settings::DEFAULT_MINUTES, settings::DEFAULT_MINUTES_DEFAULT),
                0,
            ),
            right_click_toggle: settings::bool_at(
                settings::RIGHT_CLICK_TOGGLE,
                settings::RIGHT_CLICK_TOGGLE_DEFAULT,
            ),
            timed: false,
            jiggle_available: keep_awake::MouseJiggle::is_available(),
            launch_at_login: launch_at_login::is_enabled(),
            settings_path: QString::from(&settings_location()),
        }
    }
}

fn as_i32(value: i64, fallback: i32) -> i32 {
    i32::try_from(value).unwrap_or(fallback)
}

fn settings_location() -> String {
    directories::ProjectDirs::from("dev", "", "kavverna")
        .map(|dirs| dirs.config_dir().join("settings.json").display().to_string())
        .unwrap_or_else(|| "not available".into())
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

    fn dismiss(mut self: Pin<&mut Self>) {
        if let Ok(mut at) = DISMISSED_AT.lock() {
            *at = Some(Instant::now());
        }
        self.as_mut().set_panel_open(false);
        tracing::info!("panel closed");
    }

    fn show_settings(mut self: Pin<&mut Self>, showing: bool) {
        self.as_mut().set_showing_settings(showing);
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
        command::send(Command::Engage(Hold::For(Duration::from_secs(seconds)), self.scope()));
    }

    fn choose_display_sleep(mut self: Pin<&mut Self>, allow: bool) {
        settings::put_bool(settings::ALLOW_DISPLAY_SLEEP, allow);
        self.as_mut().set_allow_display_sleep(allow);

        if *self.awake() {
            let hold = crate::awake_state::get().remaining.map_or(Hold::Indefinite, Hold::For);
            command::send(Command::Engage(hold, self.scope()));
        }
    }

    fn choose_restore_on_start(mut self: Pin<&mut Self>, restore: bool) {
        settings::put_bool(settings::RESTORE_ON_START, restore);
        self.as_mut().set_restore_on_start(restore);
    }

    fn choose_mouse_jiggle(mut self: Pin<&mut Self>, jiggle: bool) {
        settings::put_bool(settings::MOUSE_JIGGLE, jiggle);
        self.as_mut().set_mouse_jiggle(jiggle);
    }

    fn choose_jiggle_minutes(mut self: Pin<&mut Self>, minutes: i32) {
        settings::put_integer(settings::JIGGLE_MINUTES, i64::from(minutes));
        self.as_mut().set_jiggle_minutes(minutes);
    }

    fn choose_default_minutes(mut self: Pin<&mut Self>, minutes: i32) {
        settings::put_integer(settings::DEFAULT_MINUTES, i64::from(minutes));
        self.as_mut().set_default_minutes(minutes);
    }

    fn choose_right_click_toggle(mut self: Pin<&mut Self>, toggle: bool) {
        settings::put_bool(settings::RIGHT_CLICK_TOGGLE, toggle);
        self.as_mut().set_right_click_toggle(toggle);
    }

    fn extend_minutes(self: Pin<&mut Self>, minutes: i32) {
        let seconds = u64::try_from(minutes).unwrap_or(0).saturating_mul(60);
        command::send(Command::Extend(Duration::from_secs(seconds)));
    }

    fn choose_launch_at_login(mut self: Pin<&mut Self>, launch: bool) {
        match launch_at_login::set(launch) {
            Ok(()) => self.as_mut().set_launch_at_login(launch_at_login::is_enabled()),
            Err(err) => {
                tracing::error!(%err, "could not change launch at login");
                self.as_mut().set_launch_at_login(launch_at_login::is_enabled());
            }
        }
    }

    fn apply(mut self: Pin<&mut Self>, active: bool, remaining: Option<Duration>) {
        self.as_mut().set_awake(active);
        self.as_mut().set_timed(active && remaining.is_some());
        self.as_mut().set_awake_summary(QString::from(&summarise(active, remaining)));
    }
}

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

/// Opens unless the panel has just dismissed itself, which is what makes a second click on
/// the tray icon close it rather than reopen it.
pub fn toggle() {
    if let Ok(mut at) = DISMISSED_AT.lock() {
        if at.is_some_and(|moment| moment.elapsed() < DISMISS_GRACE) {
            *at = None;
            tracing::info!("tray toggle: stayed closed");
            return;
        }
        *at = None;
    }

    tracing::info!("tray toggle: opening");
    with_panel(|mut panel| {
        panel.as_mut().set_showing_settings(false);
        panel.as_mut().set_panel_open(true);
    });
}

pub fn open_settings() {
    if let Ok(mut at) = DISMISSED_AT.lock() {
        *at = None;
    }

    with_panel(|mut panel| {
        panel.as_mut().set_showing_settings(true);
        panel.as_mut().set_panel_open(true);
    });
}
