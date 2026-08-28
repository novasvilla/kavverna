use std::fs;
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

/// Writes through a temporary file in the same directory so an interrupted save leaves the
/// previous settings intact rather than a truncated file.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("settings path has no directory"))?;

    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(DIR_MODE))?;

    let staging = path.with_extension("json.tmp");
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(FILE_MODE)
            .open(&staging)?;
        io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
    }

    fs::set_permissions(&staging, fs::Permissions::from_mode(FILE_MODE))?;
    fs::rename(&staging, path)
}
