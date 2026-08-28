//! Where saved copies live.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::entry::{Entry, Kind, MAX_FILES, StoredImage};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS entry (
    id           INTEGER PRIMARY KEY,
    kind         TEXT    NOT NULL,
    body         TEXT    NOT NULL DEFAULT '',
    file_paths   TEXT    NOT NULL DEFAULT '',
    image_digest TEXT,
    image_width  INTEGER,
    image_height INTEGER,
    copied_at    INTEGER NOT NULL,
    pinned_at    INTEGER,
    position     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS entry_order ON entry (pinned_at, position);
CREATE VIRTUAL TABLE IF NOT EXISTS entry_search USING fts5(body);
";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the clipboard database is unusable: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("could not reach {path}: {source}")]
    Directory { path: PathBuf, source: std::io::Error },
}

pub struct Captured {
    pub kind: Kind,
    pub text: String,
    pub file_paths: Vec<PathBuf>,
    pub image: Option<(StoredImage, Vec<u8>)>,
}

/// A list row. The full text is fetched separately, so a long history costs only previews.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Summary {
    pub id: i64,
    pub kind: Kind,
    pub preview: String,
    pub pinned: bool,
    pub copied_at: SystemTime,
    pub image: Option<StoredImage>,
    pub file_count: usize,
}

pub struct Store {
    db: Connection,
    images: PathBuf,
}

impl Store {
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        let images = root.join("clipboard-images");
        make_directory(root)?;
        make_directory(&images)?;

        let db = Connection::open(root.join("clipboard.db"))?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        db.execute_batch(SCHEMA)?;

