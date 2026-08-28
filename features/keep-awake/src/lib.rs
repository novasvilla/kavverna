//! Holding off automatic sleep, on a timer or until told otherwise.

use std::time::{Duration, Instant};
use zbus::zvariant::OwnedFd;

mod inhibitor;

use inhibitor::{Login1ManagerProxy, ScreenSaverProxy};

const WHO: &str = "Kavverna";

#[derive(Debug, thiserror::Error)]
pub enum KeepAwakeError {
    #[error("could not reach the session bus: {0}")]
    Bus(#[from] zbus::Error),
}

type Result<T> = std::result::Result<T, KeepAwakeError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    Indefinite,
    For(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Blocks automatic idle suspend only. Displays sleep as usual and local work carries
    /// on, which is the common case for a long download or build.
    SystemOnly,
    /// Also holds off screen blanking and locking.
    SystemAndDisplay,
}

impl Scope {
    /// `idle` alone, never `idle:sleep`: blocking `sleep` would also swallow a deliberate
    /// suspend from the power menu, which is not what "keep awake" should mean.
    const LOGIND_WHAT: &'static str = "idle";
}

struct ActiveHold {
    /// Releasing the inhibitor is exactly dropping this descriptor.
    _logind: OwnedFd,
    screen_saver_cookie: Option<u32>,
    expires_at: Option<Instant>,
    scope: Scope,
}

pub struct KeepAwake {
    /// logind is a system service; the screen saver belongs to the session.
    system: zbus::Connection,
    session: zbus::Connection,
    hold: Option<ActiveHold>,
}

impl KeepAwake {
    pub async fn connect() -> Result<Self> {
        Ok(Self {
            system: zbus::Connection::system().await?,
            session: zbus::Connection::session().await?,
            hold: None,
        })
    }

    pub fn is_active(&self) -> bool {
        self.hold.is_some()
    }

    pub fn scope(&self) -> Option<Scope> {
        self.hold.as_ref().map(|hold| hold.scope)
    }

    pub fn remaining(&self) -> Option<Duration> {
        let expires_at = self.hold.as_ref()?.expires_at?;
        Some(expires_at.saturating_duration_since(Instant::now()))
    }

    pub fn expires_at(&self) -> Option<Instant> {
        self.hold.as_ref()?.expires_at
    }

    /// Replaces any hold already in place, so switching duration or scope needs no
    /// explicit release first.
    pub async fn engage(&mut self, hold: Hold, scope: Scope) -> Result<()> {
        self.release().await;

        let why = match hold {
            Hold::Indefinite => "Keep awake until switched off".to_owned(),
            Hold::For(duration) => format!("Keep awake for {}", format_duration(duration)),
        };

        let logind = Login1ManagerProxy::new(&self.system).await?;
        let descriptor =
            logind.inhibit(Scope::LOGIND_WHAT, WHO, &why, "block").await?;

        let screen_saver_cookie = match scope {
            Scope::SystemOnly => None,
            Scope::SystemAndDisplay => {
                let screen_saver = ScreenSaverProxy::new(&self.session).await?;
                Some(screen_saver.inhibit(WHO, &why).await?)
            }
        };

        self.hold = Some(ActiveHold {
            _logind: descriptor,
            screen_saver_cookie,
            expires_at: match hold {
                Hold::Indefinite => None,
                Hold::For(duration) => Instant::now().checked_add(duration),
            },
            scope,
        });

        tracing::info!(?hold, ?scope, "keep awake engaged");
        Ok(())
    }

    pub async fn release(&mut self) {
        let Some(hold) = self.hold.take() else {
            return;
        };

        if let Some(cookie) = hold.screen_saver_cookie {
            match ScreenSaverProxy::new(&self.session).await {
                Ok(screen_saver) => {
                    if let Err(err) = screen_saver.un_inhibit(cookie).await {
                        tracing::warn!(%err, "screen saver inhibition may still be held");
                    }
                }
                Err(err) => tracing::warn!(%err, "could not reach the screen saver to release"),
            }
        }

        tracing::info!("keep awake released");
    }

    pub async fn expire_if_due(&mut self) -> bool {
        let due = self
            .hold
            .as_ref()
            .and_then(|hold| hold.expires_at)
            .is_some_and(|expires_at| Instant::now() >= expires_at);

        if due {
            self.release().await;
        }
        due
    }
}

pub fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);

    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_as_a_countdown() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h 00m");
        assert_eq!(format_duration(Duration::from_secs(7845)), "2h 10m");
    }

    #[test]
    fn blocking_idle_alone_leaves_manual_suspend_working() {
        assert_eq!(Scope::LOGIND_WHAT, "idle");
    }
}
