use crate::command::{self, Command};
use crate::panel_anchor::{self, Placement};
use crate::{launch_at_login, settings};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use keep_awake::{Hold, format_duration};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
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
        #[qproperty(i32, hold_preset)]
        #[qproperty(bool, jiggle_available)]
        #[qproperty(QString, jiggle_status)]
        #[qproperty(i32, appearance)]
        #[qproperty(QString, theme_name)]
        #[qproperty(bool, launch_at_login)]
        #[qproperty(QString, settings_path)]
        #[qproperty(QString, version)]
        #[qproperty(i32, placement)]
        #[qproperty(bool, panel_on_top)]
        #[qproperty(i32, panel_width)]
        #[qproperty(QString, panel_screen)]
        #[qproperty(bool, at_bottom)]
        #[qproperty(bool, at_right)]
        #[qproperty(i32, margin_left)]
        #[qproperty(i32, margin_top)]
        #[qproperty(i32, margin_right)]
        #[qproperty(i32, margin_bottom)]
        #[qproperty(bool, ghost_visible)]
        #[qproperty(QString, ghost_screen)]
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
        fn choose_panel_on_top(self: Pin<&mut KavvernaPanel>, on_top: bool);
        #[qinvokable]
        fn drag_begun(
            self: Pin<&mut KavvernaPanel>,
            pointer_x: i32,
            pointer_y: i32,
            width: i32,
            height: i32,
        );
        #[qinvokable]
        fn drag_preview(
            self: Pin<&mut KavvernaPanel>,
            pointer_x: i32,
            pointer_y: i32,
            width: i32,
            height: i32,
        );
        #[qinvokable]
        fn drag_commit(self: Pin<&mut KavvernaPanel>, width: i32, height: i32);
        #[qinvokable]
        fn report_work_area(self: Pin<&mut KavvernaPanel>, name: &QString, width: i32, height: i32);
    }
}

use core::pin::Pin;

static PANEL: Mutex<Option<cxx_qt::CxxQtThread<qobject::KavvernaPanel>>> = Mutex::new(None);

/// What the interface last reported the connected screens to be. Qt sends a fresh report for
/// hotplug and geometry changes; other surfaces read the same snapshot behind this mutex.
static SCREENS: Mutex<Vec<panel_anchor::Screen>> = Mutex::new(Vec::new());

/// The free area of each screen, measured by the drag overlay when it first maps: the
/// compositor sizes that surface around every reserved strip, and no Qt property reports
/// that on Wayland. Free-position arithmetic uses these sizes so a spot the ghost shows is
/// the spot the panel lands on, clear of the task bar.
static WORK_AREAS: Mutex<Vec<(String, i32, i32)>> = Mutex::new(Vec::new());

/// Mirrors `panel_open` for the feature threads, which skip pushing into QML while nobody can
/// see it. Kept from the property's own change signal, since QML writes the property too.
static OPEN: AtomicBool = AtomicBool::new(false);

pub(crate) fn is_open() -> bool {
    OPEN.load(Ordering::Acquire)
}

/// What the feature threads held back while the panel was hidden goes in as soon as it shows.
/// Queued rather than run inside the signal, which fires from within the setter that changed
/// the property.
fn shown_or_hidden(panel: Pin<&mut qobject::KavvernaPanel>) {
    let open = *panel.panel_open();
    OPEN.store(open, Ordering::Release);
    if !open {
        return;
    }
    let _ = panel.qt_thread().queue(|mut panel| {
        let held = crate::awake_state::get();
        panel.as_mut().apply(held.active, held.remaining);
        crate::mixer_view::publish();
        crate::vitals_view::publish();
        crate::vitals_state::wake();
        crate::agent_usage_state::wake();
    });
}

pub(crate) fn screens() -> Vec<panel_anchor::Screen> {
    SCREENS.lock().map(|held| held.clone()).unwrap_or_default()
}

/// Whether the interface came up: `attach()` runs synchronously inside a successful QML
/// load, so right after the load this answers whether the load worked at all.
pub(crate) fn attached() -> bool {
    PANEL.lock().map(|held| held.is_some()).unwrap_or(false)
}

