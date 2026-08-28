//! Global shortcuts through KGlobalAccel.
//!
//! The only route with a real grab on Wayland, and it puts the shortcut in System Settings
//! beside every other one, so conflicts are the desktop's problem rather than ours.

use futures_util::StreamExt;
use std::sync::mpsc::Sender;

/// Qt keeps modifiers in the high bits of the same integer as the key.
pub const SHIFT: i32 = 0x0200_0000;
pub const CONTROL: i32 = 0x0400_0000;
pub const ALT: i32 = 0x0800_0000;
pub const META: i32 = 0x1000_0000;

/// Registers the key below as the default and then lets the desktop's own store win, so a
/// binding somebody changed in System Settings survives the next launch. Adding NoAutoloading
/// here would quietly put ours back every time the application starts.
const SET_AS_THE_DEFAULT: u32 = 2;

pub struct Shortcut {
    pub action: &'static str,
    pub friendly: &'static str,
    pub keys: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum ShortcutError {
    #[error("no session bus: {0}")]
    Bus(#[from] zbus::Error),
    #[error("the desktop offers no global shortcuts")]
    Unavailable,
}

#[zbus::proxy(
    interface = "org.kde.KGlobalAccel",
    default_service = "org.kde.kglobalaccel",
    default_path = "/kglobalaccel"
)]
trait KGlobalAccel {
    // KDE spells these in camelCase, which is not what the macro would derive from the Rust
    // name, and a mismatch shows up only at run time as UnknownMethod.
    #[zbus(name = "doRegister")]
    fn do_register(&self, action_id: &[&str]) -> zbus::Result<()>;
    #[zbus(name = "setShortcut")]
    fn set_shortcut(&self, action_id: &[&str], keys: &[i32], flags: u32) -> zbus::Result<Vec<i32>>;
    #[zbus(name = "getComponent")]
    fn get_component(&self, unique_name: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.kde.kglobalaccel.Component",
    default_service = "org.kde.kglobalaccel"
)]
trait Component {
    #[zbus(signal, name = "globalShortcutPressed")]
    fn global_shortcut_pressed(
        &self,
        component: String,
        action: String,
        timestamp: i64,
    ) -> zbus::Result<()>;
}

/// Registers every shortcut and then reports each press by action name until the bus goes away.
pub async fn serve(
    component: &str,
    friendly: &str,
    shortcuts: &[Shortcut],
    presses: Sender<String>,
) -> Result<(), ShortcutError> {
    let connection = zbus::Connection::session().await?;
    let accel =
        KGlobalAccelProxy::new(&connection).await.map_err(|_| ShortcutError::Unavailable)?;

    for shortcut in shortcuts {
        let action_id = [component, shortcut.action, friendly, shortcut.friendly];
        accel.do_register(&action_id).await?;

        match accel.set_shortcut(&action_id, &[shortcut.keys], SET_AS_THE_DEFAULT).await {
            Ok(given) if given.first() == Some(&shortcut.keys) => {
                tracing::info!(action = shortcut.action, "global shortcut registered")
            }
            // The desktop hands back what it actually assigned, so a key the user changed or
            // one already taken comes back different rather than as an error.
            Ok(given) => tracing::info!(
                action = shortcut.action,
                ?given,
                "the desktop had its own key for this and kept it"
            ),
            Err(err) => tracing::error!(%err, action = shortcut.action, "shortcut refused"),
        }
    }

    let path = accel.get_component(component).await?;
    let component = ComponentProxy::builder(&connection).path(path)?.build().await?;
    let mut pressed = component.receive_global_shortcut_pressed().await?;

    while let Some(signal) = pressed.next().await {
        let Ok(args) = signal.args() else {
            continue;
        };
        if presses.send(args.action.to_string()).is_err() {
            return Ok(());
        }
    }

    Ok(())
}
