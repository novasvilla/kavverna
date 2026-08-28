use std::collections::BTreeMap;

pub type Properties = BTreeMap<String, String>;

/// What a stream is remembered by between runs. Node ids are recycled, so they cannot
/// carry a saved volume across a restart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppKey(String);

impl AppKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_refined(name: &str) -> Self {
        Self(normalise(name))
    }
}

impl std::fmt::Display for AppKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

const IDENTIFYING: [&str; 3] =
    ["application.id", "pipewire.access.portal.app_id", "application.process.binary"];

const PROCESS_ID: [&str; 2] = ["application.process.id", "pipewire.sec.pid"];

pub fn app_key(node: &Properties, client: Option<&Properties>) -> AppKey {
    app_key_resolving(node, client, binary_of_process)
}

/// Falls back through the properties an application may or may not set, then to the owning
/// client, and finally to the running process, because the registry hands out a process id
/// for clients that never declare their binary.
pub fn app_key_resolving(
    node: &Properties,
    client: Option<&Properties>,
    binary_of: impl Fn(u32) -> Option<String>,
) -> AppKey {
    let bags = [Some(node), client];

    for key in IDENTIFYING {
        for bag in bags.iter().flatten() {
            if let Some(value) = non_empty(bag, key) {
                return AppKey(normalise(value));
            }
        }
    }

    for key in PROCESS_ID {
        for bag in bags.iter().flatten() {
            if let Some(pid) = non_empty(bag, key).and_then(|value| value.parse::<u32>().ok())
                && let Some(binary) = binary_of(pid)
            {
                return AppKey(normalise(&binary));
            }
        }
    }

    for bag in bags.iter().flatten() {
        if let Some(value) = non_empty(bag, "application.name") {
            return AppKey(normalise(value));
        }
    }

    AppKey("unknown".into())
}

/// Clients reaching PipeWire through the PulseAudio bridge report the bridge's process, so
/// naming a stream after one of these would lump every such application together.
const BRIDGES: [&str; 4] = ["pipewire", "pipewire-pulse", "wireplumber", "pulseaudio"];

/// Names Electron and Chromium hand out for every application built on them, which would
/// otherwise show one row per framework instead of one per application.
const GENERIC: [&str; 5] = ["chromium", "electron", "chrome", "app", "unknown"];

pub fn is_generic(name: &str) -> bool {
    GENERIC.contains(&name.trim().to_lowercase().as_str())
}

/// Recovers the real application from a framework process's arguments. Electron is told
/// where to keep its data and which bundle to run, and both name the application.
pub fn refine_from_cmdline(args: &[String]) -> Option<String> {
    let from_data_dir =
        args.iter().find_map(|arg| arg.strip_prefix("--user-data-dir=")).and_then(last_segment);

    let from_bundle = args
        .iter()
        .find(|arg| arg.ends_with(".asar"))
        .and_then(|path| path.rsplit_once('/'))
        .and_then(|(parent, _)| last_segment(parent));

    from_data_dir.or(from_bundle).filter(|name| !is_generic(name))
}

fn last_segment(path: &str) -> Option<String> {
    path.trim_end_matches('/').rsplit('/').find(|part| !part.is_empty()).map(str::to_owned)
}

/// Steam tells every game it launches which application it is, and the desktop entry Steam
/// writes for that game names its icon after the same number. That pairs a stream calling
/// itself SDL Application with the name a person would recognise, for any game rather than for
/// one that was thought of in advance.
pub fn steam_icon_of_process(pid: u32) -> Option<String> {
    let environ = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    environ
        .split(|byte| *byte == 0)
        .filter_map(|entry| std::str::from_utf8(entry).ok())
        .find_map(|entry| entry.strip_prefix("SteamAppId="))
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
        .map(|id| format!("steam_icon_{id}"))
}

pub fn cmdline_of_process(pid: u32) -> Vec<String> {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|raw| {
            raw.split(|byte| *byte == 0)
                .filter(|part| !part.is_empty())
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Turns a directory name into something worth showing in a list.
pub fn presentable(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => name.to_owned(),
    }
}

pub fn binary_of_process(pid: u32) -> Option<String> {
    let binary = std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()?
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())?;

    (!BRIDGES.contains(&binary.as_str())).then_some(binary)
}

/// The name shown in the mixer, which unlike the key is allowed to change between runs.
pub fn display_name(node: &Properties, client: Option<&Properties>) -> String {
    let bags = [Some(node), client];

    for key in ["application.name", "node.description", "node.name"] {
        for bag in bags.iter().flatten() {
            if let Some(value) = non_empty(bag, key) {
                return value.to_owned();
            }
        }
    }

    "Unknown application".into()
}

fn non_empty<'a>(bag: &'a Properties, key: &str) -> Option<&'a str> {
    bag.get(key).map(String::as_str).filter(|value| !value.trim().is_empty())
}

