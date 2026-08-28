use std::io;
use std::path::PathBuf;

const ENTRY: &str = "kavverna.desktop";

/// The desktop entry is the state rather than a mirror of it in settings, so the toggle
/// cannot drift from what the session will actually do at login.
fn entry_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

    Some(config.join("autostart").join(ENTRY))
}

pub fn is_enabled() -> bool {
    entry_path().is_some_and(|path| path.exists())
}

pub fn set(enabled: bool) -> io::Result<()> {
    let path = entry_path().ok_or_else(|| io::Error::other("no config directory"))?;

    if !enabled {
        return match std::fs::remove_file(&path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            outcome => outcome,
        };
    }

    let binary = std::env::current_exe()?;
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Kavverna\n\
         Comment=Desktop utilities in one tray icon\n\
         Exec={}\n\
         Icon=applications-utilities\n\
         Categories=Utility;System;\n\
         Terminal=false\n\
         X-KDE-autostart-phase=2\n",
        binary.display()
    );

    std::fs::create_dir_all(path.parent().expect("autostart path has a parent"))?;
    std::fs::write(&path, entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests move HOME, so they cannot run beside anything else reading it.
    static HOME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn in_temporary_home(body: impl FnOnce(&std::path::Path)) {
        let _guard = HOME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let home = tempfile::tempdir().expect("tempdir");
        let previous = std::env::var_os("XDG_CONFIG_HOME");

        unsafe { std::env::set_var("XDG_CONFIG_HOME", home.path()) };
        body(home.path());
        match previous {
            Some(value) => unsafe { std::env::set_var("XDG_CONFIG_HOME", value) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
    }

    #[test]
    fn enabling_writes_an_entry_that_names_this_binary() {
        in_temporary_home(|home| {
            assert!(!is_enabled());

            set(true).expect("enable");

            assert!(is_enabled());
            let entry = std::fs::read_to_string(home.join("autostart").join(ENTRY))
                .expect("entry written");
            assert!(entry.contains("Type=Application"));
            assert!(entry.contains("Name=Kavverna"));
            assert!(entry.contains(&format!(
                "Exec={}",
                std::env::current_exe().unwrap().display()
            )));
        });
    }

    #[test]
    fn disabling_removes_the_entry() {
        in_temporary_home(|_| {
            set(true).expect("enable");
            set(false).expect("disable");

            assert!(!is_enabled());
        });
    }

    /// Switching it off twice is what happens when the toggle and a stale state disagree.
    #[test]
    fn disabling_when_already_off_is_not_an_error() {
        in_temporary_home(|_| {
            assert!(set(false).is_ok());
            assert!(set(false).is_ok());
        });
    }
}
