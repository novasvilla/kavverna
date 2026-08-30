use crate::command::{self, Command};
use crate::panel_anchor::{self, Placement};
use crate::{launch_at_login, settings};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use keep_awake::{Hold, format_duration};
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
        #[qproperty(QString, theme_name)]
        #[qproperty(bool, launch_at_login)]
        #[qproperty(QString, settings_path)]
        #[qproperty(QString, version)]
        #[qproperty(i32, placement)]
        #[qproperty(i32, panel_width)]
        #[qproperty(QString, panel_screen)]
        #[qproperty(bool, at_bottom)]
        #[qproperty(bool, at_right)]
        #[qproperty(i32, margin_left)]
        #[qproperty(i32, margin_top)]
        #[qproperty(i32, margin_right)]
        #[qproperty(i32, margin_bottom)]
        #[qproperty(bool, ghost_visible)]
        #[qproperty(i32, ghost_left)]
        #[qproperty(i32, ghost_top)]
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
        fn choose_theme(self: Pin<&mut KavvernaPanel>, name: &QString);
        #[qinvokable]
        fn choose_middle_click_toggle(self: Pin<&mut KavvernaPanel>, toggle: bool);
        #[qinvokable]
        fn extend_minutes(self: Pin<&mut KavvernaPanel>, minutes: i32);
        #[qinvokable]
        fn nudge_now(self: Pin<&mut KavvernaPanel>);
        #[qinvokable]
        fn choose_launch_at_login(self: Pin<&mut KavvernaPanel>, launch: bool);
        #[qinvokable]
        fn report_screens(self: Pin<&mut KavvernaPanel>, report: &QString);
        #[qinvokable]
        fn choose_placement(self: Pin<&mut KavvernaPanel>, mode: i32, width: i32, height: i32);
        #[qinvokable]
        fn drag_begun(self: Pin<&mut KavvernaPanel>, width: i32, height: i32);
        #[qinvokable]
        fn drag_preview(self: Pin<&mut KavvernaPanel>, dx: i32, dy: i32, width: i32, height: i32);
        #[qinvokable]
        fn drag_commit(self: Pin<&mut KavvernaPanel>, width: i32, height: i32);
    }
}

use core::pin::Pin;

static PANEL: Mutex<Option<cxx_qt::CxxQtThread<qobject::KavvernaPanel>>> = Mutex::new(None);

/// What the interface reported the connected screens to be. Placement math happens on the Qt
/// thread but the report arrives once at startup, so a copy behind a mutex is enough.
static SCREENS: Mutex<Vec<panel_anchor::Screen>> = Mutex::new(Vec::new());

fn screens() -> Vec<panel_anchor::Screen> {
    SCREENS.lock().map(|held| held.clone()).unwrap_or_default()
}

fn gap() -> i32 {
    as_i32(settings::integer_at(settings::PLACEMENT_GAP, settings::PLACEMENT_GAP_DEFAULT), 12)
}

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
    theme_name: QString,
    launch_at_login: bool,
    settings_path: QString,
    version: QString,
    placement: i32,
    panel_width: i32,
    panel_screen: QString,
    at_bottom: bool,
    at_right: bool,
    margin_left: i32,
    margin_top: i32,
    margin_right: i32,
    margin_bottom: i32,
    ghost_visible: bool,
    ghost_left: i32,
    ghost_top: i32,
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
            theme_name: QString::from(&settings::text_at(settings::THEME, settings::THEME_DEFAULT)),
            launch_at_login: launch_at_login::is_enabled(),
            settings_path: QString::from(&settings_location()),
            version: QString::from(&crate::remote::version()),
            placement: as_i32(
                settings::integer_at(settings::PLACEMENT, settings::PLACEMENT_DEFAULT),
                0,
            ),
            panel_width: panel_anchor::WIDTH,
            panel_screen: QString::default(),
            at_bottom: true,
            at_right: true,
            margin_left: 0,
            margin_top: 0,
            margin_right: gap(),
            margin_bottom: gap(),
            ghost_visible: false,
            ghost_left: 0,
            ghost_top: 0,
        }
    }
}

