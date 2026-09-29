//! A place to put things down mid-task: files, text and links dropped onto it stay until
//! they are dragged somewhere else. Dropping stages, never copies: a local file is kept by
//! its path, and only content with no file behind it, raw image bytes or text, is written
//! into the shelf's own directory. Web addresses stay links, never downloads.

mod icon;
mod item;
mod payload;
mod pile;
mod staging;
mod store;

pub use icon::icon_for;
pub use item::{Item, ItemKind, MOST_ITEMS, MOST_STAGED_BYTES, size_label};
pub use payload::{DragPayload, file_uri};
pub use pile::Pile;
pub use staging::{Dropped, wanted_image_format};

use std::path::PathBuf;

const MOST_LINK_BYTES: usize = 8 * 1024;

pub struct Shelf {
    dir: PathBuf,
    piles: Vec<Pile>,
    next_id: u64,
    read_only: bool,
    /// The last refusal, one sentence, cleared by the next accepted drop.
    pub notice: String,
}

impl Shelf {
    /// The usual home under the user's data directory.
    pub fn usual_dir() -> Option<PathBuf> {
        directories_dir()
    }

    pub fn open(dir: PathBuf, keep_across_restarts: bool) -> Self {
        let (piles, read_only, notice) = match store::load(&dir) {
            Ok(mut piles) => {
                if keep_across_restarts {
                    store::sweep(&dir, &piles);
                } else {
                    store::forget(&dir);
                    piles.clear();
                }
                (piles, false, String::new())
            }
            Err(err) => {
                tracing::error!(%err, "the saved shelf could not be read");
                (
                    Vec::new(),
                    true,
                    "The saved shelf could not be read. Its files were left untouched.".into(),
                )
            }
        };
        let next_id = piles
            .iter()
            .flat_map(|pile| pile.items.iter().map(|item| item.id).chain([pile.id]))
            .max()
            .map_or(1, |seen| seen + 1);
        Self { dir, piles, next_id, read_only, notice }
    }

