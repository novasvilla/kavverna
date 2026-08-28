use crate::command::{self, Command};
use crate::{awake_state, panel};
use keep_awake::Hold;
use keep_awake::{format_compact, format_duration};
use std::time::Duration;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::interface;

pub const SERVICE: &str = "dev.kavverna.Shell";
const INTERFACE: &str = "dev.kavverna.Shell";
const PATH: &str = "/dev/kavverna/Shell";

struct Shell;

#[interface(name = "dev.kavverna.Shell")]
impl Shell {
    /// What a desktop file action or a second launch calls to bring the panel up.
    fn activate(&self) {
        panel::open_hub();
    }

    /// One of energy, sound, monitoring, clipboard or tools.
    fn show_page(&self, name: String) {
        panel::open_page(&name);
    }

    fn show_settings(&self) {
        panel::open_settings();
    }

    fn toggle_panel(&self) {
        panel::toggle();
    }

    /// Zero minutes means hold until switched off, matching the duration picker.
    fn keep_awake(&self, minutes: u32) {
        let hold = if minutes == 0 {
            Hold::Indefinite
        } else {
            Hold::For(Duration::from_secs(u64::from(minutes) * 60))
        };
        command::send(Command::Engage(hold, crate::tray::configured_scope()));
    }

    fn add_minutes(&self, minutes: u32) {
        command::send(Command::Extend(Duration::from_secs(u64::from(minutes) * 60)));
    }

    fn allow_sleep(&self) {
        command::send(Command::Release);
    }

    fn cycle_output(&self) -> String {
        crate::mixer_state::cycle_output().unwrap_or_else(|| "no outputs".into())
    }

    fn mute_every_input(&self, muted: bool) {
        crate::mixer_state::mute_every_input(muted);
    }

    #[zbus(property)]
    fn default_output(&self) -> String {
        crate::mixer_state::get()
            .default_output()
            .map_or_else(|| "none".into(), |device| device.description.clone())
    }

    #[zbus(property)]
    fn microphones_muted(&self) -> bool {
        crate::mixer_state::get().every_input_muted()
    }

    #[zbus(property)]
    fn awake(&self) -> bool {
        awake_state::get().active
    }

    #[zbus(property)]
    fn remaining(&self) -> String {
        let state = awake_state::get();
        if !state.active {
            return "off".into();
        }
        state.remaining.map_or_else(|| "indefinite".into(), format_duration)
    }

    #[zbus(property)]
    fn compact_remaining(&self) -> String {
        let state = awake_state::get();
        if !state.active { "off".into() } else { format_compact(state.remaining) }
    }
}

pub enum Claim {
    /// This process owns the name and is the running instance.
    Ours,
    /// Another instance already answers; it has been asked to show itself.
    AlreadyRunning,
}

/// Owning a well known name is also how a second launch is detected, so the two concerns
/// are answered by the same call.
///
/// The name is requested with `DoNotQueue`: the default would leave a second launch waiting
/// in line for the name and running a full second copy of the app in the meantime.
pub async fn claim(wanted: &Wanted) -> zbus::Result<Claim> {
    let connection = zbus::connection::Builder::session()?.serve_at(PATH, Shell)?.build().await?;

    // The bus reports a taken name as an error rather than a reply variant, so both have to
    // be handled or a second launch quietly runs a full second copy of the app.
    let reply = match connection
        .request_name_with_flags(SERVICE, RequestNameFlags::DoNotQueue.into())
        .await
    {
        Ok(reply) => reply,
        Err(zbus::Error::NameTaken) => {
            drop(connection);
            raise_running_instance(wanted).await;
            return Ok(Claim::AlreadyRunning);
        }
        Err(err) => return Err(err),
    };

    match reply {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {
            // Dropping the connection would drop the name with it.
            std::mem::forget(connection);
            Ok(Claim::Ours)
        }
        RequestNameReply::Exists | RequestNameReply::InQueue => {
            drop(connection);
            raise_running_instance(wanted).await;
            Ok(Claim::AlreadyRunning)
        }
    }
}

/// What a launch asks the running instance to show. The desktop entry's actions are these, so
/// a right click on the icon in a launcher reaches the same places the panel does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wanted {
    Panel,
    Page(String),
    Settings,
}

impl Wanted {
    pub fn from_arguments(arguments: impl Iterator<Item = String>) -> Self {
        let mut arguments = arguments.skip(1);
        match arguments.next().as_deref() {
            Some("--settings") => Self::Settings,
            Some("--page") => arguments.next().map_or(Self::Panel, Self::Page),
            _ => Self::Panel,
        }
    }
}

async fn raise_running_instance(wanted: &Wanted) {
    let Ok(connection) = zbus::Connection::session().await else {
        return;
    };

    let call = match wanted {
        Wanted::Panel => {
            connection.call_method(Some(SERVICE), PATH, Some(INTERFACE), "Activate", &()).await
        }
        Wanted::Settings => {
            connection.call_method(Some(SERVICE), PATH, Some(INTERFACE), "ShowSettings", &()).await
        }
        Wanted::Page(name) => {
            connection.call_method(Some(SERVICE), PATH, Some(INTERFACE), "ShowPage", &(name,)).await
        }
    };

    if let Err(err) = call {
        tracing::warn!(%err, "another instance holds the name but did not answer");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wanted(arguments: &[&str]) -> Wanted {
        let mut all = vec!["kavverna-shell"];
        all.extend_from_slice(arguments);
        Wanted::from_arguments(all.into_iter().map(str::to_owned))
    }

    #[test]
    fn a_plain_launch_asks_for_the_panel() {
        assert_eq!(wanted(&[]), Wanted::Panel);
        assert_eq!(wanted(&["--nonsense"]), Wanted::Panel);
    }

    #[test]
    fn the_desktop_entry_actions_are_understood() {
        assert_eq!(wanted(&["--settings"]), Wanted::Settings);
        assert_eq!(wanted(&["--page", "clipboard"]), Wanted::Page("clipboard".into()));
    }

    #[test]
    fn a_page_with_no_name_is_just_the_panel() {
        assert_eq!(wanted(&["--page"]), Wanted::Panel);
    }
}
