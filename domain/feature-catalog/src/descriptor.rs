use crate::Feature;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Sound,
    Monitoring,
    Clipboard,
    Energy,
    Tools,
}

/// A curated claim about what a feature costs at rest, not a live measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyProfile {
    Idle,
    Periodic,
    WatchesClipboard,
    WatchesInput,
}

#[derive(Debug, Clone, Copy)]
pub struct Descriptor {
    pub title: &'static str,
    pub summary: &'static str,
    pub group: Group,
    pub icon: &'static str,
    pub energy: EnergyProfile,
    pub beta: bool,
    pub enable_keys: &'static [&'static str],
}

impl Feature {
    /// Exhaustive by design: adding a variant must fail the build here.
    pub const fn describe(self) -> Descriptor {
        match self {
            Self::VolumeMixer => Descriptor {
                title: "Volume mixer",
                summary: "Per-app volume with exact percentages, boost and per-app output.",
                group: Group::Sound,
                icon: "audio-volume-high",
                energy: EnergyProfile::Idle,
                beta: false,
                enable_keys: &["volume-mixer.enabled"],
            },
            Self::OutputSwitcher => Descriptor {
                title: "Output switcher",
                summary: "Cycle chosen outputs from a shortcut and duck on headphone loss.",
                group: Group::Sound,
                icon: "audio-headphones",
                energy: EnergyProfile::Idle,
                beta: false,
                enable_keys: &["output-switcher.enabled"],
            },
            Self::MicrophoneTools => Descriptor {
                title: "Microphone tools",
                summary: "Pin a preferred input and mute every microphone at once.",
                group: Group::Sound,
                icon: "audio-input-microphone",
                energy: EnergyProfile::Idle,
                beta: false,
                enable_keys: &["microphone-tools.enabled"],
            },
            Self::SystemMonitor => Descriptor {
                title: "System monitor",
                summary: "CPU, GPU, memory and temperatures with history graphs.",
                group: Group::Monitoring,
                icon: "utilities-system-monitor",
                energy: EnergyProfile::Periodic,
                beta: false,
                enable_keys: &["system-monitor.enabled"],
            },
            Self::NetworkMonitor => Descriptor {
                title: "Network",
                summary: "Live rates and session totals per interface.",
                group: Group::Monitoring,
                icon: "network-wired",
                energy: EnergyProfile::Periodic,
                beta: false,
                enable_keys: &["network-monitor.enabled"],
            },
            Self::MonitorAlerts => Descriptor {
                title: "Alerts",
                summary: "Notify on sustained load, high temperature, memory pressure or low disk.",
                group: Group::Monitoring,
                icon: "dialog-warning",
                energy: EnergyProfile::Periodic,
                beta: false,
                enable_keys: &["monitor-alerts.enabled"],
            },
            Self::ClipboardHistory => Descriptor {
                title: "Clipboard history",
                summary: "Text, images and files with pinning, search and preview.",
                group: Group::Clipboard,
                icon: "edit-paste",
                energy: EnergyProfile::WatchesClipboard,
                beta: false,
                enable_keys: &["clipboard-history.enabled"],
            },
            Self::ClipboardAutoClear => Descriptor {
                title: "Auto clear clipboard",
                summary: "Empty the clipboard on a timer, on lock or on sleep.",
                group: Group::Clipboard,
                icon: "edit-clear-all",
                energy: EnergyProfile::WatchesClipboard,
                beta: false,
                enable_keys: &["clipboard-auto-clear.enabled"],
            },
            Self::CleanUrl => Descriptor {
                title: "Clean URL",
                summary: "Strip tracking parameters from copied links.",
                group: Group::Clipboard,
                icon: "link",
                energy: EnergyProfile::WatchesClipboard,
                beta: false,
                enable_keys: &["clean-url.enabled"],
            },
            Self::PlainTextPaste => Descriptor {
                title: "Paste as plain text",
                summary: "Paste without fonts, colours or links.",
                group: Group::Clipboard,
                icon: "edit-paste-style",
                energy: EnergyProfile::WatchesInput,
                beta: false,
                enable_keys: &["plain-text-paste.enabled"],
            },
            Self::KeepAwake => Descriptor {
                title: "Keep awake",
                summary: "Hold off sleep on a timer or indefinitely, letting displays sleep.",
                group: Group::Energy,
                icon: "preferences-system-power-management",
                energy: EnergyProfile::Idle,
                beta: false,
                enable_keys: &["keep-awake.enabled"],
            },
            Self::MouseJiggle => Descriptor {
                title: "Mouse jiggle",
                summary: "Nudge the pointer on an interval so idle watchers see activity.",
                group: Group::Tools,
                icon: "input-mouse",
                energy: EnergyProfile::Periodic,
                beta: false,
                enable_keys: &["mouse-jiggle.enabled"],
            },
            Self::FanControl => Descriptor {
                title: "Fan control",
                summary: "Manual speeds and temperature curves with live RPM.",
                group: Group::Energy,
                icon: "sensors-fan",
                energy: EnergyProfile::Periodic,
                beta: true,
                enable_keys: &["fan-control.enabled"],
            },
        }
    }
}
