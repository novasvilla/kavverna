use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;
use ksni::{MenuItem, ToolTip, Tray};

pub struct StatusIcon {
    pub status_line: String,
}

impl Tray for StatusIcon {
    fn id(&self) -> String {
        "dev.kavverna.shell".into()
    }

    fn title(&self) -> String {
        "Kavverna".into()
    }

    fn icon_name(&self) -> String {
        "applications-utilities".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Kavverna".into(),
            description: self.status_line.clone(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: self.status_line.clone(),
                enabled: false,
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
pub fn show(status_line: String) -> Option<Handle<StatusIcon>> {
    match (StatusIcon { status_line }).spawn() {
        Ok(handle) => Some(handle),
        Err(err) => {
            tracing::warn!(%err, "no StatusNotifierItem host, running without a tray icon");
            None
        }
    }
}
