//! Depends on no feature crate, so naming a feature cannot pull its service into the binary.

use strum_macros::EnumIter;

mod descriptor;

pub use descriptor::{Descriptor, EnergyProfile, Group, Readiness};

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
    ClipboardTransform,
    KeepAwake,
    MouseJiggle,
    FanControl,
    Themes,
    Shelf,
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
            Self::ClipboardTransform => "clipboard-transform",
            Self::KeepAwake => "keep-awake",
            Self::MouseJiggle => "mouse-jiggle",
            Self::FanControl => "fan-control",
            Self::Themes => "themes",
            Self::Shelf => "shelf",
        }
    }

    /// Sits above the feature's own enable keys, so removing a feature preserves whatever it
    /// was configured to do and putting it back restores exactly that.
    pub fn availability_key(self) -> String {
        format!("{}.installed", self.id())
    }

    /// Something not built yet is listed so the catalogue stays honest about where Kavverna is
    /// going, but it is never installed and never asked to run.
    pub const fn is_built(self) -> bool {
        matches!(self.describe().readiness, Readiness::Built)
    }

    /// Everything that exists arrives installed. Thirteen switches to find before the first use
    /// is not a welcome, and each feature's own setting still decides what it actually does.
    pub const fn installed_by_default(self) -> bool {
        self.is_built()
    }
}
