use crate::command::{self, Command};
use crate::{launch_at_login, settings};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use keep_awake::{Hold, Scope, format_duration};
use std::sync::Mutex;
use std::time::Duration;

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
        #[qproperty(i32, page)]
        #[qproperty(bool, awake)]
        #[qproperty(QString, awake_summary)]
        #[qproperty(bool, allow_display_sleep)]
        #[qproperty(bool, restore_on_start)]
        #[qproperty(bool, mouse_jiggle)]
        #[qproperty(i32, jiggle_shortest)]
        #[qproperty(i32, jiggle_longest)]
        #[qproperty(i32, jiggle_activity)]
        #[qproperty(i32, jiggle_keystroke)]
        #[qproperty(i32, default_minutes)]
        #[qproperty(bool, middle_click_toggle)]
        #[qproperty(bool, timed)]
        #[qproperty(bool, jiggle_available)]
        #[qproperty(QString, jiggle_status)]
        #[qproperty(i32, appearance)]
        #[qproperty(bool, launch_at_login)]
        #[qproperty(QString, settings_path)]
        #[qproperty(QString, version)]
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
        fn choose_jiggle_shortest(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn choose_jiggle_longest(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn report_screen(self: Pin<&mut KavvernaPanel>, width: i32, height: i32);
        #[qinvokable]
        fn choose_jiggle_activity(self: Pin<&mut KavvernaPanel>, activity: i32);
        #[qinvokable]
        fn choose_jiggle_keystroke(self: Pin<&mut KavvernaPanel>, keystroke: i32);
        #[qinvokable]
        fn choose_default_minutes(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn choose_appearance(self: Pin<&mut KavvernaPanel>, appearance: i32);
        #[qinvokable]
        fn choose_middle_click_toggle(self: Pin<&mut KavvernaPanel>, toggle: bool);
        #[qinvokable]
        fn extend_minutes(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn nudge_now(self: Pin<&mut KavvernaPanel>);
        #[qinvokable]
        fn choose_launch_at_login(self: Pin<&mut KavvernaPanel>, launch: bool);
    }
}

use core::pin::Pin;

static PANEL: Mutex<Option<cxx_qt::CxxQtThread<qobject::KavvernaPanel>>> = Mutex::new(None);

pub struct KavvernaPanelRust {
    panel_open: bool,
    showing_settings: bool,
    page: i32,
    awake: bool,
    awake_summary: QString,
    allow_display_sleep: bool,
    restore_on_start: bool,
    mouse_jiggle: bool,
    jiggle_shortest: i32,
    jiggle_longest: i32,
    jiggle_activity: i32,
    jiggle_keystroke: i32,
    default_minutes: i32,
    middle_click_toggle: bool,
    timed: bool,
    jiggle_available: bool,
    jiggle_status: QString,
    appearance: i32,
    launch_at_login: bool,
    settings_path: QString,
    version: QString,
}

impl Default for KavvernaPanelRust {
    fn default() -> Self {
        let allow_display_sleep =
            settings::bool_at(settings::ALLOW_DISPLAY_SLEEP, settings::ALLOW_DISPLAY_SLEEP_DEFAULT);
        let restore_on_start =
            settings::bool_at(settings::RESTORE_ON_START, settings::RESTORE_ON_START_DEFAULT);
        Self {
            panel_open: false,
            showing_settings: false,
            page: 0,
            awake: false,
            awake_summary: QString::from("Sleep allowed"),
            allow_display_sleep,
            restore_on_start,
            mouse_jiggle: settings::bool_at(settings::MOUSE_JIGGLE, settings::MOUSE_JIGGLE_DEFAULT),
            jiggle_shortest: as_i32(
                settings::integer_at(settings::JIGGLE_SHORTEST, settings::JIGGLE_SHORTEST_DEFAULT),
                2,
            ),
            jiggle_longest: as_i32(
                settings::integer_at(settings::JIGGLE_LONGEST, settings::JIGGLE_LONGEST_DEFAULT),
                7,
            ),
            jiggle_activity: as_i32(
                settings::integer_at(settings::JIGGLE_ACTIVITY, settings::JIGGLE_ACTIVITY_DEFAULT),
                0,
            ),
            jiggle_keystroke: as_i32(
                settings::integer_at(
                    settings::JIGGLE_KEYSTROKE,
                    settings::JIGGLE_KEYSTROKE_DEFAULT,
                ),
                0,
            ),
            default_minutes: as_i32(
                settings::integer_at(settings::DEFAULT_MINUTES, settings::DEFAULT_MINUTES_DEFAULT),
                0,
            ),
            middle_click_toggle: settings::bool_at(
                settings::MIDDLE_CLICK_TOGGLE,
                settings::MIDDLE_CLICK_TOGGLE_DEFAULT,
            ),
            timed: false,
            jiggle_available: keep_awake::MouseJiggle::is_available(),
            jiggle_status: QString::from("Off"),
            appearance: as_i32(
                settings::integer_at(settings::APPEARANCE, settings::APPEARANCE_DEFAULT),
                0,
            ),
            launch_at_login: launch_at_login::is_enabled(),
            settings_path: QString::from(&settings_location()),
            version: QString::from(&crate::remote::version()),
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

fn describe_jiggle(state: crate::jiggle_state::JiggleState) -> String {
    if !state.running {
        return "Off".into();
    }

    let waiting = state.waiting_seconds / 60;
    match state.seconds_until_next {
        Some(seconds) if seconds >= 60 => {
            format!(
                "Next in {}m {:02}s of {waiting}m  ·  {} so far",
                seconds / 60,
                seconds % 60,
                state.nudges
            )
        }
        Some(seconds) => format!("Next in {seconds}s of {waiting}m  ·  {} so far", state.nudges),
        None => "Starting".into(),
    }
}

fn summarise(active: bool, remaining: Option<Duration>) -> String {
    match (active, remaining) {
        (false, _) => "Sleep allowed".into(),
        (true, Some(left)) => format!("Awake for {}", format_duration(left)),
        (true, None) => "Awake until switched off".into(),
    }
}

impl qobject::KavvernaPanel {
    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Some(wanted) = REQUESTED.lock().ok().and_then(|mut held| held.take()) {
            match wanted {
                Requested::Settings => self.as_mut().set_showing_settings(true),
                Requested::Page(name) => self.as_mut().set_page(page_number(&name)),
            }
            self.as_mut().set_panel_open(true);
        }
        if let Ok(mut panel) = PANEL.lock() {
            *panel = Some(thread);
        }
    }

    fn scope(&self) -> Scope {
        if *self.allow_display_sleep() { Scope::SystemOnly } else { Scope::SystemAndDisplay }
    }

    fn dismiss(mut self: Pin<&mut Self>) {
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
            command::send(Command::Engage(settings::default_hold(), self.scope()));
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

    /// The two ends are kept apart, since a range that collapses gives a fixed rhythm again.
    fn choose_jiggle_shortest(mut self: Pin<&mut Self>, minutes: i32) {
        settings::put_integer(settings::JIGGLE_SHORTEST, i64::from(minutes));
        self.as_mut().set_jiggle_shortest(minutes);

        if *self.jiggle_longest() < minutes {
            self.as_mut().choose_jiggle_longest(minutes);
        }
    }

    fn choose_jiggle_longest(mut self: Pin<&mut Self>, minutes: i32) {
        settings::put_integer(settings::JIGGLE_LONGEST, i64::from(minutes));
        self.as_mut().set_jiggle_longest(minutes);

        if *self.jiggle_shortest() > minutes {
            settings::put_integer(settings::JIGGLE_SHORTEST, i64::from(minutes));
            self.as_mut().set_jiggle_shortest(minutes);
        }
    }

    fn choose_appearance(mut self: Pin<&mut Self>, appearance: i32) {
        settings::put_integer(settings::APPEARANCE, i64::from(appearance));
        self.as_mut().set_appearance(appearance);
    }

    fn choose_jiggle_activity(mut self: Pin<&mut Self>, activity: i32) {
        settings::put_integer(settings::JIGGLE_ACTIVITY, i64::from(activity));
        self.as_mut().set_jiggle_activity(activity);
    }

    fn choose_jiggle_keystroke(mut self: Pin<&mut Self>, keystroke: i32) {
        settings::put_integer(settings::JIGGLE_KEYSTROKE, i64::from(keystroke));
        self.as_mut().set_jiggle_keystroke(keystroke);
    }

    fn report_screen(self: Pin<&mut Self>, width: i32, height: i32) {
        crate::jiggle_state::set_screen(width, height);
    }

    fn choose_default_minutes(mut self: Pin<&mut Self>, minutes: i32) {
        settings::put_integer(settings::DEFAULT_MINUTES, i64::from(minutes));
        self.as_mut().set_default_minutes(minutes);
    }

    fn choose_middle_click_toggle(mut self: Pin<&mut Self>, toggle: bool) {
        settings::put_bool(settings::MIDDLE_CLICK_TOGGLE, toggle);
        self.as_mut().set_middle_click_toggle(toggle);
    }

    fn nudge_now(self: Pin<&mut Self>) {
        command::send(Command::NudgeNow);
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
        let jiggle = crate::jiggle_state::get();
        self.as_mut().set_jiggle_status(QString::from(&describe_jiggle(jiggle)));
        self.as_mut().set_awake(active);
        self.as_mut().set_timed(active && remaining.is_some());
        self.as_mut().set_awake_summary(QString::from(&summarise(active, remaining)));
    }
}

fn with_panel(action: impl FnOnce(Pin<&mut qobject::KavvernaPanel>) + Send + 'static) {
    let Ok(panel) = PANEL.lock() else {
        return;
    };
    match panel.as_ref() {
        Some(thread) => {
            let _ = thread.queue(action);
        }
        None => {
            static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                tracing::error!(
                    "the interface never loaded; run with QT_LOGGING_RULES='qt.qml.*=true' to \
                     see why, since a QML failure is otherwise silent"
                );
            }
        }
    }
}

pub fn publish_awake(active: bool, remaining: Option<Duration>) {
    with_panel(move |panel| panel.apply(active, remaining));
}

/// Flipped on the Qt thread, where the panel's own state is the authority. Reading a mirror
/// of it from the tray thread would race with the user closing the panel by hand.
pub fn toggle() {
    with_panel(|mut panel| {
        let open = *panel.panel_open();
        panel.as_mut().set_showing_settings(false);
        panel.as_mut().set_panel_open(!open);
        tracing::info!(now_open = !open, "tray toggled the panel");
    });
}

pub fn open_hub() {
    with_panel(|mut panel| {
        panel.as_mut().set_showing_settings(false);
        panel.as_mut().set_panel_open(true);
    });
}

/// Named rather than numbered so a script does not have to know the tab order.
fn page_number(name: &str) -> i32 {
    match name {
        "sound" => 1,
        "monitoring" => 2,
        "clipboard" => 3,
        "tools" => 4,
        _ => 0,
    }
}

/// What a launch asked for, applied once the interface exists rather than guessed at with a
/// delay, since nothing can be shown before then.
pub enum Requested {
    Page(String),
    Settings,
}

static REQUESTED: Mutex<Option<Requested>> = Mutex::new(None);

pub fn request(wanted: Requested) {
    if let Ok(mut held) = REQUESTED.lock() {
        *held = Some(wanted);
    }
}

pub fn open_page(name: &str) {
    let page = page_number(name);

    with_panel(move |mut panel| {
        panel.as_mut().set_showing_settings(false);
        panel.as_mut().set_page(page);
        panel.as_mut().set_panel_open(true);
    });
}

pub fn open_settings() {
    with_panel(|mut panel| {
        panel.as_mut().set_showing_settings(true);
        panel.as_mut().set_panel_open(true);
    });
}
