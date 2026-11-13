use std::fs;
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

/// Writes through a temporary file in the same directory, so a reader never sees half a file.
/// Each Claude Code session runs its own status line command, so several writers can race: the
/// staging name carries the process id, and the last rename wins whole.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| io::Error::other("cache path has no directory"))?;
    let name = path.file_name().ok_or_else(|| io::Error::other("cache path has no file name"))?;

    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(DIR_MODE))?;

    let staging = parent.join(format!(".{}.{}.tmp", name.to_string_lossy(), std::process::id()));
    let staged = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(FILE_MODE)
            .open(&staging)?;
        io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
        fs::set_permissions(&staging, fs::Permissions::from_mode(FILE_MODE))?;
        fs::rename(&staging, path)
    })();
    if staged.is_err() {
        let _ = fs::remove_file(&staging);
    }
    staged
}
