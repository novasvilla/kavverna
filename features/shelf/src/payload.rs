//! What a drag out of the shelf offers. Dead payloads are filtered first: every destination
//! refuses a vanished file's uri, and one dead entry makes the whole drag read as broken.

use crate::item::{Item, ItemKind};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DragPayload {
    /// What goes into the drag's mime data. Files and links as a uri list, text joined as
    /// plain text; a link travels as both so a text field can take it too.
    Mime { uris: Vec<String>, text: String },
    /// Nothing in the selection survives on disk; the drag is refused with a reason instead
    /// of offering a ghost.
    AllDead { notice: String },
}

pub fn payload_for(items: &[&Item]) -> DragPayload {
    let living: Vec<&&Item> = items.iter().filter(|item| item.alive()).collect();
    if living.is_empty() {
        return DragPayload::AllDead {
            notice: "Nothing here still exists on disk, so there is nothing to drag.".into(),
        };
    }

    let mut uris = Vec::new();
    let mut texts = Vec::new();
    for item in living {
        match &item.kind {
            ItemKind::File { path, .. } => uris.push(file_uri(path)),
            ItemKind::Text { staged, .. } => {
                if let Ok(content) = std::fs::read_to_string(staged) {
                    texts.push(content);
                }
            }
            ItemKind::Link { url, .. } => {
                uris.push(url.clone());
                texts.push(url.clone());
            }
        }
    }

    DragPayload::Mime { uris, text: texts.join("\n") }
}

/// Percent-encodes only what a uri cannot carry raw; destinations decode the rest. Also how
/// the interface addresses a thumbnail, so an odd character in a filename cannot break it.
pub fn file_uri(path: &Path) -> String {
    let mut encoded = String::from("file://");
    for byte in path.display().to_string().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.' | b'-' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file(id: u64, path: &str) -> Item {
        Item {
            id,
            kind: ItemKind::File { path: PathBuf::from(path), staged: false },
            bytes: None,
            image: None,
        }
    }

    fn link(id: u64, url: &str) -> Item {
        Item { id, kind: ItemKind::Link { url: url.into(), label: None }, bytes: None, image: None }
    }

    #[test]
    fn files_drag_out_as_a_uri_list_with_spaces_encoded() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let path = dir.path().join("a photo.png");
        std::fs::write(&path, b"x").expect("written");

        let item = file(1, &path.display().to_string());
        let DragPayload::Mime { uris, .. } = payload_for(&[&item]) else {
            panic!("a living file must drag");
        };

        assert_eq!(uris.len(), 1);
        assert!(uris[0].starts_with("file:///"));
        assert!(uris[0].ends_with("a%20photo.png"));
    }

    #[test]
    fn a_vanished_file_is_left_out_of_the_drag() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let here = dir.path().join("here.txt");
        std::fs::write(&here, b"x").expect("written");

        let living = file(1, &here.display().to_string());
        let gone = file(2, "/nowhere/gone.txt");
        let DragPayload::Mime { uris, .. } = payload_for(&[&living, &gone]) else {
            panic!("one living file is enough to drag");
        };

        assert_eq!(uris.len(), 1);
    }

    #[test]
    fn an_all_dead_selection_offers_no_drag_and_explains() {
        let gone = file(1, "/nowhere/gone.txt");
        assert!(matches!(payload_for(&[&gone]), DragPayload::AllDead { .. }));
    }

    #[test]
    fn a_link_drags_out_as_both_uri_and_text() {
        let item = link(1, "https://example.org");
        let DragPayload::Mime { uris, text } = payload_for(&[&item]) else {
            panic!("a link always drags");
        };

        assert_eq!(uris, vec!["https://example.org".to_owned()]);
        assert_eq!(text, "https://example.org");
    }

    #[test]
    fn text_keeps_its_content_through_staging_and_out_again() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let staged = crate::staging::stage_blob(dir.path(), "hello there".as_bytes(), "txt")
            .expect("staged");
        let item = Item {
            id: 1,
            kind: ItemKind::Text { staged, preview: "hello there".into() },
            bytes: Some(11),
            image: None,
        };

        let DragPayload::Mime { text, .. } = payload_for(&[&item]) else {
            panic!("staged text must drag");
        };
        assert_eq!(text, "hello there");
    }
}
