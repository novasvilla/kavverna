use crate::awake_state;
use crate::command::{self, Command};
use crate::{cup_icon, panel};
use keep_awake::{Hold, Scope, format_duration};
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{MenuItem, ToolTip, Tray};
use std::time::Duration;

const DURATIONS: [(&str, u64); 5] = [
    ("15 minutes", 15),
    ("30 minutes", 30),
    ("1 hour", 60),
    ("2 hours", 120),
    ("4 hours", 240),
];

#[derive(Default)]
pub struct StatusIcon {
    pub awake: bool,
    pub remaining: Option<Duration>,
}

impl StatusIcon {
    fn summary(&self) -> String {
        match (self.awake, self.remaining) {
            (false, _) => "Sleep allowed".into(),
            (true, Some(left)) => format!("Awake for {}", format_duration(left)),
            (true, None) => "Awake until switched off".into(),
        }
    }

    fn scope() -> Scope {
        if awake_state::get().allow_display_sleep {
            Scope::SystemOnly
        } else {
            Scope::SystemAndDisplay
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

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        cup_icon::cup(self.awake)
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Kavverna".into(),
            description: self.summary(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        panel::open();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let timed = DURATIONS
            .iter()
            .map(|&(label, minutes)| {
                StandardItem {
                    label: label.into(),
                    activate: Box::new(move |_: &mut Self| {
                        command::send(Command::Engage(
                            Hold::For(Duration::from_secs(minutes * 60)),
                            Self::scope(),
                        ));
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();

        vec![
            StandardItem {
                label: "Open Kavverna".into(),
                activate: Box::new(|_| panel::open()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem { label: self.summary(), enabled: false, ..Default::default() }.into(),
            SubMenu { label: "Keep awake for".into(), submenu: timed, ..Default::default() }.into(),
            StandardItem {
                label: if self.awake { "Allow sleep now" } else { "Keep awake" }.into(),
                activate: Box::new(|icon: &mut Self| {
                    let command = if icon.awake {
                        Command::Release
                    } else {
                        Command::Engage(Hold::Indefinite, Self::scope())
                    };
                    command::send(command);
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
pub fn show() -> Option<Handle<StatusIcon>> {
    match StatusIcon::default().spawn() {
        Ok(handle) => Some(handle),
        Err(err) => {
            tracing::warn!(%err, "no StatusNotifierItem host, running without a tray icon");
            None
        }
    }
}
