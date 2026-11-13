//! Which application a process belongs to.
//!
//! Plasma starts every application it launches in its own systemd user unit, named after the
//! desktop entry: `app-<id>@<random>.service` or `app-<id>-<random>.scope`. Everything the
//! application forks stays in that cgroup, which folds Chromium renderers, Electron helpers and
//! a game's child processes into the program a person launched, without guessing from names.
//! The parent chain covers what runs outside such a unit, and the process's own identity names
//! whatever is left.

use app_identity::DesktopEntry;
use std::collections::HashMap;

/// What decided a group, which is also what keeps two groups apart.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ApplicationKey {
    /// A desktop entry, through the unit Plasma started it in. Two windows of one program
    /// launched separately are two units and one application.
    Entry(String),
    /// A user unit that names no desktop entry: a session service, or a terminal surface.
    Unit(String),
    /// Outside any user unit, the name the process goes by.
    Named(String),
}

/// The desktop entry id inside an application unit name, with the systemd escaping undone.
/// The launcher part the specification allows, `app-flatpak-<id>-<n>.scope`, is offered as a
/// second reading, since only the desktop index can tell which one is the id.
pub(crate) fn desktop_ids(unit: &str) -> Vec<String> {
    let Some(rest) = unit.strip_prefix("app-") else {
        return Vec::new();
    };
    let id = if let Some(service) = rest.strip_suffix(".service") {
        service.split_once('@').map_or(service, |(id, _)| id)
    } else if let Some(scope) = rest.strip_suffix(".scope") {
        match scope.rsplit_once('-') {
            Some((id, _)) => id,
            None => scope,
        }
    } else {
        return Vec::new();
    };
    let id = unescape(id);
    let mut ids = vec![id.clone()];
    if let Some((_, without_launcher)) = id.split_once('-') {
        ids.push(without_launcher.to_owned());
    }
    ids
}

/// The program a unit with no entry of its own was started by, read from the front of its id:
/// a terminal starts each surface as `app-ghostty-surface-transient-<n>.scope`, which names
/// no entry, while `ghostty` names the terminal. Longest first, and never the full id, which
/// `desktop_ids` has already offered.
pub(crate) fn host_ids(unit: &str) -> Vec<String> {
    let Some(id) = desktop_ids(unit).into_iter().next() else {
        return Vec::new();
    };
    let mut hosts = Vec::new();
    let mut rest = id.as_str();
    while let Some((front, _)) = rest.rsplit_once('-') {
        if !front.is_empty() {
            hosts.push(front.to_owned());
        }
        rest = front;
    }
    hosts
}

/// The processes that are the desktop session rather than an application, under the names a
/// person knows them by and an icon the theme carries. Matched on the executable's file name,
/// since the kernel's command stops at fifteen bytes and `kwin_wayland_wrapper` would not
/// match. Each is one application however many units it runs in, so the three PipeWire
/// daemons are one row.
const SESSION_SERVICES: [(&str, &str, &str); 7] = [
    ("kwin_wayland", "KWin", "kwin"),
    ("kwin_wayland_wrapper", "KWin", "kwin"),
    ("plasmashell", "Plasma", "plasmashell"),
    ("Xwayland", "Xwayland", "xorg"),
    ("pipewire", "PipeWire", "audio-card"),
    ("pipewire-pulse", "PipeWire", "audio-card"),
    ("wireplumber", "PipeWire", "audio-card"),
];

/// The name and icon of a session service, by its binary's file name.
pub(crate) fn session_service(binary: &str) -> Option<(&'static str, &'static str)> {
    SESSION_SERVICES
        .iter()
        .find(|(known, _, _)| *known == binary)
        .map(|(_, name, icon)| (*name, *icon))
}

/// systemd writes a dash inside a name as `\x2d`, and any other unsafe byte the same way.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && bytes.get(index + 1) == Some(&b'x')
            && let Some(byte) =
                text.get(index + 2..index + 4).and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            index += 4;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What grouping needs to know about one process.
