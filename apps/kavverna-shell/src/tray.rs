use crate::command::{self, Command};
use crate::settings;
use crate::{app_icon, awake_state, mixer_state, panel};
use feature_catalog::Feature;
use keep_awake::Hold;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{MenuItem, ToolTip, Tray};
use std::sync::OnceLock;
use std::time::Duration;

const DURATIONS: [(&str, u64); 6] = [
    ("15 minutes", 15),
    ("30 minutes", 30),
    ("1 hour", 60),
    ("2 hours", 120),
    ("4 hours", 240),
    ("8 hours", 480),
];

const EXTENSIONS: [(&str, u64); 3] = [("15 minutes", 15), ("30 minutes", 30), ("1 hour", 60)];

/// Everything the tray draws, and nothing finer. Every update makes ksni rebuild each property
/// and the whole menu and redraw both icon sizes, so the tray is updated only when this value
/// changes: a hold changing, or a timed hold crossing into another minute.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StatusIcon {
    pub awake: bool,
    /// Whole minutes left, never below one while any time is, so the last minute reads as one
    /// rather than as nothing.
    pub minutes_left: Option<u64>,
}

impl StatusIcon {
    pub fn showing(awake: bool, remaining: Option<Duration>) -> Self {
        Self {
            awake,
            minutes_left: remaining.filter(|_| awake).map(|left| (left.as_secs() / 60).max(1)),
        }
    }

    fn summary(&self) -> String {
        match (self.awake, self.minutes_left) {
            (false, _) => "Sleep allowed".into(),
            (true, Some(minutes)) if minutes >= 60 => {
                format!("Awake for {}h {:02}m", minutes / 60, minutes % 60)
            }
            (true, Some(minutes)) => format!("Awake for {minutes}m"),
            (true, None) => "Awake until switched off".into(),
        }
    }
}

