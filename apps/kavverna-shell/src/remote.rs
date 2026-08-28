use crate::command::{self, Command};
use crate::{awake_state, panel};
use keep_awake::Hold;
use std::time::Duration;
use keep_awake::{format_compact, format_duration};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::interface;

pub const SERVICE: &str = "dev.kavverna.Shell";
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

    /// Moves to the next output the user cycles through, which is what a shortcut binds to.
    fn cycle_output(&self) -> String {
        let snapshot = crate::mixer_state::get();
        let cycle: Vec<String> = snapshot.outputs.iter().map(|d| d.name.clone()).collect();

        match snapshot.next_in_cycle(&cycle) {
            Some(name) => {
                let name = name.clone();
                crate::mixer_state::send(
                    sound_mixer::MixerCommand::MakeDefaultOutput(name.clone()),
                );
                name
            }
            None => "no outputs".into(),
        }
    }

    fn mute_every_input(&self, muted: bool) {
        for device in crate::mixer_state::get().inputs {
            crate::mixer_state::send(sound_mixer::MixerCommand::SetMute {
                node_id: device.node_id,
                muted,
            });
        }
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
pub async fn claim() -> zbus::Result<Claim> {
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
            raise_running_instance().await;
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
            raise_running_instance().await;
            Ok(Claim::AlreadyRunning)
        }
    }
}

async fn raise_running_instance() {
    let Ok(connection) = zbus::Connection::session().await else {
        return;
    };

    let call = connection
        .call_method(Some(SERVICE), PATH, Some("dev.kavverna.Shell"), "Activate", &())
        .await;

    if let Err(err) = call {
        tracing::warn!(%err, "another instance holds the name but did not answer");
    }
}
