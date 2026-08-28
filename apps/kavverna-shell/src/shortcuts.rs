use crate::panel;
use kde_bridge::shortcuts::{ALT, CONTROL, Shortcut};

const COMPONENT: &str = "kavverna";
const FRIENDLY: &str = "Kavverna";

const SHOW_CLIPBOARD: &str = "show-clipboard";

/// Ctrl+Alt+V rather than the reference app's four-key combination: it is the shape a Linux
/// user already reaches for, and Plasma leaves it free.
const SHORTCUTS: &[Shortcut] = &[Shortcut {
    action: SHOW_CLIPBOARD,
    friendly: "Show clipboard history",
    keys: CONTROL | ALT | b'V' as i32,
}];

pub fn serve(runtime: tokio::runtime::Handle) {
    let (presses, incoming) = std::sync::mpsc::channel();

    runtime.spawn(async move {
        if let Err(err) =
            kde_bridge::shortcuts::serve(COMPONENT, FRIENDLY, SHORTCUTS, presses).await
        {
            tracing::error!(%err, "no global shortcuts");
        }
    });

    std::thread::spawn(move || {
        for action in incoming {
            tracing::info!(%action, "global shortcut");
            match action.as_str() {
                SHOW_CLIPBOARD => panel::open_page("clipboard"),
                other => tracing::warn!(action = other, "a shortcut fired that nothing answers"),
            }
        }
    });
}
