//! Adopting Plasma's clipboard history.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::entry::{self, Kind};
use crate::store::{Captured, Store, StoreError};

const DATABASE: &str = "klipper/history3.sqlite";

pub fn history_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
    })?;
    let path = base.join(DATABASE);
    path.is_file().then_some(path)
}

pub fn waiting() -> usize {
    let Some(path) = history_path() else {
        return 0;
    };
    read(&path).map(|entries| entries.len()).unwrap_or(0)
}

/// Oldest first, so the newest ends up on top as it was in Klipper.
pub fn import_into(store: &mut Store) -> Result<usize, StoreError> {
    let Some(path) = history_path() else {
        return Ok(0);
    };
    let Some(entries) = read(&path) else {
        return Ok(0);
    };

    let mut adopted = 0;
    for saved in entries {
        let Some(text) = entry::storable_text(&saved.text) else {
            continue;
        };
        let id = store.remember(Captured {
            kind: Kind::Text,
            text,
            file_paths: Vec::new(),
            image: None,
        })?;
        store.backdate(id, saved.copied_at)?;
        if saved.starred {
            store.set_pinned(id, true)?;
        }
        adopted += 1;
    }

    tracing::info!(adopted, "adopted Plasma's clipboard history");
    Ok(adopted)
}

struct Saved {
    text: String,
    copied_at: SystemTime,
    starred: bool,
}

/// Klipper holds the database open with write-ahead logging, so the copy is read and its file
/// is never touched.
fn read(path: &std::path::Path) -> Option<Vec<Saved>> {
    let room = tempfile::tempdir().ok()?;
    let copy = room.path().join("history.sqlite");
    std::fs::copy(path, &copy).ok()?;
    for tail in ["-wal", "-shm"] {
        let beside = path.with_file_name(format!("history3.sqlite{tail}"));
        if beside.is_file() {
            let _ = std::fs::copy(&beside, room.path().join(format!("history.sqlite{tail}")));
        }
    }

    let db = Connection::open(&copy).ok()?;
    let mut statement = db
        .prepare(
            "SELECT text, added_time, starred FROM main
             WHERE text IS NOT NULL AND text <> ''
             ORDER BY added_time ASC",
        )
        .ok()?;

    let rows = statement
        .query_map([], |row| {
            Ok(Saved {
                text: row.get(0)?,
                // Klipper stores seconds with a fraction, not milliseconds.
                copied_at: UNIX_EPOCH
                    + Duration::from_secs_f64(row.get::<_, f64>(1).unwrap_or(0.0).max(0.0)),
                starred: row.get::<_, Option<bool>>(2).unwrap_or_default().unwrap_or(false),
            })
        })
        .ok()?;

    rows.collect::<Result<Vec<_>, _>>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_history_written_the_way_klipper_writes_one_is_adopted_in_order() {
        let room = tempfile::tempdir().expect("a temporary directory");
        let theirs = room.path().join("klipper");
        std::fs::create_dir_all(&theirs).unwrap();
        let path = theirs.join("history3.sqlite");

        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE main (uuid char(40) PRIMARY KEY, added_time REAL NOT NULL,
                 last_used_time REAL, mimetypes TEXT NOT NULL, text NTEXT, starred BOOLEAN);
             INSERT INTO main VALUES ('a', 1000.5, NULL, 'text/plain', 'older', 0);
             INSERT INTO main VALUES ('b', 2000.5, NULL, 'text/plain', 'newer', 1);
             INSERT INTO main VALUES ('c', 3000.5, NULL, 'text/plain', '', 0);",
        )
        .unwrap();
        drop(db);

        let adopted = read(&path).expect("their history should be readable");
        assert_eq!(adopted.len(), 2, "an entry with no text has nothing to adopt");
        assert_eq!(adopted[0].text, "older", "oldest first");
        assert!(adopted[1].starred);
        assert_eq!(adopted[0].copied_at, UNIX_EPOCH + Duration::from_secs_f64(1000.5));
    }

    #[test]
    fn a_machine_without_klipper_has_nothing_to_adopt() {
        let room = tempfile::tempdir().expect("a temporary directory");
        assert!(read(&room.path().join("missing.sqlite")).is_none());
    }

    #[test]
    fn adopting_twice_does_not_double_the_history() {
        let room = tempfile::tempdir().expect("a temporary directory");
        let mut store = Store::open(room.path()).unwrap();
        let saved = Captured {
            kind: Kind::Text,
            text: "already here".into(),
            file_paths: Vec::new(),
            image: None,
        };
        store.remember(saved).unwrap();

        let again = Captured {
            kind: Kind::Text,
            text: "already here".into(),
            file_paths: Vec::new(),
            image: None,
        };
        store.remember(again).unwrap();
        assert_eq!(store.counts().unwrap(), (0, 1));
    }
}