pub(crate) struct Placement<'a> {
    pub pid: u32,
    pub parent: u32,
    pub unit: Option<&'a str>,
    /// The process's own name, already resolved through the desktop index where it could be.
    pub name: &'a str,
    /// A session service, grouped by `name` whatever unit it runs in.
    pub session: bool,
}

pub(crate) struct Assigned {
    pub key: ApplicationKey,
    /// The entry the unit names, when it names one.
    pub entry: Option<DesktopEntry>,
}

/// The program that started a unit naming no entry of its own, through `resolve_host`, which
/// knows programs by binary as well as by identity.
pub(crate) fn host_of(
    unit: &str,
    resolve_host: &dyn Fn(&str) -> Option<DesktopEntry>,
) -> Option<DesktopEntry> {
    host_ids(unit).iter().find_map(|id| resolve_host(id))
}

/// One key per process, in the order given. `resolve` looks a desktop id up, which is the
/// desktop index in use and a fixed table in tests.
pub(crate) fn assign(
    processes: &[Placement<'_>],
    resolve: &dyn Fn(&str) -> Option<DesktopEntry>,
) -> Vec<Assigned> {
    let mut entries: HashMap<String, Option<(String, DesktopEntry)>> = HashMap::new();
    let mut by_unit = |unit: &str| -> Option<Assigned> {
        let entry = entries
            .entry(unit.to_owned())
            .or_insert_with(|| {
                desktop_ids(unit).into_iter().find_map(|id| resolve(&id).map(|entry| (id, entry)))
            })
            .clone();
        Some(match entry {
            Some((id, entry)) => {
                Assigned { key: ApplicationKey::Entry(id.to_ascii_lowercase()), entry: Some(entry) }
            }
            None => Assigned { key: ApplicationKey::Unit(unit.to_owned()), entry: None },
        })
    };

    // A session service takes its whole unit with it: Plasma's unit also holds the shells and
    // KIO workers it started, which are Plasma, not applications of their own to be ended.
    // A session service of its own keeps its own name, so Xwayland in KWin's unit is still
    // Xwayland; the rest of a unit go by its lowest session process.
    let mut session_units: HashMap<&str, (u32, &str)> = HashMap::new();
    for process in processes.iter().filter(|process| process.session) {
        let Some(unit) = process.unit else {
            continue;
        };
        let held = session_units.entry(unit).or_insert((process.pid, process.name));
        if process.pid < held.0 {
            *held = (process.pid, process.name);
        }
    }
    let mut assigned: Vec<Option<Assigned>> = processes
        .iter()
        .map(|process| {
            let session = if process.session {
                Some(process.name)
            } else {
                process.unit.and_then(|unit| session_units.get(unit)).map(|(_, name)| *name)
            };
            if let Some(name) = session {
                return Some(Assigned { key: ApplicationKey::Named(name.to_owned()), entry: None });
            }
            process.unit.and_then(&mut by_unit)
        })
        .collect();

    // Outside a user unit, a process belongs to the nearest ancestor that is in one. The walk
    // is bounded, since a parent table read across two moments can hold a cycle.
    let index: HashMap<u32, usize> =
        processes.iter().enumerate().map(|(at, process)| (process.pid, at)).collect();
    for at in 0..processes.len() {
        if assigned[at].is_some() {
            continue;
        }
        let mut parent = processes[at].parent;
        let mut inherited = None;
        for _ in 0..64 {
            let Some(&above) = index.get(&parent) else {
                break;
            };
            if let Some(found) = &assigned[above] {
                if matches!(found.key, ApplicationKey::Entry(_)) {
                    inherited =
                        Some(Assigned { key: found.key.clone(), entry: found.entry.clone() });
                }
                break;
            }
            parent = processes[above].parent;
        }
        assigned[at] = Some(inherited.unwrap_or_else(|| Assigned {
            key: ApplicationKey::Named(processes[at].name.to_owned()),
            entry: None,
        }));
    }
    assigned.into_iter().map(|found| found.expect("every process was assigned")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> DesktopEntry {
        DesktopEntry { name: name.into(), icon: Some(name.to_ascii_lowercase()) }
    }

    fn known(id: &str) -> Option<DesktopEntry> {
        match id {
            "org.kde.konsole" => Some(entry("Konsole")),
            "com.google.Chrome" => Some(entry("Google Chrome")),
            "org.mozilla.firefox" => Some(entry("Firefox")),
            "arch-update-tray" => Some(entry("Arch Update")),
            _ => None,
        }
    }

    #[test]
    fn unit_names_give_the_desktop_id() {
        assert_eq!(desktop_ids("app-org.kde.konsole@3f2a.service"), vec!["org.kde.konsole"]);
        assert_eq!(desktop_ids("app-com.anthropic.Claude-11416.scope")[0], "com.anthropic.Claude");
        assert_eq!(desktop_ids("app-com.mitchellh.ghostty.service")[0], "com.mitchellh.ghostty");
        assert_eq!(
            desktop_ids(r"app-arch\x2dupdate\x2dtray@autostart.service")[0],
            "arch-update-tray"
        );
        assert_eq!(
            desktop_ids("app-flatpak-org.mozilla.firefox-2201.scope"),
            vec!["flatpak-org.mozilla.firefox".to_owned(), "org.mozilla.firefox".to_owned()]
        );
        assert!(desktop_ids("plasma-plasmashell.service").is_empty());
        assert!(desktop_ids("app.slice").is_empty());
    }

    #[test]
    fn a_browser_and_its_helpers_are_one_application_across_two_launches() {
        let processes = [
            Placement {
                pid: 10,
                parent: 1,
                unit: Some("app-com.google.Chrome@a.service"),
                name: "Google Chrome",
                session: false,
            },
            Placement {
                pid: 11,
                parent: 10,
                unit: Some("app-com.google.Chrome@a.service"),
                name: "Chrome_crashpad",
                session: false,
            },
            Placement {
                pid: 20,
                parent: 1,
                unit: Some("app-com.google.Chrome-77.scope"),
                name: "Google Chrome",
                session: false,
            },
            Placement {
                pid: 30,
                parent: 1,
                unit: Some("app-flatpak-org.mozilla.firefox-5.scope"),
                name: "bwrap",
                session: false,
            },
        ];
        let keys: Vec<ApplicationKey> =
            assign(&processes, &known).into_iter().map(|a| a.key).collect();
        assert_eq!(keys[0], ApplicationKey::Entry("com.google.chrome".into()));
        assert_eq!(keys[1], keys[0]);
        assert_eq!(keys[2], keys[0]);
        assert_eq!(keys[3], ApplicationKey::Entry("org.mozilla.firefox".into()));
    }

    #[test]
    fn a_unit_without_an_entry_is_its_own_group() {
        let processes = [
            Placement {
                pid: 5,
                parent: 1,
                unit: Some("app-ghostty-surface-transient-5656.scope"),
                name: "zsh",
                session: false,
            },
            Placement {
                pid: 6,
                parent: 1,
                unit: Some("app-ghostty-surface-transient-6612.scope"),
                name: "zsh",
                session: false,
            },
            Placement {
                pid: 7,
                parent: 1,
                unit: Some("plasma-plasmashell.service"),
                name: "Plasma",
                session: false,
            },
        ];
        let keys: Vec<ApplicationKey> =
            assign(&processes, &known).into_iter().map(|a| a.key).collect();
        assert_eq!(
            keys[0],
            ApplicationKey::Unit("app-ghostty-surface-transient-5656.scope".into())
        );
        assert_ne!(keys[0], keys[1], "two terminal surfaces are two groups, not one called zsh");
        assert_eq!(keys[2], ApplicationKey::Unit("plasma-plasmashell.service".into()));
    }

    #[test]
    fn outside_a_unit_the_parent_chain_decides_and_then_the_name() {
        let processes = [
            Placement {
                pid: 40,
                parent: 1,
                unit: Some("app-org.kde.konsole@x.service"),
                name: "Konsole",
                session: false,
            },
            Placement { pid: 41, parent: 40, unit: None, name: "bash", session: false },
            Placement { pid: 42, parent: 41, unit: None, name: "cargo", session: false },
            Placement { pid: 50, parent: 1, unit: None, name: "bash", session: false },
            Placement { pid: 51, parent: 1, unit: None, name: "bash", session: false },
        ];
        let keys: Vec<ApplicationKey> =
            assign(&processes, &known).into_iter().map(|a| a.key).collect();
        assert_eq!(keys[1], ApplicationKey::Entry("org.kde.konsole".into()));
        assert_eq!(keys[2], ApplicationKey::Entry("org.kde.konsole".into()));
        assert_eq!(keys[3], ApplicationKey::Named("bash".into()));
        assert_eq!(keys[3], keys[4]);
    }

    #[test]
    fn a_unit_without_an_entry_is_traced_to_the_program_that_started_it() {
        assert_eq!(
            host_ids("app-ghostty-surface-transient-5656.scope"),
            vec!["ghostty-surface".to_owned(), "ghostty".to_owned()]
        );
        assert!(host_ids("app-ghostty-5656.scope").is_empty(), "the full id is not offered twice");
        assert!(host_ids("plasma-plasmashell.service").is_empty());

        let by_binary = |id: &str| (id == "ghostty").then(|| entry("Ghostty"));
        let host = host_of("app-ghostty-surface-transient-5656.scope", &by_binary).unwrap();
        assert_eq!(host.name, "Ghostty");
        assert!(host_of("app-unknown-thing-3.scope", &by_binary).is_none());
    }

    #[test]
    fn the_session_services_go_by_their_own_names_across_units() {
        assert_eq!(session_service("kwin_wayland_wrapper"), Some(("KWin", "kwin")));
        assert_eq!(session_service("wireplumber"), Some(("PipeWire", "audio-card")));
        assert_eq!(session_service("kwin_wayland_wr"), None, "the truncated command is not enough");
        assert_eq!(session_service("firefox"), None);

        let processes = [
            Placement {
                pid: 70,
                parent: 1,
                unit: Some("pipewire.service"),
                name: "PipeWire",
                session: true,
            },
            Placement {
                pid: 71,
                parent: 1,
                unit: Some("wireplumber.service"),
                name: "PipeWire",
                session: true,
            },
        ];
        let keys: Vec<ApplicationKey> =
            assign(&processes, &known).into_iter().map(|a| a.key).collect();
        assert_eq!(keys[0], ApplicationKey::Named("PipeWire".into()));
        assert_eq!(keys[0], keys[1], "the sound server is one application, not three");

        let plasma = [
            Placement {
                pid: 80,
                parent: 1,
                unit: Some("plasma-plasmashell.service"),
                name: "Plasma",
                session: true,
            },
            Placement {
                pid: 81,
                parent: 80,
                unit: Some("plasma-plasmashell.service"),
                name: "kioworker",
                session: false,
            },
            Placement {
                pid: 90,
                parent: 1,
                unit: Some("plasma-kwin_wayland.service"),
                name: "KWin",
                session: true,
            },
            Placement {
                pid: 91,
                parent: 90,
                unit: Some("plasma-kwin_wayland.service"),
                name: "Xwayland",
                session: true,
            },
        ];
        let keys: Vec<ApplicationKey> =
            assign(&plasma, &known).into_iter().map(|a| a.key).collect();
        assert_eq!(keys[1], ApplicationKey::Named("Plasma".into()), "a worker Plasma started");
        assert_eq!(keys[2], ApplicationKey::Named("KWin".into()));
        assert_eq!(keys[3], ApplicationKey::Named("Xwayland".into()));
    }

    #[test]
    fn a_parent_cycle_does_not_hang() {
        let processes = [
            Placement { pid: 60, parent: 61, unit: None, name: "a", session: false },
            Placement { pid: 61, parent: 60, unit: None, name: "b", session: false },
        ];
        assert_eq!(assign(&processes, &known).len(), 2);
    }
}