/// The screen with its size cut down to the measured free area, when one is known.
pub(crate) fn worked(screen: &panel_anchor::Screen) -> panel_anchor::Screen {
    let mut shaped = screen.clone();
    if let Ok(held) = WORK_AREAS.lock() {
        if let Some((_, width, height)) = held.iter().find(|(name, _, _)| *name == screen.name) {
            shaped.width = *width;
            shaped.height = *height;
        }
    }
    shaped
}

pub(crate) fn gap() -> i32 {
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
    /// The panel preset the running hold was started from, in minutes, or 0 when it came
    /// from anywhere else or has since been extended. Only the panel knows which button was
    /// pressed; a remaining time cannot say, since every hold counts down past the others.
    hold_preset: i32,
    /// When that preset was pressed and how long it asked for, so a published hold can be
    /// recognised as the preset's own by where it ends.
    preset_started: Option<(Instant, Duration)>,
    jiggle_available: bool,
    jiggle_status: QString,
    appearance: i32,
    theme_name: QString,
    launch_at_login: bool,
    settings_path: QString,
    version: QString,
    placement: i32,
    panel_on_top: bool,
    panel_width: i32,
    panel_screen: QString,
    at_bottom: bool,
    at_right: bool,
    margin_left: i32,
    margin_top: i32,
    margin_right: i32,
    margin_bottom: i32,
    ghost_visible: bool,
    ghost_screen: QString,
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
            hold_preset: 0,
            preset_started: None,
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
            panel_on_top: settings::bool_at(settings::PANEL_ON_TOP, settings::PANEL_ON_TOP_DEFAULT),
            panel_width: panel_anchor::WIDTH,
            panel_screen: QString::default(),
            at_bottom: true,
            at_right: true,
            margin_left: 0,
            margin_top: 0,
            margin_right: gap(),
            margin_bottom: gap(),
            ghost_visible: false,
            ghost_screen: QString::default(),
            ghost_left: 0,
            ghost_top: 0,
        }
    }
}

/// Where a drag started from. `origin` is the surface's top-left corner in global desktop
/// coordinates, worked out from its placement, and `pressed` is the press in the surface's own
/// coordinates: on Wayland a window never learns its global position, so a global pointer is
/// only ever origin plus a surface-local reading. One drag at a time, on the Qt thread only;
/// the mutex is for the static.
struct DragStart {
    origin: (i32, i32),
    pressed: (i32, i32),
}

static DRAG_FROM: Mutex<Option<DragStart>> = Mutex::new(None);

/// When the interface last closed the panel on its own. A tray click on an ordinary-layer
/// panel takes the focus first, which closes the panel, and only then arrives here over
/// D-Bus asking to toggle: without this the click that meant close would open it again.
static DISMISSED_AT: Mutex<Option<std::time::Instant>> = Mutex::new(None);