    pub fn piles(&self) -> &[Pile] {
        &self.piles
    }

    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.piles.iter().flat_map(|pile| pile.items.iter())
    }

    pub fn item(&self, id: u64) -> Option<&Item> {
        self.items().find(|item| item.id == id)
    }

    pub fn len(&self) -> usize {
        self.items().count()
    }

    pub fn is_empty(&self) -> bool {
        self.piles.is_empty()
    }

    fn staged_bytes(&self) -> u64 {
        self.items().filter(|item| item.owned_blob().is_some()).filter_map(|item| item.bytes).sum()
    }

    /// One drop gesture in: everything it carried becomes one pile, or nothing does. A full
    /// shelf refuses and says so, because what is here was parked on purpose.
    pub fn deposit(&mut self, dropped: &staging::Dropped) -> bool {
        if self.read_only {
            return false;
        }
        let incoming = staging::classify(dropped);
        if incoming.is_empty() {
            self.notice = "Nothing in that drop could be kept.".into();
            return false;
        }
        if self.len() + incoming.len() > MOST_ITEMS {
            self.notice = format!("The shelf is full at {MOST_ITEMS} items.");
            return false;
        }
        if incoming.iter().any(|arrival| match arrival {
            staging::Incoming::Link { url, label } => {
                url.len().saturating_add(label.as_ref().map_or(0, String::len)) > MOST_LINK_BYTES
            }
            _ => false,
        }) {
            self.notice = "That link is too large for the shelf.".into();
            return false;
        }
        let incoming_bytes: u64 = incoming
            .iter()
            .map(|arrival| match arrival {
                staging::Incoming::ImageBytes { .. } => dropped.image_bytes.len() as u64,
                staging::Incoming::Text(text) => text.len() as u64,
                _ => 0,
            })
            .sum();
        if self.staged_bytes().saturating_add(incoming_bytes) > MOST_STAGED_BYTES {
            self.notice = "The shelf's staging space is full.".into();
            return false;
        }

        let items_dir = store::items_dir(&self.dir);
        let mut items = Vec::new();
        let mut next_id = self.next_id;
        for arrival in incoming {
            let id = next_id;
            next_id += 1;
            let item = match arrival {
                staging::Incoming::LocalFile(path) => {
                    let bytes = std::fs::metadata(&path).ok().map(|meta| meta.len());
                    let image = image_size(&path);
                    Item { id, kind: ItemKind::File { path, staged: false }, bytes, image }
                }
                staging::Incoming::Link { url, label } => {
                    Item { id, kind: ItemKind::Link { url, label }, bytes: None, image: None }
                }
                staging::Incoming::ImageBytes { extension } => {
                    match staging::stage_blob(&items_dir, &dropped.image_bytes, extension) {
                        Ok(path) => Item {
                            id,
                            image: image_size(&path),
                            bytes: Some(dropped.image_bytes.len() as u64),
                            kind: ItemKind::File { path, staged: true },
                        },
                        Err(err) => {
                            tracing::warn!(%err, "image bytes could not be staged");
                            self.notice = "The shelf could not stage this drop.".into();
                            return false;
                        }
                    }
                }
                staging::Incoming::Text(text) => {
                    match staging::stage_blob(&items_dir, text.as_bytes(), "txt") {
                        Ok(staged) => Item {
                            id,
                            bytes: Some(text.len() as u64),
                            image: None,
                            kind: ItemKind::Text { staged, preview: first_line(&text) },
                        },
                        Err(err) => {
                            tracing::warn!(%err, "text could not be staged");
                            self.notice = "The shelf could not stage this drop.".into();
                            return false;
                        }
                    }
                }
            };
            items.push(item);
        }

        if items.is_empty() {
            self.notice = "Nothing in that drop could be kept.".into();
            return false;
        }

        let pile = Pile { id: next_id, items };
        let mut piles = self.piles.clone();
        piles.push(pile);
        if self.save_change(piles) {
            self.next_id = next_id + 1;
            true
        } else {
            false
        }
    }

    /// Removal on purpose deletes the blobs the shelf owns; nothing else is ever touched.
    pub fn remove(&mut self, id: u64) {
        if self.read_only {
            return;
        }
        let mut piles = self.piles.clone();
        let mut removed = None;
        for pile in &mut piles {
            if let Some(at) = pile.items.iter().position(|item| item.id == id) {
                removed = Some(pile.items.remove(at));
                break;
            }
        }
        let Some(item) = removed else { return };
        piles.retain(|pile| !pile.items.is_empty());
        let orphan = item
            .owned_blob()
            .filter(|blob| {
                !piles
                    .iter()
                    .flat_map(|pile| &pile.items)
                    .any(|remaining| remaining.owned_blob() == Some(*blob))
            })
            .map(std::path::Path::to_path_buf);
        if self.save_change(piles) {
            if let Some(blob) = orphan {
                let _ = std::fs::remove_file(blob);
            }
        }
    }

    pub fn clear(&mut self) {
        if self.read_only {
            return;
        }
        let blobs: Vec<PathBuf> =
            self.items().filter_map(Item::owned_blob).map(std::path::Path::to_path_buf).collect();
        if self.save_change(Vec::new()) {
            for blob in blobs {
                let _ = std::fs::remove_file(blob);
            }
        }
    }

    /// After a drop somewhere accepted the drag. Items leave; their blobs stay for the next
    /// start's sweep, because the receiving side copies the file after the drag reports done.
    pub fn taken(&mut self, ids: &[u64]) {
        if self.read_only {
            return;
        }
        let mut piles = self.piles.clone();
        for pile in &mut piles {
            pile.items.retain(|item| !ids.contains(&item.id));
        }
        piles.retain(|pile| !pile.items.is_empty());
        self.save_change(piles);
    }

    pub fn payload_for(&self, ids: &[u64]) -> DragPayload {
        let chosen: Vec<&Item> = self.items().filter(|item| ids.contains(&item.id)).collect();
        payload::payload_for(&chosen)
    }

    fn save_change(&mut self, piles: Vec<Pile>) -> bool {
        match store::save(&self.dir, &piles) {
            Ok(()) => {
                self.piles = piles;
                self.notice.clear();
                true
            }
            Err(err) => {
                tracing::error!(%err, "the shelf did not save");
                self.notice = "The shelf could not save this change.".into();
                false
            }
        }
    }
}

fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    let mut preview: String = line.chars().take(80).collect();
    if preview.len() < line.len() || text.lines().count() > 1 {
        preview.push('…');
    }
    preview
}

/// Header only, the way the clipboard measures its images: dimensions without a decode.
fn image_size(path: &std::path::Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_dimensions().ok()
}

fn directories_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "", "kavverna").map(|dirs| dirs.data_dir().join("shelf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drop_of_text(text: &str) -> Dropped {
        Dropped { text: text.into(), ..Default::default() }
    }

    fn drop_of_file(path: &str) -> Dropped {
        Dropped { urls: vec![format!("file://{path}")], ..Default::default() }
    }

    #[test]
    fn several_items_in_one_gesture_form_one_pile() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        let one = dir.path().join("one.txt");
        let two = dir.path().join("two.txt");
        std::fs::write(&one, b"1").unwrap();
        std::fs::write(&two, b"2").unwrap();

        let dropped = Dropped {
            urls: vec![format!("file://{}", one.display()), format!("file://{}", two.display())],
            ..Default::default()
        };
        assert!(shelf.deposit(&dropped));

        assert_eq!(shelf.piles().len(), 1);
        assert_eq!(shelf.piles()[0].items.len(), 2);
    }

    #[test]
    fn a_full_shelf_refuses_the_drop_and_says_so() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        for n in 0..MOST_ITEMS {
            assert!(shelf.deposit(&drop_of_text(&format!("note {n}"))));
        }

        assert!(!shelf.deposit(&drop_of_text("one too many")));
        assert!(shelf.notice.contains("full"));
        assert_eq!(shelf.len(), MOST_ITEMS);
    }

    #[test]
    fn removing_a_staged_item_deletes_its_blob() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        shelf.deposit(&drop_of_text("staged words"));

        let staged = shelf.items().next().unwrap().owned_blob().unwrap().to_path_buf();
        let id = shelf.items().next().unwrap().id;
        assert!(staged.exists());

        shelf.remove(id);
        assert!(!staged.exists());
        assert!(shelf.is_empty());
    }

    #[test]
    fn removing_one_of_two_equal_notes_keeps_the_other_alive() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        assert!(shelf.deposit(&drop_of_text("same note")));
        assert!(shelf.deposit(&drop_of_text("same note")));

        let ids: Vec<_> = shelf.items().map(|item| item.id).collect();
        let staged = shelf.item(ids[0]).unwrap().owned_blob().unwrap().to_path_buf();
        assert_eq!(shelf.item(ids[1]).unwrap().owned_blob(), Some(staged.as_path()));

        shelf.remove(ids[0]);
        assert!(staged.exists());
        assert!(shelf.item(ids[1]).unwrap().alive());
        assert_eq!(
            shelf.payload_for(&[ids[1]]),
            DragPayload::Mime { uris: Vec::new(), text: "same note".into() }
        );

        shelf.remove(ids[1]);
        assert!(!staged.exists());
    }

    #[test]
    fn a_note_larger_than_staging_space_is_refused_before_writing() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        let note = "x".repeat(MOST_STAGED_BYTES as usize + 1);

        assert!(!shelf.deposit(&drop_of_text(&note)));
        assert!(shelf.notice.contains("full"));
        assert!(shelf.is_empty());
        assert!(!store::items_dir(dir.path()).exists());
    }

    #[test]
    fn an_oversized_link_is_refused_before_saving_json() {
        let dir = tempfile::tempdir().unwrap();
        let mut shelf = Shelf::open(dir.path().into(), true);
        let link = format!("https://example.org/?x={}", "a".repeat(MOST_STAGED_BYTES as usize));
        assert!(!shelf.deposit(&drop_of_text(&link)));
        assert!(shelf.notice.contains("too large"));
        assert!(!dir.path().join("shelf.json").exists());
    }

    #[test]
    fn an_unreadable_saved_list_never_sweeps_its_blobs() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let staged =
            staging::stage_blob(&store::items_dir(dir.path()), b"kept", "txt").expect("staged");
        std::fs::write(dir.path().join("shelf.json"), b"{").expect("broken list");

        let mut shelf = Shelf::open(dir.path().into(), true);
        assert!(staged.exists());
        assert!(!shelf.deposit(&drop_of_text("another note")));
        assert_eq!(std::fs::read(dir.path().join("shelf.json")).unwrap(), b"{");
    }

    #[test]
    fn a_newer_saved_list_is_left_for_a_newer_version() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let staged =
            staging::stage_blob(&store::items_dir(dir.path()), b"kept", "txt").expect("staged");
        let future = br#"{"version":999,"piles":[]}"#;
        std::fs::write(dir.path().join("shelf.json"), future).unwrap();

        let mut shelf = Shelf::open(dir.path().into(), false);
        assert!(!shelf.deposit(&drop_of_text("another note")));
        shelf.clear();
        assert!(staged.exists());
        assert_eq!(std::fs::read(dir.path().join("shelf.json")).unwrap(), future);
    }

    #[test]
    fn removing_a_local_file_item_never_touches_the_file() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let precious = dir.path().join("precious.txt");
        std::fs::write(&precious, b"kept").unwrap();

        let mut shelf = Shelf::open(dir.path().into(), true);
        shelf.deposit(&drop_of_file(&precious.display().to_string()));
        let id = shelf.items().next().unwrap().id;

        shelf.remove(id);
        assert!(precious.exists());
    }

    #[test]
    fn taken_items_leave_but_their_blobs_wait_for_the_sweep() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        shelf.deposit(&drop_of_text("dragged away"));
        let staged = shelf.items().next().unwrap().owned_blob().unwrap().to_path_buf();
        let id = shelf.items().next().unwrap().id;

        shelf.taken(&[id]);
        assert!(shelf.is_empty());
        assert!(staged.exists(), "the receiver copies after the drag reports done");

        let reopened = Shelf::open(dir.path().into(), true);
        assert!(reopened.is_empty());
        assert!(!staged.exists(), "the next start sweeps what nothing names");
    }

    #[test]
    fn what_was_shelved_is_back_after_a_restart() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        shelf.deposit(&drop_of_text("still here"));
        drop(shelf);

        let back = Shelf::open(dir.path().into(), true);
        assert_eq!(back.len(), 1);
        assert_eq!(back.items().next().unwrap().name(), "still here");
    }

    #[test]
    fn with_keeping_off_a_restart_starts_empty() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        shelf.deposit(&drop_of_text("gone tomorrow"));
        let staged = shelf.items().next().unwrap().owned_blob().unwrap().to_path_buf();
        drop(shelf);

        let fresh = Shelf::open(dir.path().into(), false);
        assert!(fresh.is_empty());
        assert!(!staged.exists());
    }

    #[test]
    fn ids_keep_growing_after_a_reload() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let mut shelf = Shelf::open(dir.path().into(), true);
        shelf.deposit(&drop_of_text("first"));
        let highest = shelf.items().map(|item| item.id).max().unwrap();
        drop(shelf);

        let mut back = Shelf::open(dir.path().into(), true);
        back.deposit(&drop_of_text("second"));
        let next = back.items().map(|item| item.id).max().unwrap();
        assert!(next > highest);
    }
}
