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
