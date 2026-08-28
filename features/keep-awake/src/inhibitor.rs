use zbus::proxy;
use zbus::zvariant::OwnedFd;

/// The inhibitor stays in force only while the returned descriptor is held open, so the
/// caller owning that descriptor is what keeps the machine awake.
#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub trait Login1Manager {
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;
}

#[proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver"
)]
pub trait ScreenSaver {
    fn inhibit(&self, application_name: &str, reason: &str) -> zbus::Result<u32>;

    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
}