fn dismissed_just_now() -> bool {
    DISMISSED_AT
        .lock()
        .ok()
        .and_then(|held| *held)
        .is_some_and(|at| at.elapsed() < Duration::from_millis(250))
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
        // Before anything below can open the panel, so that opening is seen too.
        self.as_mut().on_panel_open_changed(shown_or_hidden).release();
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
        if let Ok(mut held) = DISMISSED_AT.lock() {
            *held = Some(std::time::Instant::now());
        }
        tracing::info!("panel closed");
    }

    fn show_settings(mut self: Pin<&mut Self>, showing: bool) {
        self.as_mut().set_showing_settings(showing);
    }

    /// Switching a hold on starts the default duration, which lights its preset when there is
    /// one; switching it off clears the preset when the release is published.
    fn toggle_awake(mut self: Pin<&mut Self>) {
        if !*self.awake() {
            let minutes = *self.default_minutes();
            self.as_mut().choose_preset(minutes);
        }
        crate::awake_state::toggle();
    }

    fn keep_awake_minutes(mut self: Pin<&mut Self>, minutes: i32) {
        self.as_mut().choose_preset(minutes);
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

    fn extend_minutes(mut self: Pin<&mut Self>, minutes: i32) {
        self.as_mut().choose_preset(0);
        let seconds = u64::try_from(minutes).unwrap_or(0).saturating_mul(60);
        command::send(Command::Extend(Duration::from_secs(seconds)));
    }

    /// Arrives at startup and again whenever a screen joins, leaves or changes shape. A panel
    /// whose screen left moves to the first one still here, keeping its spot on the screen it
    /// lost for when that comes back; the interface remaps it, since the compositor closed the
    /// surface with the output.
    fn report_screens(mut self: Pin<&mut Self>, report: &QString) {
        let seen = panel_anchor::screens_from_report(&report.to_string());
        tracing::info!(screens = seen.len(), "screens reported");
        let previous = self.panel_screen().to_string();
        let before = screens();
        let previous_position =
            before.iter().find(|screen| screen.name == previous).map(|screen| {
                panel_anchor::position_of(
                    &self.read_placement(),
                    &worked(screen),
                    (panel_anchor::WIDTH, panel_anchor::tallest(screen.height)),
                )
            });
        if let Ok(mut held) = SCREENS.lock() {
            *held = seen.clone();
        }
        // A measured free area is only true of the geometry it was measured on.
        if let Ok(mut held) = WORK_AREAS.lock() {
            held.retain(|(name, _, _)| {
                let old = before.iter().find(|screen| screen.name == *name);
                let current = seen.iter().find(|screen| screen.name == *name);
                old.is_some() && old == current
            });
        }

        if !seen.is_empty() && !seen.iter().any(|screen| screen.name == previous) {
            let screen = &seen[0];
            let place = panel_anchor::pinned(
                previous_position.unwrap_or((*self.margin_left(), *self.margin_top())),
                &worked(screen),
                (panel_anchor::WIDTH, panel_anchor::tallest(screen.height)),
                gap(),
            );
            apply_placement(&mut self, place, &screen.name);
        }
        crate::shelf_view::screens_changed();
    }

    fn report_work_area(self: Pin<&mut Self>, name: &QString, width: i32, height: i32) {
        if width <= 0 || height <= 0 {
            return;
        }
        let name = name.to_string();
        if let Ok(mut held) = WORK_AREAS.lock() {
            held.retain(|(known, _, _)| *known != name);
            held.push((name, width, height));
        }
    }

    /// The layer is read when the surface maps, so the interface remaps an open panel the
    /// moment this changes.
    fn choose_panel_on_top(mut self: Pin<&mut Self>, on_top: bool) {
        settings::put_bool(settings::PANEL_ON_TOP, on_top);
        self.as_mut().set_panel_on_top(on_top);
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
    /// The pointer arrives in the surface's own coordinates, the only kind Wayland gives.
    fn drag_begun(
        mut self: Pin<&mut Self>,
        pointer_x: i32,
        pointer_y: i32,
        width: i32,
        height: i32,
    ) {
        let screens = screens();
        let name = self.panel_screen().to_string();
        let Some(screen) = screens.iter().find(|s| s.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let local =
            panel_anchor::position_of(&self.read_placement(), &worked(screen), (width, height));
        if let Ok(mut held) = DRAG_FROM.lock() {
            *held = Some(DragStart {
                origin: (screen.x + local.0, screen.y + local.1),
                pressed: (pointer_x, pointer_y),
            });
        }
        self.as_mut().set_ghost_screen(QString::from(&screen.name));
        self.as_mut().set_ghost_left(local.0);
        self.as_mut().set_ghost_top(local.1);
    }

    fn drag_preview(
        mut self: Pin<&mut Self>,
        pointer_x: i32,
        pointer_y: i32,
        width: i32,
        height: i32,
    ) {
        let Some((origin, pressed)) = DRAG_FROM
            .lock()
            .ok()
            .and_then(|held| held.as_ref().map(|drag| (drag.origin, drag.pressed)))
        else {
            return;
        };
        // A click is not a drag; the ghost only appears once the hand has clearly moved.
        if !*self.ghost_visible()
            && (pointer_x - pressed.0).abs() + (pointer_y - pressed.1).abs() < 8
        {
            return;
        }
        let screens = screens();
        let name = self.ghost_screen().to_string();
        let Some(screen) = screens.iter().find(|s| s.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let local = panel_anchor::dragged_local(origin, pressed, (pointer_x, pointer_y), screen);
        let at = panel_anchor::pinned(local, &worked(screen), (width, height), gap());
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
        let name = self.ghost_screen().to_string();
        let Some(screen) = screens.iter().find(|s| s.name == name).or_else(|| screens.first())
        else {
            return;
        };
        let landed = panel_anchor::pinned(
            (*self.ghost_left(), *self.ghost_top()),
            &worked(screen),
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
        let shaped = worked(screen);
        let (x, y) = panel_anchor::position_of(&self.read_placement(), &shaped, (width, height));
        let pinned = panel_anchor::pinned((x, y), &shaped, (width, height), gap());
        let name = screen.name.clone();
        let entries = settings::texts_at(settings::PLACEMENT_REMEMBERED).unwrap_or_default();
        settings::put_texts(
            settings::PLACEMENT_REMEMBERED,
            &panel_anchor::with_position(entries, &name, pinned.left, pinned.top),
        );
        apply_placement(&mut self, pinned, &name);
    }

    /// Lights the preset that started the hold, or none for 0. Nothing is sent from here: the
    /// caller starts or changes the hold itself.
    fn choose_preset(mut self: Pin<&mut Self>, minutes: i32) {
        let length = Duration::from_secs(u64::try_from(minutes).unwrap_or(0).saturating_mul(60));
        self.as_mut().rust_mut().preset_started = (minutes > 0).then(|| (Instant::now(), length));
        self.as_mut().set_hold_preset(minutes.max(0));
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
        if !still_the_preset(self.rust().preset_started, active, remaining, Instant::now()) {
            self.as_mut().choose_preset(0);
        }
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
                let size = (panel_anchor::WIDTH, panel_anchor::tallest(screen.height));
                (panel_anchor::pinned((x, y), &worked(screen), size, spacing), screen.name.clone())
            })
        }
        2 => None,
        _ => beside_tray_or_remembered_anchor(click, &screens, spacing),
    };

    match chosen {
        Some((placement, screen)) => apply_placement(panel, placement, &screen),
        None => match screens.first() {
            Some(screen) => apply_placement(panel, panel_anchor::corner(spacing), &screen.name),
            None => apply_placement(panel, panel_anchor::corner(spacing), ""),
        },
    }
}

/// The exact last click, for the opens later in this session that carry none. The stored copy
/// only has to survive a restart, so it is rewritten when the click lands somewhere else rather
/// than on every click, each of which hits the icon a pixel or two from the last.
static LAST_CLICK: Mutex<Option<String>> = Mutex::new(None);

/// Wider than any tray icon Plasma draws, so clicks on the same icon count as the same place.
const ICON_SLACK: i32 = 48;

fn moved_off_the_icon(stored: &str, clicked: &str) -> bool {
    match (panel_anchor::parse(stored), panel_anchor::parse(clicked)) {
        (Some((was, x0, y0)), Some((now, x1, y1))) => {
            was != now || (x1 - x0).abs() > ICON_SLACK || (y1 - y0).abs() > ICON_SLACK
        }
        _ => true,
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
            let clicked = panel_anchor::entry(&screen.name, point.0, point.1);
            if moved_off_the_icon(&settings::text_at(settings::PLACEMENT_ANCHOR, ""), &clicked) {
                settings::put_text(settings::PLACEMENT_ANCHOR, &clicked);
            }
            if let Ok(mut held) = LAST_CLICK.lock() {
                *held = Some(clicked);
            }
            return Some((panel_anchor::beside_tray(point, screen, spacing), screen.name.clone()));
        }
        tracing::info!(?point, "tray click outside every screen, treated as absent");
    }

    let stored = LAST_CLICK
        .lock()
        .ok()
        .and_then(|held| held.clone())
        .unwrap_or_else(|| settings::text_at(settings::PLACEMENT_ANCHOR, ""));
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

/// How long after a preset is pressed a publish may still describe the hold from before it:
/// the awake thread can have sent one just before it read the new command.
const PRESET_SETTLING: Duration = Duration::from_secs(2);
/// How far a hold's end may sit from where its preset put it and still be that preset's hold.
const PRESET_DRIFT: Duration = Duration::from_secs(3);

/// Whether the published hold is still the one a preset started. A hold that ends somewhere
/// else was extended, or replaced from the tray or the remote interface, and no preset stands
/// for it any more.
fn still_the_preset(
    started: Option<(Instant, Duration)>,
    active: bool,
    remaining: Option<Duration>,
    now: Instant,
) -> bool {
    let Some((pressed, length)) = started else {
        return false;
    };
    let settling = now.saturating_duration_since(pressed) < PRESET_SETTLING;
    let ends_where_it_should = active
        && remaining.is_some_and(|left| {
            let wanted = pressed + length;
            let ends = now + left;
            ends.saturating_duration_since(wanted).max(wanted.saturating_duration_since(ends))
                <= PRESET_DRIFT
        });
    ends_where_it_should || settling
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
        if !open && dismissed_just_now() {
            tracing::info!("tray click arrived after the focus it took had closed the panel");
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn another_click_on_the_same_icon_is_not_stored_again() {
        let stored = panel_anchor::entry("DP-1", 1850, 1060);

        assert!(!moved_off_the_icon(&stored, &panel_anchor::entry("DP-1", 1853, 1058)));
    }

    #[test]
    fn a_click_somewhere_else_or_on_another_screen_is_stored() {
        let stored = panel_anchor::entry("DP-1", 1850, 1060);

        assert!(moved_off_the_icon(&stored, &panel_anchor::entry("DP-1", 1700, 1060)));
        assert!(moved_off_the_icon(&stored, &panel_anchor::entry("HDMI-A-1", 1850, 1060)));
        assert!(moved_off_the_icon("", &panel_anchor::entry("DP-1", 1850, 1060)));
    }

    const HOUR: Duration = Duration::from_secs(3600);

    #[test]
    fn a_hold_ending_where_its_preset_put_it_keeps_the_preset() {
        let pressed = Instant::now();
        let later = pressed + Duration::from_secs(600);
        let left = HOUR - Duration::from_secs(600);

        assert!(still_the_preset(Some((pressed, HOUR)), true, Some(left), later));
    }

    #[test]
    fn an_extended_hold_is_no_preset_of_ours() {
        let pressed = Instant::now();
        let later = pressed + Duration::from_secs(600);
        let left = HOUR - Duration::from_secs(600) + Duration::from_secs(900);

        assert!(!still_the_preset(Some((pressed, HOUR)), true, Some(left), later));
    }

    #[test]
    fn the_hold_from_before_the_press_is_forgiven_while_the_press_settles() {
        let pressed = Instant::now();
        let moments_later = pressed + Duration::from_millis(300);

        assert!(still_the_preset(Some((pressed, HOUR)), false, None, moments_later));
        assert!(still_the_preset(Some((pressed, HOUR)), true, Some(2 * HOUR), moments_later));
    }

    #[test]
    fn a_released_hold_clears_the_preset_once_settled() {
        let pressed = Instant::now();
        let later = pressed + Duration::from_secs(10);

        assert!(!still_the_preset(Some((pressed, HOUR)), false, None, later));
    }

    #[test]
    fn a_hold_with_no_end_has_no_preset() {
        let pressed = Instant::now();
        let later = pressed + Duration::from_secs(10);

        assert!(!still_the_preset(Some((pressed, HOUR)), true, None, later));
        assert!(!still_the_preset(None, true, Some(HOUR), later));
    }
}