/// Where a drag started from, in the screen's local coordinates. One drag at a time, on the
/// Qt thread only; the mutex is for the static.
static DRAG_FROM: Mutex<Option<(i32, i32)>> = Mutex::new(None);

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
            place(&mut self, None);
            self.as_mut().set_panel_open(true);
        }
        if let Ok(mut panel) = PANEL.lock() {
            *panel = Some(thread);
        }
    }

    fn dismiss(mut self: Pin<&mut Self>) {
        self.as_mut().set_panel_open(false);
        tracing::info!("panel closed");
    }

    fn show_settings(mut self: Pin<&mut Self>, showing: bool) {
        self.as_mut().set_showing_settings(showing);
    }

    fn toggle_awake(self: Pin<&mut Self>) {
        crate::awake_state::toggle();
    }

    fn keep_awake_minutes(self: Pin<&mut Self>, minutes: i32) {
        let seconds = u64::try_from(minutes).unwrap_or(0).saturating_mul(60);
        command::send(Command::Engage(Hold::For(Duration::from_secs(seconds)), settings::scope()));
    }

    fn choose_display_sleep(mut self: Pin<&mut Self>, allow: bool) {
        settings::put_bool(settings::ALLOW_DISPLAY_SLEEP, allow);
        self.as_mut().set_allow_display_sleep(allow);

        if *self.awake() {
            let hold = crate::awake_state::get().remaining.map_or(Hold::Indefinite, Hold::For);
            command::send(Command::Engage(hold, settings::scope()));
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

    fn choose_theme(mut self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        settings::put_text(settings::THEME, &name);
        self.as_mut().set_theme_name(QString::from(&name));
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

    fn report_screens(self: Pin<&mut Self>, report: &QString) {
        let seen = panel_anchor::screens_from_report(&report.to_string());
        tracing::info!(screens = seen.len(), "screens reported");
        if let Ok(mut held) = SCREENS.lock() {
            *held = seen;
        }
    }

    fn choose_placement(mut self: Pin<&mut Self>, mode: i32, width: i32, height: i32) {
        settings::put_integer(settings::PLACEMENT, i64::from(mode));
        self.as_mut().set_placement(mode);
        if mode == 1 {
            // The spot it is in right now becomes the remembered one, so choosing this
            // moves nothing.
            self.remember_position(width, height);
        } else {
            place(&mut self, None);
        }
    }

    /// A drag never moves the panel while the button is down: moving the surface under the
    /// pointer shifts the pointer's surface-local reading by exactly the move, and the
    /// compositor replays that shift on its own schedule, which fed back as a panel that
    /// jittered off at random. The panel holds still, a ghost outline tracks the gesture in
    /// clean press-relative coordinates, and release places the panel where the ghost is.
    fn drag_begun(mut self: Pin<&mut Self>, width: i32, height: i32) {
        let screens = screens();
        let name = self.panel_screen().to_string();
        let Some(screen) = screens.iter().find(|s| s.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let from = panel_anchor::position_of(&self.read_placement(), screen, (width, height));
        if let Ok(mut held) = DRAG_FROM.lock() {
            *held = Some(from);
        }
        self.as_mut().set_ghost_left(from.0);
        self.as_mut().set_ghost_top(from.1);
    }

    fn drag_preview(mut self: Pin<&mut Self>, dx: i32, dy: i32, width: i32, height: i32) {
        let Some((x, y)) = DRAG_FROM.lock().ok().and_then(|held| *held) else {
            return;
        };
        // A click is not a drag; the ghost only appears once the hand has clearly moved.
        if !*self.ghost_visible() && dx.abs() + dy.abs() < 8 {
            return;
        }
        let screens = screens();
        let name = self.panel_screen().to_string();
        let Some(screen) = screens.iter().find(|s| s.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let at = panel_anchor::pinned((x + dx, y + dy), screen, (width, height), gap());
        self.as_mut().set_ghost_left(at.left);
        self.as_mut().set_ghost_top(at.top);
        self.as_mut().set_ghost_visible(true);
    }

    fn drag_commit(mut self: Pin<&mut Self>, width: i32, height: i32) {
        let began = DRAG_FROM.lock().ok().and_then(|mut held| held.take());
        if began.is_none() || !*self.ghost_visible() {
            self.as_mut().set_ghost_visible(false);
            return;
        }
        self.as_mut().set_ghost_visible(false);

        let screens = screens();
        let name = self.panel_screen().to_string();
        let Some(screen) = screens.iter().find(|s| s.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let landed = panel_anchor::pinned(
            (*self.ghost_left(), *self.ghost_top()),
            screen,
            (width, height),
            gap(),
        );
        let name = screen.name.clone();
        apply_placement(&mut self, landed, &name);

        if settings::integer_at(settings::PLACEMENT, settings::PLACEMENT_DEFAULT) == 1 {
            self.remember_position(width, height);
        }
    }

    fn read_placement(&self) -> Placement {
        Placement {
            at_bottom: *self.at_bottom(),
            at_right: *self.at_right(),
            left: *self.margin_left(),
            top: *self.margin_top(),
            right: *self.margin_right(),
            bottom: *self.margin_bottom(),
        }
    }

    fn remember_position(mut self: Pin<&mut Self>, width: i32, height: i32) {
        let screens = screens();
        let name = self.panel_screen().to_string();
        let Some(screen) = screens.iter().find(|s| s.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let (x, y) = panel_anchor::position_of(&self.read_placement(), screen, (width, height));
        let pinned = panel_anchor::pinned((x, y), screen, (width, height), gap());
        let name = screen.name.clone();
        let entries = settings::texts_at(settings::PLACEMENT_REMEMBERED).unwrap_or_default();
        settings::put_texts(
            settings::PLACEMENT_REMEMBERED,
            &panel_anchor::with_position(entries, &name, pinned.left, pinned.top),
        );
        apply_placement(&mut self, pinned, &name);
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

fn apply_placement(panel: &mut Pin<&mut qobject::KavvernaPanel>, place: Placement, screen: &str) {
    panel.as_mut().set_at_bottom(place.at_bottom);
    panel.as_mut().set_at_right(place.at_right);
    panel.as_mut().set_margin_left(place.left);
    panel.as_mut().set_margin_top(place.top);
    panel.as_mut().set_margin_right(place.right);
    panel.as_mut().set_margin_bottom(place.bottom);
    panel.as_mut().set_panel_screen(QString::from(screen));
}

/// Works out where the panel opens this time. Every opening path passes through here, so a
/// mode change or a tray icon that moved takes effect on the very next open.
fn place(panel: &mut Pin<&mut qobject::KavvernaPanel>, click: Option<(i32, i32)>) {
    let spacing = gap();
    let screens = screens();
    let mode = settings::integer_at(settings::PLACEMENT, settings::PLACEMENT_DEFAULT);

    let chosen = match mode {
        1 => {
            let entries = settings::texts_at(settings::PLACEMENT_REMEMBERED).unwrap_or_default();
            panel_anchor::last_remembered(&entries, &screens).map(|((x, y), screen)| {
                let size = (panel_anchor::WIDTH, 720.min(screen.height - 24));
                (panel_anchor::pinned((x, y), screen, size, spacing), screen.name.clone())
            })
        }
        2 => None,
        _ => beside_tray_or_remembered_anchor(click, &screens, spacing),
    };

    match chosen {
        Some((placement, screen)) => apply_placement(panel, placement, &screen),
        None => apply_placement(panel, panel_anchor::corner(spacing), ""),
    }
}

/// A fresh click wins and is kept for the opens that carry none, a shortcut or a script. A
/// stored click is trusted only while its screen is still connected and still contains it.
fn beside_tray_or_remembered_anchor(
    click: Option<(i32, i32)>,
    screens: &[panel_anchor::Screen],
    spacing: i32,
) -> Option<(Placement, String)> {
    if let Some(point) = click {
        if let Some(screen) = panel_anchor::screen_containing(point, screens) {
            settings::put_text(
                settings::PLACEMENT_ANCHOR,
                &panel_anchor::entry(&screen.name, point.0, point.1),
            );
            return Some((panel_anchor::beside_tray(point, screen, spacing), screen.name.clone()));
        }
        tracing::info!(?point, "tray click outside every screen, treated as absent");
    }

    let stored = settings::text_at(settings::PLACEMENT_ANCHOR, "");
    let (name, x, y) = panel_anchor::parse(&stored)?;
    let screen = screens.iter().find(|screen| screen.name == name)?;
    panel_anchor::screen_containing((x, y), std::slice::from_ref(screen))?;
    Some((panel_anchor::beside_tray((x, y), screen, spacing), screen.name.clone()))
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
    toggle_from(None);
}

/// The tray click carries the icon's screen coordinates, the one thing on Wayland that knows
/// where the icon is.
pub fn toggle_at(x: i32, y: i32) {
    toggle_from(Some((x, y)));
}

fn toggle_from(click: Option<(i32, i32)>) {
    with_panel(move |mut panel| {
        let open = *panel.panel_open();
        panel.as_mut().set_showing_settings(false);
        if !open {
            place(&mut panel, click);
        }
        panel.as_mut().set_panel_open(!open);
        tracing::info!(now_open = !open, "tray toggled the panel");
    });
}

pub fn open_hub() {
    with_panel(|mut panel| {
        panel.as_mut().set_showing_settings(false);
        place(&mut panel, None);
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
        place(&mut panel, None);
        panel.as_mut().set_panel_open(true);
    });
}

pub fn open_settings() {
    with_panel(|mut panel| {
        panel.as_mut().set_showing_settings(true);
        place(&mut panel, None);
        panel.as_mut().set_panel_open(true);
    });
}
