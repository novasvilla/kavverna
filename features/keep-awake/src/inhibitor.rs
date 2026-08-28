use zbus::proxy;
use zbus::zvariant::OwnedFd;

/// Blocks the idle action logind itself would take. On a KDE session that action is usually
/// `ignore`, so this alone stops nothing; it is held for sessions where logind, rather than
/// a desktop power daemon, owns the idle timeout.
#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub trait Login1Manager {
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;
}

/// PowerDevil runs its own idle timer and suspends without consulting logind's idle
/// inhibitors, so this is the one that actually stops a KDE session suspending.
#[proxy(
    interface = "org.kde.Solid.PowerManagement.PolicyAgent",
    default_service = "org.kde.Solid.PowerManagement",
    default_path = "/org/kde/Solid/PowerManagement/PolicyAgent"
)]
pub trait PolicyAgent {
    fn add_inhibition(&self, types: u32, app_name: &str, reason: &str) -> zbus::Result<u32>;

    fn release_inhibition(&self, cookie: u32) -> zbus::Result<()>;
}

/// PowerDevil's `RequiredPolicies` bits.
pub const INTERRUPT_SESSION: u32 = 1;
pub const CHANGE_SCREEN_SETTINGS: u32 = 4;

#[proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver"
)]
pub trait ScreenSaver {
    fn inhibit(&self, application_name: &str, reason: &str) -> zbus::Result<u32>;

    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
}
