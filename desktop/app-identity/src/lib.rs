//! What the desktop already knows about the program behind a process.
//!
//! A stream says what a toolkit called it, which for anything built on SDL, Electron or Qt is
//! the toolkit's name rather than the program's; a process reports a binary that is often the
//! same story. The desktop entries say what a person calls it, and they carry the icon too.
//! Kept apart from any one feature so that whatever names a process next resolves it through
//! this index and calls it what Sound does.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopEntry {
    pub name: String,
    pub icon: Option<String>,
}

/// What a process could be identified as. `name` is empty-handed when nothing but a process id
/// was readable, which is the caller's cue to fall back to whatever the kernel calls it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub name: Option<String>,
    pub icon: Option<String>,
}

#[derive(Default)]
struct Index {
    by_binary: HashMap<String, DesktopEntry>,
    by_icon: HashMap<String, DesktopEntry>,
    /// What a program calls itself, which for anything built on Electron is the only thing that
    /// matches: those report `electron` as their binary, and no entry runs a program by that
    /// name. `StartupWMClass` exists to tie an announced identity back to an entry, and the
    /// file's own name is the other half of the same convention.
    by_identity: HashMap<String, DesktopEntry>,
}

/// Read once. Entries change when software is installed, which is rare enough that rescanning
/// for every stream or every sample would be work for nothing.
fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| build(&search_path()))
}

pub fn named_after_binary(binary: &str) -> Option<&'static DesktopEntry> {
    index().by_binary.get(&binary.to_ascii_lowercase())
}

pub fn named_after_icon(icon: &str) -> Option<&'static DesktopEntry> {
    index().by_icon.get(icon)
}

pub fn named_after_identity(identity: &str) -> Option<&'static DesktopEntry> {
    index().by_identity.get(&identity.to_ascii_lowercase())
}

/// Three ways in, best first: the game Steam says it launched, the identity a framework
/// process carries in its arguments, and the binary the entry runs.
pub fn process(pid: u32) -> ProcessIdentity {
    process_in(Path::new("/proc"), pid)
}

pub fn process_in(proc_root: &Path, pid: u32) -> ProcessIdentity {
    let binary = executable_in(proc_root, pid)
        .as_deref()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned());
    let refined = refine_from_cmdline(&cmdline_in(proc_root, pid));
    let entry = steam_icon_in(proc_root, pid)
        .as_deref()
        .and_then(named_after_icon)
        .or_else(|| refined.as_deref().and_then(named_after_identity))
        .or_else(|| binary.as_deref().and_then(named_after_binary));

    ProcessIdentity {
        name: entry
            .map(|entry| entry.name.clone())
            .or_else(|| refined.or(binary).map(|name| presentable(&name))),
        icon: entry.and_then(|entry| entry.icon.clone()),
    }
}

fn search_path() -> Vec<PathBuf> {
    let home = |tail: &str| {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
            .map(|base| base.join(tail))
    };
    let shared =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());
    home("applications")
        .into_iter()
        .chain(
            shared
                .split(':')
                .filter(|part| !part.is_empty())
                .map(|dir| PathBuf::from(dir).join("applications")),
        )
        .collect()
}

/// Earlier directories win, so an entry a user installed beats the one the system ships.
fn build(directories: &[PathBuf]) -> Index {
    let mut index = Index::default();
    for directory in directories {
        let Ok(listing) = std::fs::read_dir(directory) else {
            continue;
        };
        for file in listing.flatten() {
            let path = file.path();
            if path.extension().and_then(|kind| kind.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(found) = parse(&text) else {
                continue;
            };
            if let Some(binary) = found.binary {
                index.by_binary.entry(binary).or_insert_with(|| found.entry.clone());
            }
            for identity in [found.window_class, file_stem(&path)].into_iter().flatten() {
                index
                    .by_identity
                    .entry(identity.to_ascii_lowercase())
                    .or_insert_with(|| found.entry.clone());
            }
            if let Some(icon) = found.entry.icon.clone() {
                index.by_icon.entry(icon).or_insert(found.entry);
            }
        }
    }
    index
}

fn file_stem(path: &Path) -> Option<String> {
    path.file_stem().map(|stem| stem.to_string_lossy().into_owned())
}

struct Found {
    entry: DesktopEntry,
    binary: Option<String>,
    window_class: Option<String>,
}

fn parse(text: &str) -> Option<Found> {
    let mut name = None;
    let mut icon = None;
    let mut command = None;
    let mut try_command = None;
    let mut kind = None;
    let mut window_class = None;
    let mut hidden = false;
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[Desktop Entry]";
            continue;
        }
        if !inside {
            continue;
        }
        // Translations are `Name[es]`, and the untranslated key is the one to match on.
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "Name" => name = Some(value.trim().to_owned()),
            "Icon" => icon = Some(value.trim().to_owned()),
            "Exec" => command = Some(value.trim().to_owned()),
            "TryExec" => try_command = Some(value.trim().to_owned()),
            "Type" => kind = Some(value.trim().to_owned()),
            "StartupWMClass" => window_class = Some(value.trim().to_owned()),
            "NoDisplay" => hidden = value.trim() == "true",
            _ => {}
        }
    }
    // An entry hidden from menus is a helper, a URL handler or a wrapper, and it runs the same
    // binary as the entry a person knows: `code-url-handler.desktop` would otherwise name
    // every editor window "Visual Studio Code - URL Handler".
    if kind.as_deref() != Some("Application") || hidden {
        return None;
    }
    Some(Found {
        entry: DesktopEntry { name: name?, icon },
        binary: try_command.or(command).and_then(|line| binary_of_exec(&line)),
        window_class,
    })
}

