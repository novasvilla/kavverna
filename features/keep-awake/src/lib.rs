//! Holding off automatic sleep, on a timer or until told otherwise.

use std::time::{Duration, Instant};
use zbus::zvariant::OwnedFd;

mod inhibitor;
mod mouse_jiggle;

pub use mouse_jiggle::{Activity, Keystroke, MouseJiggle, Screen};

use inhibitor::{
    CHANGE_SCREEN_SETTINGS, INTERRUPT_SESSION, Login1ManagerProxy, PolicyAgentProxy,
    ScreenSaverProxy,
};

const WHO: &str = "Kavverna";

#[derive(Debug, thiserror::Error)]
pub enum KeepAwakeError {
    #[error("could not reach the bus: {0}")]
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
    /// Blocks automatic suspend only. Displays sleep as usual and local work carries on,
    /// which is the common case for a long download or build.
    SystemOnly,
    /// Also holds off screen blanking and locking.
    SystemAndDisplay,
}

impl Scope {
    /// `idle` alone, never `idle:sleep`: blocking `sleep` would also swallow a deliberate
    /// suspend from the power menu, which is not what "keep awake" should mean.
    const LOGIND_WHAT: &'static str = "idle";

    fn policy_bits(self) -> u32 {
        match self {
            Self::SystemOnly => INTERRUPT_SESSION,
            Self::SystemAndDisplay => INTERRUPT_SESSION | CHANGE_SCREEN_SETTINGS,
        }
    }
}

/// Whether the user asked for this hold or a rule did, which decides whether a rule is
/// allowed to end it again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Automation,
}

struct ActiveHold {
    /// Only held where no desktop power daemon answered, since PowerDevil mirrors logind's
    /// idle inhibitors and holding both would register the same hold twice.
    _logind: Option<OwnedFd>,
    policy_cookie: Option<u32>,
    screen_saver_cookie: Option<u32>,
    expires_at: Option<Instant>,
    scope: Scope,
    trigger: Trigger,
}

pub struct KeepAwake {
    /// logind is a system service; PowerDevil and the screen saver belong to the session.
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

    pub fn trigger(&self) -> Option<Trigger> {
        self.hold.as_ref().map(|hold| hold.trigger)
    }

    pub fn remaining(&self) -> Option<Duration> {
        let expires_at = self.hold.as_ref()?.expires_at?;
        Some(expires_at.saturating_duration_since(Instant::now()))
    }

    /// Whether the desktop power daemon accepted the hold. False means we are relying on
    /// logind alone, which some sessions ignore.
    pub fn power_daemon_holds(&self) -> bool {
        self.hold.as_ref().is_some_and(|hold| hold.policy_cookie.is_some())
    }

    pub fn is_timed(&self) -> bool {
        self.hold.as_ref().is_some_and(|hold| hold.expires_at.is_some())
    }

    /// Replaces any hold already in place, so switching duration or scope needs no explicit
    /// release first.
    pub async fn engage(&mut self, hold: Hold, scope: Scope, trigger: Trigger) -> Result<()> {
        self.release().await;

        let why = match hold {
            Hold::Indefinite => "Keep awake until switched off".to_owned(),
            Hold::For(duration) => format!("Keep awake for {}", format_duration(duration)),
        };

        // Preferred over logind's idle inhibitor because it takes effect at once rather than
        // after the several seconds PowerDevil takes to notice one, and because it can ask
        // for the screen to stay on, which logind has no way to express.
        let policy_cookie = match PolicyAgentProxy::new(&self.session).await {
            Ok(agent) => match agent.add_inhibition(scope.policy_bits(), WHO, &why).await {
                Ok(cookie) => Some(cookie),
                Err(err) => {
                    tracing::error!(%err, "the power daemon refused the inhibition");
                    None
                }
            },
            Err(err) => {
                tracing::warn!(%err, "no desktop power daemon on this session");
                None
            }
        };

        let logind_hold = match policy_cookie {
            Some(_) => None,
            None => {
                let logind = Login1ManagerProxy::new(&self.system).await?;
                Some(logind.inhibit(Scope::LOGIND_WHAT, WHO, &why, "block").await?)
            }
        };

        let screen_saver_cookie = match scope {
            Scope::SystemOnly => None,
            Scope::SystemAndDisplay => {
                let screen_saver = ScreenSaverProxy::new(&self.session).await?;
                Some(screen_saver.inhibit(WHO, &why).await?)
            }
        };

        self.hold = Some(ActiveHold {
            _logind: logind_hold,
            policy_cookie,
            screen_saver_cookie,
            expires_at: match hold {
                Hold::Indefinite => None,
                Hold::For(duration) => Instant::now().checked_add(duration),
            },
            scope,
            trigger,
        });

        tracing::info!(?hold, ?scope, ?trigger, policy_cookie, "keep awake engaged");
        Ok(())
    }