fn normalise(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> Properties {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn the_binary_wins_over_a_display_name() {
        let node =
            props(&[("application.name", "Chromium"), ("application.process.binary", "electron")]);

        assert_eq!(app_key_resolving(&node, None, |_| None).as_str(), "electron");
    }

    /// A second stream from the same application often carries nothing but its name, while
    /// the client behind it still knows the binary.
    #[test]
    fn a_bare_stream_is_identified_through_its_client() {
        let node = props(&[("application.name", "SDL Application")]);
        let client = props(&[
            ("application.name", "SDL Application"),
            ("application.process.binary", "dota2"),
        ]);

        assert_eq!(app_key_resolving(&node, Some(&client), |_| None).as_str(), "dota2");
    }

    #[test]
    fn both_streams_of_one_application_share_a_key() {
        let with_binary = props(&[
            ("application.name", "SDL Application"),
            ("application.process.binary", "dota2"),
        ]);
        let bare = props(&[("application.name", "SDL Application")]);
        let client = props(&[("application.process.binary", "dota2")]);

        assert_eq!(
            app_key_resolving(&with_binary, None, |_| None),
            app_key_resolving(&bare, Some(&client), |_| None)
        );
    }

    /// The registry hands out `pipewire.sec.pid` for clients that never set a binary.
    #[test]
    fn a_client_with_only_a_process_id_is_keyed_by_its_binary() {
        let node = props(&[("application.name", "SDL Application")]);
        let client =
            props(&[("application.name", "SDL Application"), ("pipewire.sec.pid", "541940")]);

        let key = app_key_resolving(&node, Some(&client), |pid| {
            (pid == 541940).then(|| "dota2".to_owned())
        });

        assert_eq!(key.as_str(), "dota2");
    }

    #[test]
    fn a_process_that_has_gone_falls_through_to_the_name() {
        let node = props(&[("application.name", "SDL Application"), ("pipewire.sec.pid", "1")]);

        let key = app_key_resolving(&node, None, |_| None);

        assert_eq!(key.as_str(), "sdl application");
    }

    #[test]
    fn a_sandboxed_application_is_keyed_by_its_portal_id() {
        let node = props(&[
            ("application.name", "Spotify"),
            ("pipewire.access.portal.app_id", "com.spotify.Client"),
            ("application.process.binary", "bwrap"),
        ]);

        assert_eq!(app_key_resolving(&node, None, |_| None).as_str(), "com.spotify.client");
    }

    #[test]
    fn a_stream_with_nothing_useful_still_gets_a_key() {
        assert_eq!(app_key_resolving(&props(&[]), None, |_| None).as_str(), "unknown");
        assert_eq!(
            app_key_resolving(&props(&[("application.name", "  ")]), None, |_| None).as_str(),
            "unknown"
        );
    }

    /// Every Electron application calls itself Chromium, so a mixer would show several
    /// identical rows with no way to tell which is which.
    #[test]
    fn an_electron_application_is_recovered_from_its_data_directory() {
        let args: Vec<String> = [
            "/proc/self/exe",
            "--type=utility",
            "--user-data-dir=/home/someone/.config/vesktop",
            "--standard-schemes=vesktop",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

        assert_eq!(refine_from_cmdline(&args).as_deref(), Some("vesktop"));
        assert_eq!(presentable("vesktop"), "Vesktop");
    }

    #[test]
    fn the_bundle_path_names_the_application_when_the_data_directory_does_not() {
        let args: Vec<String> = ["/usr/lib/electron39/electron", "/usr/lib/vesktop/app.asar"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();

        assert_eq!(refine_from_cmdline(&args).as_deref(), Some("vesktop"));
    }

    #[test]
    fn a_data_directory_that_names_the_framework_is_no_better_than_what_we_had() {
        let args = vec!["--user-data-dir=/home/someone/.config/Electron".to_owned()];

        assert_eq!(refine_from_cmdline(&args), None);
    }

    #[test]
    fn arguments_that_name_nothing_refine_nothing() {
        assert_eq!(refine_from_cmdline(&[]), None);
        assert_eq!(refine_from_cmdline(&["--type=renderer".to_owned()]), None);
    }

    #[test]
    fn the_framework_names_are_the_ones_worth_replacing() {
        assert!(is_generic("Chromium") && is_generic("electron") && is_generic("unknown"));
        assert!(!is_generic("Vesktop") && !is_generic("Firefox"));
    }

    #[test]
    fn the_shown_name_keeps_its_capitals() {
        let node =
            props(&[("application.name", "Chromium"), ("application.process.binary", "electron")]);

        assert_eq!(display_name(&node, None), "Chromium");
        assert_eq!(display_name(&props(&[]), None), "Unknown application");
    }
}
