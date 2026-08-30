//! Per-application volume, routing and input selection over PipeWire.

mod app_identity;
mod desktop_entry;
mod model;
mod session;
mod volume;

pub use app_identity::{
    AppKey, Properties, app_key, app_key_resolving, binary_of_process, cmdline_of_process,
    display_name, is_generic, presentable, refine_from_cmdline, steam_icon_of_process,
};
pub use desktop_entry::Entry as DesktopEntry;
pub use model::{
    Anchor, AudioApplication, AudioDevice, AudioStream, ChosenDevice, DeviceRole, MixerSnapshot,
    RouteState,
};
pub use session::{MixerCommand, MixerCommands, StreamTarget, start};
pub use volume::{MAX_PERCENT, UNITY_PERCENT, Volume};