    pub async fn release(&mut self) {
        let Some(hold) = self.hold.take() else {
            return;
        };

        if let Some(cookie) = hold.policy_cookie {
            match PolicyAgentProxy::new(&self.session).await {
                Ok(agent) => {
                    if let Err(err) = agent.release_inhibition(cookie).await {
                        tracing::warn!(%err, cookie, "power inhibition may still be held");
                    }
                }
                Err(err) => tracing::warn!(%err, "could not reach the power daemon to release"),
            }
        }

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

    /// Anchors to now when the deadline has already passed, so extending a lapsed hold does
    /// not hand back a moment in the past. Indefinite holds have nothing to extend.
    pub fn extend(&mut self, extra: Duration) -> bool {
        let Some(hold) = self.hold.as_mut() else {
            return false;
        };
        let Some(expires_at) = hold.expires_at else {
            return false;
        };

        let anchor = expires_at.max(Instant::now());
        hold.expires_at = anchor.checked_add(extra);
        tracing::info!(extra_secs = extra.as_secs(), "keep awake extended");
        true
    }

    /// Drops a timed hold once it has run out. Returns whether this call ended one.
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

    /// Asks the power daemon whether it is honouring a suspend inhibition right now, which
    /// is the only answer that says the machine will actually stay up.
    pub async fn power_daemon_honours_us(&self) -> bool {
        match PolicyAgentProxy::new(&self.session).await {
            Ok(agent) => agent.has_inhibition(INTERRUPT_SESSION).await.unwrap_or(false),
            Err(_) => false,
        }
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

/// The compact form for the tray, where there is room for a few characters at most.
pub fn format_compact(remaining: Option<Duration>) -> String {
    let Some(remaining) = remaining else {
        return "∞".into();
    };

    let total = remaining.as_secs();
    if total >= 3600 {
        format!("{}:{:02}", total / 3600, (total % 3600) / 60)
    } else {
        format!("{}m", (total / 60).max(1))
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
    fn the_compact_form_fits_a_tray_label() {
        assert_eq!(format_compact(None), "∞");
        assert_eq!(format_compact(Some(Duration::from_secs(7200))), "2:00");
        assert_eq!(format_compact(Some(Duration::from_secs(5400))), "1:30");
        assert_eq!(format_compact(Some(Duration::from_secs(600))), "10m");
        assert_eq!(format_compact(Some(Duration::from_secs(20))), "1m");
    }

    #[test]
    fn blocking_idle_alone_leaves_manual_suspend_working() {
        assert_eq!(Scope::LOGIND_WHAT, "idle");
    }

    #[test]
    fn letting_displays_sleep_asks_for_less() {
        assert_eq!(Scope::SystemOnly.policy_bits(), INTERRUPT_SESSION);
        assert_eq!(
            Scope::SystemAndDisplay.policy_bits(),
            INTERRUPT_SESSION | CHANGE_SCREEN_SETTINGS
        );
    }
}
