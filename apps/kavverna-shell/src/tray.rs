use keep_awake::{Hold, Scope, format_duration};
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
use ksni::{MenuItem, ToolTip, Tray};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub enum Command {
    Engage(Hold, Scope),
    Release,
}

const DURATIONS: [(&str, u64); 5] = [
    ("15 minutes", 15 * 60),
    ("30 minutes", 30 * 60),
    ("1 hour", 60 * 60),
    ("2 hours", 2 * 60 * 60),
    ("4 hours", 4 * 60 * 60),
];

pub struct StatusIcon {
    pub awake: bool,
    pub remaining: Option<Duration>,
    pub allow_display_sleep: bool,
    pub commands: Sender<Command>,
}

impl StatusIcon {
    fn scope(&self) -> Scope {
        if self.allow_display_sleep { Scope::SystemOnly } else { Scope::SystemAndDisplay }
    }

    fn summary(&self) -> String {
        match (self.awake, self.remaining) {
            (false, _) => "Sleep allowed".into(),
            (true, Some(left)) => format!("Awake for {}", format_duration(left)),
            (true, None) => "Awake until switched off".into(),
        }
    }
}

impl Tray for StatusIcon {
    fn id(&self) -> String {
        "dev.kavverna.shell".into()
    }

    fn title(&self) -> String {
        "Kavverna".into()
    }

    fn icon_name(&self) -> String {
        if self.awake { "caffeine-cup-full".into() } else { "caffeine-cup-empty".into() }
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Kavverna".into(),
            description: self.summary(),
            ..Default::default()
        }
    }

    /// Left click is the fast path: switch off if awake, otherwise hold indefinitely.
    fn activate(&mut self, _x: i32, _y: i32) {
        let command = if self.awake {
            Command::Release
        } else {
            Command::Engage(Hold::Indefinite, self.scope())
        };
        let _ = self.commands.send(command);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut timed: Vec<MenuItem<Self>> = DURATIONS
            .iter()
            .map(|&(label, seconds)| {
                StandardItem {
                    label: label.into(),
                    activate: Box::new(move |icon: &mut Self| {
                        let _ = icon.commands.send(Command::Engage(
                            Hold::For(Duration::from_secs(seconds)),
                            icon.scope(),
                        ));
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();

        timed.push(MenuItem::Separator);
        timed.push(
            StandardItem {
                label: "Until I switch it off".into(),
                activate: Box::new(|icon: &mut Self| {
                    let _ = icon
                        .commands
                        .send(Command::Engage(Hold::Indefinite, icon.scope()));
                }),
                ..Default::default()
            }
            .into(),
        );

        vec![
            StandardItem { label: self.summary(), enabled: false, ..Default::default() }.into(),
            MenuItem::Separator,
            SubMenu { label: "Keep awake for".into(), submenu: timed, ..Default::default() }.into(),
            CheckmarkItem {
                label: "Let displays sleep".into(),
                checked: self.allow_display_sleep,
                activate: Box::new(|icon: &mut Self| {
                    icon.allow_display_sleep = !icon.allow_display_sleep;
                    if icon.awake {
                        let hold = icon
                            .remaining
                            .map(Hold::For)
                            .unwrap_or(Hold::Indefinite);
                        let _ = icon.commands.send(Command::Engage(hold, icon.scope()));
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Allow sleep now".into(),
                enabled: self.awake,
                activate: Box::new(|icon: &mut Self| {
                    let _ = icon.commands.send(Command::Release);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Fails when no StatusNotifierItem host is running, which on a bare session is expected
/// rather than fatal.
pub fn show(commands: Sender<Command>) -> Option<Handle<StatusIcon>> {
    let icon = StatusIcon {
        awake: false,
        remaining: None,
        allow_display_sleep: true,
        commands,
    };

    match icon.spawn() {
        Ok(handle) => Some(handle),
        Err(err) => {
            tracing::warn!(%err, "no StatusNotifierItem host, running without a tray icon");
            None
        }
    }
}
