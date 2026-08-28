//! The two moments when leaving something on the clipboard stops being a convenience.

use futures_util::StreamExt;
use std::sync::mpsc::Sender;
use zbus::zvariant::OwnedFd;

/// logind's delay lock, held while the clipboard is emptied. Suspend is held off until this is
/// dropped, and dropping it is the only thing that lets the machine go, so it is handed to
/// whoever does the work rather than released here.
#[derive(Debug)]
pub struct SleepDelay(#[allow(dead_code)] Option<OwnedFd>);

#[derive(Debug)]
pub enum SessionEvent {
    AboutToSuspend(SleepDelay),
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
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;

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

const WHO: &str = "Kavverna";
const WHY: &str = "Emptying the clipboard before the machine sleeps";

/// `delay`, never `block`: this asks for the few seconds logind allows before suspending, and
/// refusing the suspend outright is not what auto clear is for. Taken only when it will be used,
/// since an inhibitor held for nothing still shows up in everybody's `systemd-inhibit --list`.
async fn take_delay(logind: &Login1Proxy<'_>, wanted: &(impl Fn() -> bool + Sync)) -> SleepDelay {
    if !wanted() {
        return SleepDelay(None);
    }
    match logind.inhibit("sleep", WHO, WHY, "delay").await {
        Ok(lock) => SleepDelay(Some(lock)),
        Err(err) => {
            // Worth continuing without: the clipboard still clears, it just races the suspend.
            tracing::warn!(%err, "no delay lock, so clearing races the suspend");
            SleepDelay(None)
        }
    }
}

/// `clears_on_suspend` is asked again each time the lock is taken, so switching the setting on
/// takes effect at the next wake rather than needing a restart.
pub async fn watch(
    events: Sender<SessionEvent>,
    clears_on_suspend: impl Fn() -> bool + Sync,
) -> Result<(), SessionError> {
    let system = zbus::Connection::system().await.map_err(SessionError::System)?;
    let session = zbus::Connection::session().await.map_err(SessionError::Session)?;

    let logind = Login1Proxy::new(&system).await.map_err(SessionError::System)?;
    let saver = ScreenSaverProxy::new(&session).await.map_err(SessionError::Session)?;

    let mut sleeping = logind.receive_prepare_for_sleep().await.map_err(SessionError::System)?;
    let mut locking = saver.receive_active_changed().await.map_err(SessionError::Session)?;

    let mut delay = take_delay(&logind, &clears_on_suspend).await;

    loop {
        tokio::select! {
            Some(signal) = sleeping.next() => {
                let Ok(args) = signal.args() else { continue };
                if args.going_to_sleep {
                    // The lock goes with the event. Suspend waits until the far end drops it,
                    // which is after the clipboard has actually been emptied.
                    if events.send(SessionEvent::AboutToSuspend(delay)).is_err() {
                        return Ok(());
                    }
                    delay = SleepDelay(None);
                } else {
                    // Waking up. The old lock went with the suspend, so this takes the next one.
                    delay = take_delay(&logind, &clears_on_suspend).await;
                }
            }
            Some(signal) = locking.next() => {
                let Ok(args) = signal.args() else { continue };
                if args.active && events.send(SessionEvent::ScreenLocked).is_err() {
                    return Ok(());
                }
            }
            else => return Ok(()),
        }
    }
}
