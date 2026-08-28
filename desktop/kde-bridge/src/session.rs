//! The two moments when leaving something on the clipboard stops being a convenience.

use futures_util::StreamExt;
use std::sync::mpsc::Sender;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionEvent {
    AboutToSuspend,
    ScreenLocked,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("no session bus: {0}")]
    Session(zbus::Error),
    #[error("no system bus: {0}")]
    System(zbus::Error),
}

/// logind lives on the system bus. Reaching for it on the session bus is silent and does
/// nothing, which is a mistake this project has already made once with keep awake.
#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Login1 {
    #[zbus(signal)]
    fn prepare_for_sleep(&self, going_to_sleep: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/ScreenSaver"
)]
trait ScreenSaver {
    #[zbus(signal)]
    fn active_changed(&self, active: bool) -> zbus::Result<()>;
}

pub async fn watch(events: Sender<SessionEvent>) -> Result<(), SessionError> {
    let system = zbus::Connection::system().await.map_err(SessionError::System)?;
    let session = zbus::Connection::session().await.map_err(SessionError::Session)?;

    let logind = Login1Proxy::new(&system).await.map_err(SessionError::System)?;
    let saver = ScreenSaverProxy::new(&session).await.map_err(SessionError::Session)?;

    let sleeping = logind
        .receive_prepare_for_sleep()
        .await
        .map_err(SessionError::System)?
        // The same signal announces waking up, and coming back is not a reason to wipe
        // what someone copied before locking the machine.
        .filter_map(|signal| async move {
            signal.args().ok()?.going_to_sleep.then_some(SessionEvent::AboutToSuspend)
        });

    let locking = saver
        .receive_active_changed()
        .await
        .map_err(SessionError::Session)?
        .filter_map(|signal| async move {
            signal.args().ok()?.active.then_some(SessionEvent::ScreenLocked)
        });

    let mut both = std::pin::pin!(futures_util::stream::select(sleeping, locking));
    while let Some(event) = both.next().await {
        if events.send(event).is_err() {
            return Ok(());
        }
    }

    Ok(())
}
