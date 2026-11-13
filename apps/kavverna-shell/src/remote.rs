use crate::command::{self, Command};
use crate::{awake_state, panel, process_state};
use feature_catalog::Feature;
use keep_awake::Hold;
use keep_awake::{format_compact, format_duration};
use std::sync::OnceLock;
use std::time::Duration;
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::interface;

pub const SERVICE: &str = "dev.kavverna.Shell";
const INTERFACE: &str = "dev.kavverna.Shell";
const PATH: &str = "/dev/kavverna/Shell";

/// The runtime the bus was claimed on. zbus answers calls on an executor of its own, and a
/// reading that waits a second for a rate has to wait somewhere other than there, or every
/// other call on the interface waits with it.
static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();

/// Runs a blocking read on the bus runtime's blocking pool and waits for it without holding
/// up the interface.
async fn off_the_bus<T: Send + 'static>(
    read: impl FnOnce() -> T + Send + 'static,
) -> zbus::fdo::Result<T> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| zbus::fdo::Error::Failed("the bus runtime is not running".into()))?;
    runtime.spawn_blocking(read).await.map_err(|err| zbus::fdo::Error::Failed(err.to_string()))
}

fn system_monitor_installed() -> zbus::fdo::Result<()> {
    if crate::settings::is_installed(Feature::SystemMonitor) {
        Ok(())
    } else {
        Err(zbus::fdo::Error::NotSupported("the system monitor is not installed".into()))
    }
}

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
        command::send(Command::Engage(hold, crate::settings::scope()));
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

    /// The AI agents reading as JSON, read afresh for the call, with each window's freshness
    /// and age judged at the moment of answering. Reads nothing when the utility is removed.
    async fn agent_usage(&self) -> String {
        crate::agent_usage_state::reading_as_json().await
    }

    /// The applications spending a resource most, as JSON, so what a fold on the monitoring
    /// page shows can be checked. `resource` is cpu, memory, disk, gpu (the discrete card) or
    /// gpu:<card name>; `limit` 0 means the eight a fold shows. Measured now on a sampler of its
    /// own, over one second where the figure is a rate, and ordered by the figure alone.
    async fn top_applications(&self, resource: String, limit: u32) -> zbus::fdo::Result<String> {
        system_monitor_installed()?;
        let limit = match limit {
            0 => process_state::SHOWN,
            limit => usize::try_from(limit).unwrap_or(usize::MAX),
        };
        off_the_bus(move || process_state::top_applications_json(&resource, limit))
            .await?
            .map_err(zbus::fdo::Error::InvalidArgs)
    }

    /// The ports this user's programs listen on, as JSON, the list the panel shows.
    async fn listening_ports(&self) -> zbus::fdo::Result<String> {
        system_monitor_installed()?;
        off_the_bus(process_state::listening_ports_json).await
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
    let _ = RUNTIME.set(tokio::runtime::Handle::current());
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
pub const PAGES: [&str; 5] = ["energy", "sound", "monitoring", "clipboard", "tools"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wanted {
    Panel,
    Page(String),
    Settings,
    /// Answered here and now rather than by a running instance. These used to open the panel in
    /// silence, which on Linux is the opposite of what anyone expects.
    Version,
    Usage,
    Selftest,
    /// Claude Code's status line command: answered from stdin with no interface and no bus, since
    /// it runs once for every status line every session draws.
    ClaudeStatusLine,
    /// Said back with a non-zero exit rather than swallowed. A typo that opens the panel looks
    /// exactly like the flag having worked.
    Unknown(String),
}

impl Wanted {
    pub fn from_arguments(arguments: impl Iterator<Item = String>) -> Self {
        let mut arguments = arguments.skip(1);
        match arguments.next().as_deref() {
            Some("--settings") => Self::Settings,
            Some("--page") => match arguments.next() {
                Some(name) if PAGES.contains(&name.as_str()) => Self::Page(name),
                Some(name) => Self::Unknown(format!("--page {name}")),
                None => Self::Unknown("--page, with no page named".into()),
            },
            Some("--version" | "-V") => Self::Version,
            Some("--help" | "-h") => Self::Usage,
            Some("--selftest") => Self::Selftest,
            Some("--claude-statusline") => Self::ClaudeStatusLine,
            Some(other) => Self::Unknown(other.to_owned()),
            None => Self::Panel,
        }
    }
}

/// The release scheme is four numbers. The first three are the release and the fourth is the
/// build that produced this binary, stamped by CI and zero for anything built by hand.
pub fn version() -> String {
    format!("{}.{}", env!("CARGO_PKG_VERSION"), env!("KAVVERNA_BUILD"))
}

pub const USAGE: &str = "\
Kavverna, one tray icon for the utilities a KDE Plasma desktop is missing.

Usage: kavverna-shell [OPTION]

  --page <name>   Open the panel on a page: sound, monitoring, clipboard, tools, energy
  --settings      Open the settings page
  --selftest      Report what this machine offers of what Kavverna relies on, and exit
  --claude-statusline
                  Claude Code's status line command: keeps the allowance from the status
                  line input on stdin for the AI agents page, prints a short status line
  -V, --version   Print the version and exit
  -h, --help      Print this and exit

With no option it opens the panel, or raises the instance already running.

The session bus name dev.kavverna.Shell opens the panel and its pages, holds and releases
keep awake, cycles the output, mutes every microphone, and answers AgentUsage with the AI
agents reading as JSON. The clipboard history and the shelf are not on the bus.
";

async fn raise_running_instance(wanted: &Wanted) {
    let Ok(connection) = zbus::Connection::session().await else {
        return;
    };

    let call = match wanted {
        // Answered before the bus is reached, so a running instance never sees them.
        Wanted::Version
        | Wanted::Usage
        | Wanted::Selftest
        | Wanted::ClaudeStatusLine
        | Wanted::Unknown(_) => return,
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
    }

    /// A typo that opened the panel looked exactly like the flag having worked, which is how
    /// --selftest could be mistyped for months without anyone noticing it did nothing.
    #[test]
    fn what_is_not_understood_is_said_back() {
        assert_eq!(wanted(&["--nonsense"]), Wanted::Unknown("--nonsense".into()));
        assert_eq!(wanted(&["--page", "banana"]), Wanted::Unknown("--page banana".into()));
        assert_eq!(wanted(&["--page"]), Wanted::Unknown("--page, with no page named".into()));
    }

    #[test]
    fn every_page_the_panel_has_is_reachable() {
        for page in PAGES {
            assert_eq!(wanted(&["--page", page]), Wanted::Page(page.into()));
        }
    }

    #[test]
    fn the_selftest_is_understood() {
        assert_eq!(wanted(&["--selftest"]), Wanted::Selftest);
    }

    #[test]
    fn the_claude_status_line_is_understood() {
        assert_eq!(wanted(&["--claude-statusline"]), Wanted::ClaudeStatusLine);
    }

    #[test]
    fn a_question_is_recognised_by_either_spelling() {
        assert_eq!(wanted(&["--version"]), Wanted::Version);
        assert_eq!(wanted(&["-V"]), Wanted::Version);
        assert_eq!(wanted(&["--help"]), Wanted::Usage);
        assert_eq!(wanted(&["-h"]), Wanted::Usage);
    }

    /// The fourth number is the build, and a package reporting zero would mean CI never handed
    /// it one.
    #[test]
    fn the_version_carries_four_numbers() {
        let printed = version();
        let parts: Vec<&str> = printed.split('.').collect();

        assert_eq!(parts.len(), 4, "version was {printed}");
        assert!(parts.iter().all(|part| part.parse::<u32>().is_ok()), "version was {printed}");
    }

    #[test]
    fn the_desktop_entry_actions_are_understood() {
        assert_eq!(wanted(&["--settings"]), Wanted::Settings);
        assert_eq!(wanted(&["--page", "clipboard"]), Wanted::Page("clipboard".into()));
    }
}
