//! The shelf on disk: one JSON file beside an items directory of staged blobs, all of it
//! readable by nobody else. The file is written on every mutation whatever the keep setting
//! says, so a crash loses nothing; what the setting decides is whether the next start reads
//! it back or sweeps it away.

use crate::item::Item;
use crate::pile::Pile;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    piles: Vec<Pile>,
}

pub fn items_dir(shelf_dir: &Path) -> PathBuf {
    shelf_dir.join("items")
}

fn shelf_file(shelf_dir: &Path) -> PathBuf {
    shelf_dir.join("shelf.json")
}

pub fn load(shelf_dir: &Path) -> io::Result<Vec<Pile>> {
    let bytes = match std::fs::read(shelf_file(shelf_dir)) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let saved: Saved = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if saved.version != SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported shelf version {}", saved.version),
        ));
    }
    Ok(saved.piles)
}

pub fn save(shelf_dir: &Path, piles: &[Pile]) -> std::io::Result<()> {
    std::fs::create_dir_all(shelf_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(shelf_dir, std::fs::Permissions::from_mode(0o700))?;
    }

    let saved = Saved { version: SCHEMA_VERSION, piles: piles.to_vec() };
    let text = serde_json::to_vec_pretty(&saved).map_err(io::Error::other)?;
    let path = shelf_file(shelf_dir);
    let mut staging = tempfile::NamedTempFile::new_in(shelf_dir)?;
    staging.write_all(&text)?;
    staging.as_file().sync_all()?;
    staging.persist(path).map_err(|err| err.error)?;
    std::fs::File::open(shelf_dir)?.sync_all()?;
    Ok(())
}

/// Deletes the saved list and every staged blob: what starting fresh means when keeping the
/// shelf across restarts is switched off.
pub fn forget(shelf_dir: &Path) {
    let _ = std::fs::remove_file(shelf_file(shelf_dir));
    if let Ok(entries) = std::fs::read_dir(items_dir(shelf_dir)) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Removes every blob no item names any more. Blobs outlive a drag-out on purpose, because
/// the receiving side copies the file after the drag reports done; the next start is late
/// enough to be safe.
pub fn sweep(shelf_dir: &Path, piles: &[Pile]) {
    let named: Vec<PathBuf> = piles
        .iter()
        .flat_map(|pile| pile.items.iter())
        .filter_map(Item::owned_blob)
        .map(Path::to_path_buf)
        .collect();

    let Ok(entries) = std::fs::read_dir(items_dir(shelf_dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !named.contains(&path) {
            tracing::info!(blob = %path.display(), "sweeping a blob nothing names");
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemKind;

    fn pile_of(id: u64, item: Item) -> Pile {
        Pile { id, items: vec![item] }
    }

    #[test]
    fn what_was_shelved_is_back_after_a_reload() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let item = Item {
            id: 7,
            kind: ItemKind::Link { url: "https://example.org".into(), label: None },
            bytes: None,
            image: None,
        };
        save(dir.path(), &[pile_of(1, item.clone())]).expect("saved");

        let back = load(dir.path()).expect("loaded");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].items[0], item);
    }

    #[test]
    fn forgetting_starts_empty_and_removes_the_blobs() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let staged =
            crate::staging::stage_blob(&items_dir(dir.path()), b"bytes", "txt").expect("staged");
        save(dir.path(), &[]).expect("saved");

        forget(dir.path());
        assert!(load(dir.path()).unwrap().is_empty());
        assert!(!staged.exists());
    }

    #[test]
    fn a_blob_no_item_names_is_swept_and_a_named_one_stays() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let named =
            crate::staging::stage_blob(&items_dir(dir.path()), b"kept", "txt").expect("staged");
        let orphan =
            crate::staging::stage_blob(&items_dir(dir.path()), b"orphan", "txt").expect("staged");

        let item = Item {
            id: 1,
            kind: ItemKind::Text { staged: named.clone(), preview: "kept".into() },
            bytes: Some(4),
            image: None,
        };
        sweep(dir.path(), &[pile_of(1, item)]);

        assert!(named.exists());
        assert!(!orphan.exists());
    }

    #[test]
    fn nothing_it_writes_is_readable_by_anyone_else() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("a tempdir");
        save(dir.path(), &[]).expect("saved");
        let staged =
            crate::staging::stage_blob(&items_dir(dir.path()), b"private", "txt").expect("staged");

        let file_mode = std::fs::metadata(dir.path().join("shelf.json")).unwrap().permissions();
        let blob_mode = std::fs::metadata(&staged).unwrap().permissions();
        let items_mode = std::fs::metadata(items_dir(dir.path())).unwrap().permissions();
        assert_eq!(file_mode.mode() & 0o777, 0o600);
        assert_eq!(blob_mode.mode() & 0o777, 0o600);
        assert_eq!(items_mode.mode() & 0o777, 0o700);
    }
}