        Ok(Self { db, images })
    }

    #[cfg(test)]
    fn in_memory(images: PathBuf) -> Result<Self, StoreError> {
        let db = Connection::open_in_memory()?;
        db.execute_batch(SCHEMA)?;
        make_directory(&images)?;
        Ok(Self { db, images })
    }

    pub fn image_path(&self, digest: &str) -> PathBuf {
        self.images.join(format!("{digest}.png"))
    }

    /// Pinned first, then the rest, each newest at the top.
    pub fn summaries(&self) -> Result<Vec<Summary>, StoreError> {
        let mut statement = self.db.prepare(
            "SELECT id, kind, body, file_paths, image_digest, image_width, image_height,
                    copied_at, pinned_at
             FROM entry
             ORDER BY (pinned_at IS NULL), position DESC",
        )?;
        let rows = statement.query_map([], |row| summary_from(row))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn search(&self, query: &str) -> Result<Vec<Summary>, StoreError> {
        let Some(pattern) = match_pattern(query) else {
            return self.summaries();
        };
        let mut statement = self.db.prepare(
            "SELECT e.id, e.kind, e.body, e.file_paths, e.image_digest, e.image_width,
                    e.image_height, e.copied_at, e.pinned_at
             FROM entry_search s JOIN entry e ON e.id = s.rowid
             WHERE entry_search MATCH ?1
             ORDER BY e.position DESC",
        )?;
        let rows = statement.query_map(params![pattern], |row| summary_from(row))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn entry(&self, id: i64) -> Result<Option<Entry>, StoreError> {
        let found = self
            .db
            .query_row(
                "SELECT id, kind, body, file_paths, image_digest, image_width, image_height,
                        copied_at, pinned_at
                 FROM entry WHERE id = ?1",
                params![id],
                |row| entry_from(row),
            )
            .optional()?;
        Ok(found)
    }

    /// A repeat keeps its identity and its pin, and returns to the top of its group.
    pub fn remember(&mut self, captured: Captured) -> Result<i64, StoreError> {
        if let Some((image, bytes)) = &captured.image {
            let path = self.image_path(&image.digest);
            if !path.exists() {
                fs::write(&path, bytes)
                    .map_err(|source| StoreError::Directory { path: path.clone(), source })?;
            }
        }

        let paths = join_paths(&captured.file_paths);
        let digest = captured.image.as_ref().map(|(image, _)| image.digest.clone());
        let existing: Option<i64> = self
            .db
            .query_row(
                "SELECT id FROM entry
                 WHERE kind = ?1
                   AND ((?1 = 'text'  AND body = ?2)
                     OR (?1 = 'files' AND file_paths = ?3)
                     OR (?1 = 'image' AND image_digest IS NOT NULL AND image_digest = ?4))
                 LIMIT 1",
                params![captured.kind.as_str(), captured.text, paths, digest],
                |row| row.get(0),
            )
            .optional()?;

        let now = millis(SystemTime::now());
        let position = self.next_position()?;

        if let Some(id) = existing {
            self.db.execute(
                "UPDATE entry SET copied_at = ?2, position = ?3 WHERE id = ?1",
                params![id, now, position],
            )?;
            return Ok(id);
        }

        let image = captured.image.as_ref().map(|(image, _)| image);
        self.db.execute(
            "INSERT INTO entry
                (kind, body, file_paths, image_digest, image_width, image_height,
                 copied_at, pinned_at, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
            params![
                captured.kind.as_str(),
                captured.text,
                paths,
                image.map(|image| image.digest.clone()),
                image.map(|image| image.width),
                image.map(|image| image.height),
                now,
                position,
            ],
        )?;
        let id = self.db.last_insert_rowid();

        let entry = self.entry(id)?.expect("the row was just written");
        self.db.execute(
            "INSERT INTO entry_search(rowid, body) VALUES (?1, ?2)",
            params![id, entry.searchable_text()],
        )?;
        Ok(id)
    }

    pub fn touch(&mut self, id: i64) -> Result<(), StoreError> {
        let position = self.next_position()?;
        self.db.execute(
            "UPDATE entry SET copied_at = ?2, position = ?3 WHERE id = ?1",
            params![id, millis(SystemTime::now()), position],
        )?;
        Ok(())
    }

    /// Keeps an adopted entry's original time instead of the time it was imported.
    pub fn backdate(&mut self, id: i64, copied_at: SystemTime) -> Result<(), StoreError> {
        self.db.execute(
            "UPDATE entry SET copied_at = ?2 WHERE id = ?1",
            params![id, millis(copied_at)],
        )?;
        Ok(())
    }

    pub fn rewrite(&mut self, id: i64, text: &str) -> Result<bool, StoreError> {
        let changed = self.db.execute(
            "UPDATE entry SET body = ?2 WHERE id = ?1 AND kind = 'text'",
            params![id, text],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        self.db.execute("DELETE FROM entry_search WHERE rowid = ?1", params![id])?;
        self.db.execute(
            "INSERT INTO entry_search(rowid, body) VALUES (?1, ?2)",
            params![id, text],
        )?;
        Ok(true)
    }

    pub fn set_pinned(&mut self, id: i64, pinned: bool) -> Result<(), StoreError> {
        let stamp = pinned.then(|| millis(SystemTime::now()));
        let position = self.next_position()?;
        self.db.execute(
            "UPDATE entry SET pinned_at = ?2, position = ?3 WHERE id = ?1",
            params![id, stamp, position],
        )?;
        Ok(())
    }

    /// Swaps with the neighbour inside the same group, so pinned and recent never mix.
    pub fn move_entry(&mut self, id: i64, towards_top: bool) -> Result<bool, StoreError> {
        let Some((pinned, position)) = self
            .db
            .query_row(
                "SELECT pinned_at IS NOT NULL, position FROM entry WHERE id = ?1",
                params![id],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
        else {
            return Ok(false);
        };

        let neighbour: Option<(i64, i64)> = self
            .db
            .query_row(
                &format!(
                    "SELECT id, position FROM entry
                     WHERE (pinned_at IS NOT NULL) = ?1 AND position {} ?2
                     ORDER BY position {} LIMIT 1",
                    if towards_top { ">" } else { "<" },
                    if towards_top { "ASC" } else { "DESC" },
                ),
                params![pinned, position],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let Some((other_id, other_position)) = neighbour else {
            return Ok(false);
        };
        let swap = self.db.transaction()?;
        swap.execute("UPDATE entry SET position = ?2 WHERE id = ?1", params![id, other_position])?;
        swap.execute("UPDATE entry SET position = ?2 WHERE id = ?1", params![other_id, position])?;
        swap.commit()?;
        Ok(true)
    }

    pub fn forget(&mut self, id: i64) -> Result<(), StoreError> {
        self.db.execute("DELETE FROM entry WHERE id = ?1", params![id])?;
        self.db.execute("DELETE FROM entry_search WHERE rowid = ?1", params![id])?;
        self.sweep_images()
    }

    /// Pinned entries are never removed in bulk, only one at a time.
    pub fn clear_unpinned(&mut self) -> Result<(), StoreError> {
        self.db.execute(
            "DELETE FROM entry_search WHERE rowid IN (SELECT id FROM entry WHERE pinned_at IS NULL)",
            [],
        )?;
        self.db.execute("DELETE FROM entry WHERE pinned_at IS NULL", [])?;
        self.sweep_images()
    }

    /// Pinned entries do not count toward the limit.
    pub fn trim_to(&mut self, limit: u32) -> Result<(), StoreError> {
        if limit == 0 {
            return Ok(());
        }
        self.db.execute(
            "DELETE FROM entry_search WHERE rowid IN (
                 SELECT id FROM entry WHERE pinned_at IS NULL
                 ORDER BY position DESC LIMIT -1 OFFSET ?1)",
            params![limit],
        )?;
        let removed = self.db.execute(
            "DELETE FROM entry WHERE id IN (
                 SELECT id FROM entry WHERE pinned_at IS NULL
                 ORDER BY position DESC LIMIT -1 OFFSET ?1)",
            params![limit],
        )?;
        if removed > 0 { self.sweep_images() } else { Ok(()) }
    }

    pub fn counts(&self) -> Result<(usize, usize), StoreError> {
        let pinned: i64 = self
            .db
            .query_row("SELECT COUNT(*) FROM entry WHERE pinned_at IS NOT NULL", [], |row| {
                row.get(0)
            })?;
        let recent: i64 = self
            .db
            .query_row("SELECT COUNT(*) FROM entry WHERE pinned_at IS NULL", [], |row| row.get(0))?;
        Ok((pinned as usize, recent as usize))
    }

    fn next_position(&self) -> Result<i64, StoreError> {
        let highest: i64 =
            self.db.query_row("SELECT COALESCE(MAX(position), 0) FROM entry", [], |row| {
                row.get(0)
            })?;
        Ok(highest + 1)
    }

    fn sweep_images(&self) -> Result<(), StoreError> {
        let mut statement = self
            .db
            .prepare("SELECT image_digest FROM entry WHERE image_digest IS NOT NULL")?;
        let kept: Vec<String> =
            statement.query_map([], |row| row.get(0))?.collect::<Result<_, _>>()?;

        let Ok(listing) = fs::read_dir(&self.images) else {
            return Ok(());
        };
        for file in listing.flatten() {
            let name = file.file_name();
            let name = name.to_string_lossy();
            let Some(digest) = name.strip_suffix(".png") else {
                continue;
            };
            if !kept.iter().any(|held| held == digest) {
                let _ = fs::remove_file(file.path());
            }
        }
        Ok(())
    }
}

/// FTS5 reads its own syntax, so terms are quoted: typing NOT searches for the word.
fn match_pattern(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| term.replace('"', ""))
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" AND "))
}

