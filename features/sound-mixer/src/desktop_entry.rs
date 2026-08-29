//! What the desktop already knows about an installed application.
//!
//! A stream says what a toolkit called it, which for anything built on SDL, Electron or Qt is
//! the toolkit's name rather than the program's. The desktop entries say what a person calls
//! it, and they carry the icon too.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Default)]
struct Index {
    by_binary: HashMap<String, Entry>,
    by_icon: HashMap<String, Entry>,
    /// What a program calls itself, which for anything built on Electron is the only thing that
    /// matches: those report `electron` as their binary, and no entry runs a program by that
    /// name. `StartupWMClass` exists to tie an announced identity back to an entry, and the
    /// file's own name is the other half of the same convention.
    by_identity: HashMap<String, Entry>,
}

/// Read once. Entries change when software is installed, which is rare enough that rescanning
/// for every stream would be work for nothing.
fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| build(&search_path()))
}

pub fn named_after_binary(binary: &str) -> Option<&'static Entry> {
    index().by_binary.get(&binary.to_ascii_lowercase())
}

pub fn named_after_icon(icon: &str) -> Option<&'static Entry> {
    index().by_icon.get(icon)
}

pub fn named_after_identity(identity: &str) -> Option<&'static Entry> {
    index().by_identity.get(&identity.to_ascii_lowercase())
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

fn file_stem(path: &std::path::Path) -> Option<String> {
    path.file_stem().map(|stem| stem.to_string_lossy().into_owned())
}

struct Found {
    entry: Entry,
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
            _ => {}
        }
    }

    if kind.as_deref() != Some("Application") {
        return None;
    }
    Some(Found {
        entry: Entry { name: name?, icon },
        binary: try_command.or(command).and_then(|line| binary_of(&line)),
        window_class,
    })
}

/// The first word that is not an environment assignment or a launcher wrapper, reduced to its
/// file name, which is what a process reports as its own binary.
fn binary_of(exec: &str) -> Option<String> {
    let mut words = exec.split_whitespace().peekable();

    if words.peek() == Some(&"env") {
        words.next();
        while words.peek().is_some_and(|word| word.contains('=')) {
            words.next();
        }
    }

    let first = words.next()?;
    let name = first.rsplit('/').next()?;
    (!name.is_empty() && !name.starts_with('%')).then(|| name.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(room: &std::path::Path, file: &str, body: &str) {
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
        assert_eq!(binary_of("/usr/bin/firefox %u").as_deref(), Some("firefox"));
        assert_eq!(binary_of("env GDK_BACKEND=x11 /usr/bin/gimp").as_deref(), Some("gimp"));
        assert_eq!(binary_of("%f").as_deref(), None);
    }

    #[test]
    fn an_entry_is_also_found_by_its_icon() {
        let room = tempfile::tempdir().unwrap();
        written(
            room.path(),
            "game.desktop",
            "[Desktop Entry]\nType=Application\nName=Dota 2\nExec=steam steam://rungameid/570\nIcon=steam_icon_570\n",
        );
        let index = build(&[room.path().to_path_buf()]);

        assert_eq!(index.by_icon.get("steam_icon_570").unwrap().name, "Dota 2");
        // Its Exec runs the launcher, so the launcher is what it is indexed under.
        assert!(index.by_binary.contains_key("steam"));
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
}