impl StatusIcon {
    fn keep_awake_items(&self) -> Vec<MenuItem<Self>> {
        let timed = DURATIONS
            .iter()
            .map(|&(label, minutes)| {
                StandardItem {
                    label: label.into(),
                    activate: Box::new(move |_: &mut Self| {
                        command::send(Command::Engage(
                            Hold::For(Duration::from_secs(minutes * 60)),
                            settings::scope(),
                        ));
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();

        let extensions = EXTENSIONS
            .iter()
            .map(|&(label, minutes)| {
                StandardItem {
                    label: format!("+ {label}"),
                    activate: Box::new(move |_: &mut Self| {
                        command::send(Command::Extend(Duration::from_secs(minutes * 60)));
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();

        vec![
            StandardItem { label: self.summary(), enabled: false, ..Default::default() }.into(),
            SubMenu { label: "Keep awake for".into(), submenu: timed, ..Default::default() }.into(),
            SubMenu {
                label: "Add more time".into(),
                enabled: self.minutes_left.is_some(),
                submenu: extensions,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: if self.awake { "Allow sleep now" } else { "Keep awake" }.into(),
                activate: Box::new(|_: &mut Self| awake_state::toggle()),
                ..Default::default()
            }
            .into(),
        ]
    }

    /// The two sound actions worth reaching without opening anything. Both need a live mixer,
    /// so neither is offered when there is nothing to act on.
    fn sound_items(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = Vec::new();
        if !mixer_state::is_running() {
            return items;
        }

        if settings::is_installed(Feature::MicrophoneTools) {
            let muted = mixer_state::every_input_muted();
            items.push(
                StandardItem {
                    label: if muted { "Unmute microphones" } else { "Mute every microphone" }
                        .into(),
                    icon_name: if muted {
                        "audio-input-microphone"
                    } else {
                        "microphone-sensitivity-muted"
                    }
                    .into(),
                    activate: Box::new(move |_: &mut Self| mixer_state::mute_every_input(!muted)),
                    ..Default::default()
                }
                .into(),
            );
        }

        if settings::is_installed(Feature::OutputSwitcher) {
            items.push(
                StandardItem {
                    label: "Next sound output".into(),
                    icon_name: "audio-headphones".into(),
                    activate: Box::new(|_: &mut Self| {
                        mixer_state::cycle_output();
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }

        items
    }
}

impl Tray for StatusIcon {
    fn id(&self) -> String {
        "dev.kavverna.shell".into()
    }

    fn title(&self) -> String {
        "Kavverna".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        app_icon::mark(self.awake)
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip { title: "Kavverna".into(), description: self.summary(), ..Default::default() }
    }

    /// Middle click rather than right: on a StatusNotifierItem the host owns the right
    /// button for the context menu, so it never reaches us.
    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        if !settings::bool_at(settings::MIDDLE_CLICK_TOGGLE, settings::MIDDLE_CLICK_TOGGLE_DEFAULT)
        {
            return;
        }

        awake_state::toggle();
    }

    /// The host passes where the icon was clicked, and on Plasma those are real screen
    /// coordinates: the only way a Wayland client ever learns where its own icon is.
    fn activate(&mut self, x: i32, y: i32) {
        panel::toggle_at(x, y);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = vec![
            StandardItem {
                label: "Open Kavverna".into(),
                activate: Box::new(|_| panel::toggle()),
                ..Default::default()
            }
            .into(),
        ];

        if settings::is_installed(Feature::KeepAwake) {
            items.push(MenuItem::Separator);
            items.extend(self.keep_awake_items());
        }

        let sound = self.sound_items();
        if !sound.is_empty() {
            items.push(MenuItem::Separator);
            items.extend(sound);
        }

        if settings::is_installed(Feature::ClipboardHistory) {
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: "Clipboard history".into(),
                    icon_name: "edit-paste".into(),
                    activate: Box::new(|_| panel::open_page("clipboard")),
                    ..Default::default()
                }
                .into(),
            );
        }

        if settings::is_installed(Feature::Shelf) {
            items.push(
                StandardItem {
                    label: "Shelf".into(),
                    icon_name: "mail-attachment".into(),
                    activate: Box::new(|_| crate::shelf_view::toggle()),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Settings".into(),
                icon_name: "configure".into(),
                activate: Box::new(|_| panel::open_settings()),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

/// Filled in as soon as a host answers, which on a session started from autostart is usually
/// not on the first try.
pub type TrayIcon = std::sync::Arc<std::sync::Mutex<Option<Handle<StatusIcon>>>>;

/// For the parts of the menu that read other features' state when it is built, which ksni
/// keeps until the next update.
static SHOWN: OnceLock<TrayIcon> = OnceLock::new();

/// Rebuilds the menu without changing what the icon says. Must not be called with the mixer
/// snapshot or the settings locked, since building the menu takes both. False when no tray has
/// answered yet, so the caller can try again later.
pub fn refresh() -> bool {
    let handle = SHOWN.get().and_then(|icon| icon.lock().ok().and_then(|held| held.clone()));
    handle.and_then(|handle| handle.update(|_: &mut StatusIcon| {})).is_some()
}

/// Returns at once and keeps asking in the background. Blocking here would hold the interface
/// back on exactly the session where the panel is still starting, and one attempt would leave
/// the application running with no way to reach it.
pub fn show() -> TrayIcon {
    const ATTEMPTS: u32 = 60;
    const BETWEEN: std::time::Duration = std::time::Duration::from_millis(500);

    let icon: TrayIcon = std::sync::Arc::new(std::sync::Mutex::new(None));
    let filling = std::sync::Arc::clone(&icon);
    let _ = SHOWN.set(std::sync::Arc::clone(&icon));

    std::thread::spawn(move || {
        for attempt in 1..=ATTEMPTS {
            match StatusIcon::default().spawn() {
                Ok(handle) => {
                    if let Ok(mut held) = filling.lock() {
                        *held = Some(handle);
                    }
                    if attempt > 1 {
                        tracing::info!(attempt, "the tray host answered eventually");
                    }
                    return;
                }
                Err(err) if attempt == ATTEMPTS => {
                    tracing::warn!(%err, "no StatusNotifierItem host, running without a tray icon");
                }
                Err(_) => std::thread::sleep(BETWEEN),
            }
        }
    });

    icon
}

#[cfg(test)]
mod tests {
    use super::*;

    fn left(seconds: u64) -> Option<Duration> {
        Some(Duration::from_secs(seconds))
    }

    #[test]
    fn seconds_ticking_within_a_minute_change_nothing_the_tray_shows() {
        assert_eq!(
            StatusIcon::showing(true, left(12 * 60 + 59)),
            StatusIcon::showing(true, left(12 * 60))
        );
    }

    #[test]
    fn the_minute_rolling_over_changes_what_the_tray_shows() {
        assert_ne!(
            StatusIcon::showing(true, left(12 * 60)),
            StatusIcon::showing(true, left(12 * 60 - 1))
        );
    }

    #[test]
    fn a_hold_starting_or_ending_changes_what_the_tray_shows() {
        assert_ne!(StatusIcon::showing(false, None), StatusIcon::showing(true, None));
        assert_ne!(StatusIcon::showing(true, None), StatusIcon::showing(true, left(600)));
    }

    #[test]
    fn hours_read_with_their_minutes() {
        assert_eq!(
            StatusIcon::showing(true, left(2 * 3600 + 10 * 60 + 30)).summary(),
            "Awake for 2h 10m"
        );
    }

    #[test]
    fn the_last_minute_still_reads_as_one() {
        assert_eq!(StatusIcon::showing(true, left(20)).minutes_left, Some(1));
        assert_eq!(StatusIcon::showing(true, left(20)).summary(), "Awake for 1m");
    }
}