fn summary_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<Summary> {
    let entry = entry_from(row)?;
    Ok(Summary {
        id: entry.id,
        kind: entry.kind,
        preview: entry.preview(),
        pinned: entry.is_pinned(),
        copied_at: entry.copied_at,
        image: entry.image.clone(),
        file_count: entry.file_paths.len(),
    })
}

fn entry_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let digest: Option<String> = row.get(4)?;
    let image = digest.map(|digest| StoredImage {
        digest,
        width: row.get::<_, Option<u32>>(5).unwrap_or_default().unwrap_or_default(),
        height: row.get::<_, Option<u32>>(6).unwrap_or_default().unwrap_or_default(),
    });

    Ok(Entry {
        id: row.get(0)?,
        kind: Kind::parse(&row.get::<_, String>(1)?).unwrap_or(Kind::Text),
        text: row.get(2)?,
        file_paths: split_paths(&row.get::<_, String>(3)?),
        image,
        copied_at: from_millis(row.get(7)?),
        pinned_at: row.get::<_, Option<i64>>(8)?.map(from_millis),
    })
}

fn join_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .take(MAX_FILES)
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn split_paths(raw: &str) -> Vec<PathBuf> {
    raw.lines().filter(|line| !line.is_empty()).map(PathBuf::from).collect()
}

