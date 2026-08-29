use crate::command::{self, Command};
use crate::settings;
use crate::{app_icon, mixer_state, panel};
use feature_catalog::Feature;
use keep_awake::{Hold, Scope, format_compact, format_duration};
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{MenuItem, ToolTip, Tray};
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

#[derive(Default)]
pub struct StatusIcon {
    pub awake: bool,
    pub remaining: Option<Duration>,
}

/// Read from settings rather than held in the icon, so the tray, the panel and the remote
/// interface cannot disagree about it.
pub fn configured_scope() -> Scope {
    if settings::bool_at(settings::ALLOW_DISPLAY_SLEEP, settings::ALLOW_DISPLAY_SLEEP_DEFAULT) {
        Scope::SystemOnly
    } else {
        Scope::SystemAndDisplay
    }
}

impl StatusIcon {
    fn summary(&self) -> String {
        match (self.awake, self.remaining) {
            (false, _) => "Sleep allowed".into(),
            (true, Some(left)) => format!("Awake for {}", format_duration(left)),
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
                            configured_scope(),
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
                enabled: self.remaining.is_some(),
                submenu: extensions,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: if self.awake { "Allow sleep now" } else { "Keep awake" }.into(),
                activate: Box::new(|icon: &mut Self| {
                    let command = if icon.awake {
                        Command::Release
                    } else {
                        Command::Engage(settings::default_hold(), configured_scope())
                    };
                    command::send(command);
                }),
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
        let description = if self.awake {
            format!("{}  ({})", self.summary(), format_compact(self.remaining))
        } else {
            self.summary()
        };

        ToolTip { title: "Kavverna".into(), description, ..Default::default() }
    }

    /// Middle click rather than right: on a StatusNotifierItem the host owns the right
    /// button for the context menu, so it never reaches us.
    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        if !settings::bool_at(settings::MIDDLE_CLICK_TOGGLE, settings::MIDDLE_CLICK_TOGGLE_DEFAULT)
        {
            return;
        }

        let command = if self.awake {
            Command::Release
        } else {
            Command::Engage(Hold::Indefinite, configured_scope())
        };
        command::send(command);
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        panel::toggle();
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

/// Returns at once and keeps asking in the background. Blocking here would hold the interface
/// back on exactly the session where the panel is still starting, and one attempt would leave
/// the application running with no way to reach it.
pub fn show() -> TrayIcon {
    const ATTEMPTS: u32 = 60;
    const BETWEEN: std::time::Duration = std::time::Duration::from_millis(500);

    let icon: TrayIcon = std::sync::Arc::new(std::sync::Mutex::new(None));
    let filling = std::sync::Arc::clone(&icon);

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