/// The first word that is not an environment assignment or a launcher wrapper, reduced to its
/// file name, which is what a process reports as its own binary. An entry that hands a
/// launcher an address, `steam steam://rungameid/570`, names the game, not the launcher, so it
/// claims no binary at all: the Steam client would otherwise wear the name of whichever game
/// entry was read first.
fn binary_of_exec(exec: &str) -> Option<String> {
    let mut words = exec.split_whitespace().peekable();
    if words.peek() == Some(&"env") {
        words.next();
        while words.peek().is_some_and(|word| word.contains('=')) {
            words.next();
        }
    }
    let name = words.next()?.rsplit('/').next()?;
    if words.any(|word| word.contains("://")) {
        return None;
    }
    (!name.is_empty() && !name.starts_with('%')).then(|| name.to_ascii_lowercase())
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
    steam_icon_in(Path::new("/proc"), pid)
}

fn steam_icon_in(proc_root: &Path, pid: u32) -> Option<String> {
    let environ = std::fs::read(proc_root.join(pid.to_string()).join("environ")).ok()?;
    environ
        .split(|byte| *byte == 0)
        .filter_map(|entry| std::str::from_utf8(entry).ok())
        .find_map(|entry| entry.strip_prefix("SteamAppId="))
        .filter(|id| !id.is_empty() && id.chars().all(|character| character.is_ascii_digit()))
        .map(|id| format!("steam_icon_{id}"))
}

pub fn cmdline_of_process(pid: u32) -> Vec<String> {
    cmdline_in(Path::new("/proc"), pid)
}