fn millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).map(|since| since.as_millis() as i64).unwrap_or(0)
}

fn from_millis(millis: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis.max(0) as u64)
}

fn make_directory(path: &Path) -> Result<(), StoreError> {
    fs::create_dir_all(path)
        .map_err(|source| StoreError::Directory { path: path.to_path_buf(), source })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Store, tempfile::TempDir) {
        let room = tempfile::tempdir().expect("a temporary directory");
        let store = Store::in_memory(room.path().join("images")).expect("an empty store");
        (store, room)
    }

    fn text(body: &str) -> Captured {
        Captured {
            kind: Kind::Text,
            text: body.into(),
            file_paths: Vec::new(),
            image: None,
        }
    }

    #[test]
    fn the_newest_copy_is_at_the_top() {
        let (mut store, _room) = store();
        store.remember(text("first")).unwrap();
        store.remember(text("second")).unwrap();

        let previews: Vec<String> =
            store.summaries().unwrap().into_iter().map(|row| row.preview).collect();
        assert_eq!(previews, vec!["second", "first"]);
    }

    #[test]
    fn copying_the_same_thing_again_moves_it_rather_than_adding_it() {
        let (mut store, _room) = store();
        let first = store.remember(text("keep me")).unwrap();
        store.remember(text("something else")).unwrap();
        let again = store.remember(text("keep me")).unwrap();

        assert_eq!(first, again, "the entry keeps its identity");
        assert_eq!(store.counts().unwrap(), (0, 2));
        assert_eq!(store.summaries().unwrap()[0].preview, "keep me");
    }

    #[test]
    fn a_pinned_entry_keeps_its_pin_when_copied_again() {
        let (mut store, _room) = store();
        let id = store.remember(text("kept")).unwrap();
        store.set_pinned(id, true).unwrap();
        store.remember(text("kept")).unwrap();

        assert!(store.summaries().unwrap()[0].pinned);
        assert_eq!(store.counts().unwrap(), (1, 0));
    }

    #[test]
    fn pinned_entries_sit_above_the_rest() {
        let (mut store, _room) = store();
        let old = store.remember(text("old")).unwrap();
        store.remember(text("new")).unwrap();
        store.set_pinned(old, true).unwrap();

        let previews: Vec<String> =
            store.summaries().unwrap().into_iter().map(|row| row.preview).collect();
        assert_eq!(previews, vec!["old", "new"]);
    }

    #[test]
    fn the_limit_never_removes_a_pinned_entry() {
        let (mut store, _room) = store();
        let kept = store.remember(text("pinned")).unwrap();
        store.set_pinned(kept, true).unwrap();
        for index in 0..5 {
            store.remember(text(&format!("entry {index}"))).unwrap();
        }

        store.trim_to(2).unwrap();
        assert_eq!(store.counts().unwrap(), (1, 2));
        assert!(store.entry(kept).unwrap().is_some());
    }

    #[test]
    fn clearing_leaves_the_pinned_entries_alone() {
        let (mut store, _room) = store();
        let kept = store.remember(text("pinned")).unwrap();
        store.set_pinned(kept, true).unwrap();
        store.remember(text("passing through")).unwrap();

        store.clear_unpinned().unwrap();
        assert_eq!(store.counts().unwrap(), (1, 0));
    }

    #[test]
    fn search_finds_a_word_from_the_middle_of_an_entry() {
        let (mut store, _room) = store();
        store.remember(text("the quick brown fox")).unwrap();
        store.remember(text("nothing to see")).unwrap();

        let found = store.search("brown").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].preview, "the quick brown fox");
    }

    #[test]
    fn search_matches_a_word_that_has_only_been_started() {
        let (mut store, _room) = store();
        store.remember(text("kavverna")).unwrap();
        assert_eq!(store.search("kavv").unwrap().len(), 1);
    }

    #[test]
    fn search_treats_its_own_operators_as_ordinary_text() {
        let (mut store, _room) = store();
        store.remember(text("this AND that")).unwrap();
        assert_eq!(store.search("\"quote").unwrap().len(), 0);
        assert!(!store.search("AND").unwrap().is_empty());
    }

    #[test]
    fn a_deleted_entry_leaves_the_search_index() {
        let (mut store, _room) = store();
        let id = store.remember(text("findable")).unwrap();
        store.forget(id).unwrap();
        assert!(store.search("findable").unwrap().is_empty());
    }

    #[test]
    fn a_file_list_is_the_same_copy_only_in_the_same_order() {
        let (mut store, _room) = store();
        let listed = |paths: Vec<&str>| Captured {
            kind: Kind::Files,
            text: String::new(),
            file_paths: paths.into_iter().map(PathBuf::from).collect(),
            image: None,
        };
        store.remember(listed(vec!["/a", "/b"])).unwrap();
        store.remember(listed(vec!["/b", "/a"])).unwrap();
        assert_eq!(store.counts().unwrap(), (0, 2));

        store.remember(listed(vec!["/a", "/b"])).unwrap();
        assert_eq!(store.counts().unwrap(), (0, 2));
    }

    #[test]
    fn an_image_with_no_digest_is_never_the_same_copy_as_another() {
        let (mut store, _room) = store();
        let unnamed = || Captured {
            kind: Kind::Image,
            text: String::new(),
            file_paths: Vec::new(),
            image: None,
        };
        store.remember(unnamed()).unwrap();
        store.remember(unnamed()).unwrap();
        assert_eq!(store.counts().unwrap(), (0, 2));
    }

    #[test]
    fn an_edited_entry_is_findable_by_its_new_words() {
        let (mut store, _room) = store();
        let id = store.remember(text("before")).unwrap();
        assert!(store.rewrite(id, "after").unwrap());

        assert!(store.search("before").unwrap().is_empty());
        assert_eq!(store.search("after").unwrap().len(), 1);
        assert_eq!(store.entry(id).unwrap().unwrap().text, "after");
    }

    #[test]
    fn only_text_can_be_edited() {
        let (mut store, _room) = store();
        let id = store
            .remember(Captured {
                kind: Kind::Files,
                text: String::new(),
                file_paths: vec![PathBuf::from("/tmp/one")],
                image: None,
            })
            .unwrap();
        assert!(!store.rewrite(id, "not allowed").unwrap());
    }

    #[test]
    fn moving_an_entry_stays_inside_its_own_group() {
        let (mut store, _room) = store();
        let pinned = store.remember(text("pinned")).unwrap();
        store.set_pinned(pinned, true).unwrap();
        let older = store.remember(text("older")).unwrap();
        store.remember(text("newer")).unwrap();

        assert_eq!(
            store.summaries().unwrap().into_iter().map(|row| row.preview).collect::<Vec<_>>(),
            vec!["pinned", "newer", "older"]
        );

        assert!(!store.move_entry(pinned, true).unwrap(), "nothing above it in its own group");
        assert!(store.move_entry(older, true).unwrap());

        assert_eq!(
            store.summaries().unwrap().into_iter().map(|row| row.preview).collect::<Vec<_>>(),
            vec!["pinned", "older", "newer"]
        );
    }
}
