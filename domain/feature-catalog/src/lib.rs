//! Depends on no feature crate, so naming a feature cannot pull its service into the binary.

use strum_macros::EnumIter;

mod descriptor;

pub use descriptor::{Descriptor, EnergyProfile, Group};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, EnumIter)]
pub enum Feature {
    VolumeMixer,
    OutputSwitcher,
    MicrophoneTools,
    SystemMonitor,
    NetworkMonitor,
    MonitorAlerts,
    ClipboardHistory,
    ClipboardAutoClear,
    CleanUrl,
    PlainTextPaste,
    KeepAwake,
    FanControl,
}

impl Feature {
    /// Renaming a returned id orphans that feature's stored settings.
    pub const fn id(self) -> &'static str {
        match self {
            Self::VolumeMixer => "volume-mixer",
            Self::OutputSwitcher => "output-switcher",
            Self::MicrophoneTools => "microphone-tools",
            Self::SystemMonitor => "system-monitor",
            Self::NetworkMonitor => "network-monitor",
            Self::MonitorAlerts => "monitor-alerts",
            Self::ClipboardHistory => "clipboard-history",
            Self::ClipboardAutoClear => "clipboard-auto-clear",
            Self::CleanUrl => "clean-url",
            Self::PlainTextPaste => "plain-text-paste",
            Self::KeepAwake => "keep-awake",
            Self::FanControl => "fan-control",
        }
    }

    /// Sits above the feature's own enable keys, so uninstalling preserves its configuration.
    pub fn availability_key(self) -> String {
        format!("{}.installed", self.id())
    }
}