fn cmdline_in(proc_root: &Path, pid: u32) -> Vec<String> {
    std::fs::read(proc_root.join(pid.to_string()).join("cmdline"))
        .map(|raw| {
            raw.split(|byte| *byte == 0)
                .filter(|part| !part.is_empty())
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Turns a directory or binary name into something worth showing in a list.
pub fn presentable(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => name.to_owned(),
    }
}

fn executable_in(proc_root: &Path, pid: u32) -> Option<PathBuf> {
    std::fs::read_link(proc_root.join(pid.to_string()).join("exe")).ok()
}

pub fn binary_of_process(pid: u32) -> Option<String> {
    let binary = executable_in(Path::new("/proc"), pid)?
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())?;
    (!BRIDGES.contains(&binary.as_str())).then_some(binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(room: &Path, file: &str, body: &str) {
        std::fs::write(room.join(file), body).unwrap();
    }

    #[test]
    fn an_application_is_found_by_the_binary_it_runs() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=Kate\nExec=kate %U\nIcon=kate\n",
        );
        let index = build(&[room.path().to_path_buf()]);

        let found = index.by_binary.get("kate").unwrap();
        assert_eq!(found.name, "Kate");
        assert_eq!(found.icon.as_deref(), Some("kate"));
    }

    #[test]
    fn a_full_path_and_an_env_wrapper_still_name_the_binary() {
        assert_eq!(binary_of_exec("/usr/bin/firefox %u").as_deref(), Some("firefox"));
        assert_eq!(binary_of_exec("env GDK_BACKEND=x11 /usr/bin/gimp").as_deref(), Some("gimp"));
        assert_eq!(binary_of_exec("%f").as_deref(), None);
    }

    #[test]
    fn a_game_entry_is_found_by_its_icon_and_does_not_claim_the_launcher() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "game.desktop",
            "[Desktop Entry]\nType=Application\nName=Dota 2\nExec=steam steam://rungameid/570\nIcon=steam_icon_570\n",
        );
        written(
            room.path(),
            "steam.desktop",
            "[Desktop Entry]\nType=Application\nName=Steam\nExec=/usr/bin/steam %U\nIcon=steam\n",
        );
        let index = build(&[room.path().to_path_buf()]);

        assert_eq!(index.by_icon.get("steam_icon_570").unwrap().name, "Dota 2");
        assert_eq!(index.by_binary.get("steam").unwrap().name, "Steam");
    }

    /// The case every Electron program lands in: it reports `electron` as its binary, which no
    /// entry runs, so the only way back to its entry is the identity it announces.
    #[test]
    fn a_program_is_found_by_the_identity_it_announces() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "vesktop.desktop",
            "[Desktop Entry]\nType=Application\nName=Vesktop\nExec=vesktop %U\n\
             Icon=vesktop\nStartupWMClass=vesktop\n",
        );
        let index = build(&[room.path().to_path_buf()]);

        let found = index.by_identity.get("vesktop").unwrap();
        assert_eq!(found.name, "Vesktop");
        assert_eq!(found.icon.as_deref(), Some("vesktop"));
        assert!(!index.by_binary.contains_key("electron"), "the binary leads nowhere, as expected");
    }

    /// The other half of the same convention: the file is named after the program even when the
    /// entry says nothing about a window class.
    #[test]
    fn the_entry_file_name_is_an_identity_too() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "org.kde.kate.desktop",
            "[Desktop Entry]\nType=Application\nName=Kate\nExec=kate %U\nIcon=kate\n",
        );
        let index = build(&[room.path().to_path_buf()]);

        assert_eq!(index.by_identity.get("org.kde.kate").unwrap().name, "Kate");
    }

    #[test]
    fn an_identity_is_matched_whatever_its_case() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "signal.desktop",
            "[Desktop Entry]\nType=Application\nName=Signal\nExec=signal-desktop\n\
             StartupWMClass=Signal\n",
        );
        let index = build(&[room.path().to_path_buf()]);

        assert!(index.by_identity.contains_key("signal"), "stored lowercased");
    }

    #[test]
    fn anything_that_is_not_an_application_is_left_out() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "link.desktop",
            "[Desktop Entry]\nType=Link\nName=Somewhere\nURL=https://example.org\n",
        );
        assert!(build(&[room.path().to_path_buf()]).by_binary.is_empty());
    }

    #[test]
    fn an_entry_hidden_from_menus_names_nothing() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "code-url-handler.desktop",
            "[Desktop Entry]\nType=Application\nName=Visual Studio Code - URL Handler\nExec=/usr/bin/code --open-url %U\nNoDisplay=true\n",
        );
        written(
            room.path(),
            "code.desktop",
            "[Desktop Entry]\nType=Application\nName=Visual Studio Code\nExec=/usr/bin/code %F\n",
        );
        let index = build(&[room.path().to_path_buf()]);
        assert_eq!(index.by_binary["code"].name, "Visual Studio Code");
    }

    #[test]
    fn keys_outside_the_first_group_are_not_read() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "act.desktop",
            "[Desktop Entry]\nType=Application\nName=Real\nExec=real\n\n[Desktop Action Other]\nName=Other\nExec=other\n",
        );
        let index = build(&[room.path().to_path_buf()]);
        assert!(index.by_binary.contains_key("real"));
        assert!(!index.by_binary.contains_key("other"));
    }

    #[test]
    fn framework_arguments_recover_the_application_name() {
        let args = vec!["electron".into(), "--user-data-dir=/home/person/.config/vesktop".into()];
        assert_eq!(refine_from_cmdline(&args).as_deref(), Some("vesktop"));

        let bundle =
            vec!["/usr/lib/electron39/electron".into(), "/usr/lib/vesktop/app.asar".into()];
        assert_eq!(refine_from_cmdline(&bundle).as_deref(), Some("vesktop"));

        let generic = vec!["chrome".into(), "--user-data-dir=/tmp/chromium".into()];
        assert_eq!(refine_from_cmdline(&generic), None);
    }

    #[test]
    fn a_name_is_presentable_with_its_first_letter_raised() {
        assert_eq!(presentable("vesktop"), "Vesktop");
        assert_eq!(presentable(""), "");
    }

    #[test]
    fn a_framework_name_is_generic_whatever_its_case() {
        assert!(is_generic("Chromium"));
        assert!(is_generic(" electron "));
        assert!(!is_generic("dota2"));
    }

    /// A process tree written by hand: the identity readers take the root, so nothing here
    /// looks at the machine's own processes.
    #[test]
    fn a_process_is_named_from_its_arguments_or_its_binary_and_otherwise_not_at_all() {
        let root = tempfile::tempdir().unwrap();
        let process = root.path().join("41");
        std::fs::create_dir(&process).unwrap();
        std::fs::write(
            process.join("cmdline"),
            b"electron\0--user-data-dir=/home/me/.config/vesktop\0",
        )
        .unwrap();
        std::fs::write(process.join("environ"), b"HOME=/home/me\0").unwrap();
        assert_eq!(process_in(root.path(), 41).name.as_deref(), Some("Vesktop"));

        let bare = root.path().join("42");
        std::fs::create_dir(&bare).unwrap();
        std::os::unix::fs::symlink("/opt/nowhere/nobody-wrote-an-entry", bare.join("exe")).unwrap();
        assert_eq!(process_in(root.path(), 42).name.as_deref(), Some("Nobody-wrote-an-entry"));

        assert_eq!(process_in(root.path(), 43), ProcessIdentity { name: None, icon: None });
        assert_eq!(steam_icon_in(root.path(), 41), None);
    }

    #[test]
    fn the_game_steam_launched_is_read_from_its_environment() {
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("570");
        std::fs::create_dir(&game).unwrap();
        std::fs::write(game.join("environ"), b"SteamAppId=570\0HOME=/home/me\0").unwrap();
        assert_eq!(steam_icon_in(root.path(), 570).as_deref(), Some("steam_icon_570"));
    }
}
