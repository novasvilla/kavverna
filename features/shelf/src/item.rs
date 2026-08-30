use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The shelf holds things someone deliberately parked, so it refuses new drops when full
/// rather than silently evicting what was placed first.
pub const MOST_ITEMS: usize = 100;
pub const MOST_STAGED_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    /// A file somewhere on this machine. `staged` marks the ones the shelf itself wrote,
    /// which are the only ones removal may delete.
    File { path: PathBuf, staged: bool },
    /// Dropped text, staged as a file so it can be dragged out again; the preview is what
    /// the shelf shows.
    Text { staged: PathBuf, preview: String },
    /// A web address. Never fetched: turning a link into a file would need network code,
    /// and Kavverna promises to have none.
    Link { url: String, label: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: u64,
    pub kind: ItemKind,
    /// Size on disk when the item has one.
    pub bytes: Option<u64>,
    /// Width and height when the file decodes as an image, read from the header only.
    pub image: Option<(u32, u32)>,
}

impl Item {
    /// The file behind the item, when there is one.
    pub fn lives_at(&self) -> Option<&Path> {
        match &self.kind {
            ItemKind::File { path, .. } => Some(path),
            ItemKind::Text { staged, .. } => Some(staged),
            ItemKind::Link { .. } => None,
        }
    }

    /// A link cannot die; a file item dies with its file, and a dead one is dimmed and
    /// left out of drags rather than offered as a ghost.
    pub fn alive(&self) -> bool {
        self.lives_at().is_none_or(Path::exists)
    }

    /// The blob to delete when the item is removed on purpose: only what the shelf wrote.
    pub fn owned_blob(&self) -> Option<&Path> {
        match &self.kind {
            ItemKind::File { path, staged: true } => Some(path),
            ItemKind::Text { staged, .. } => Some(staged),
            _ => None,
        }
    }

    pub fn name(&self) -> String {
        match &self.kind {
            ItemKind::File { path, .. } => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            ItemKind::Text { preview, .. } => preview.clone(),
            ItemKind::Link { url, label } => {
                label.clone().filter(|label| !label.is_empty()).unwrap_or_else(|| url.clone())
            }
        }
    }

    /// The quiet second line: size and dimensions for files, the host for links.
    pub fn detail(&self) -> String {
        match &self.kind {
            ItemKind::File { .. } | ItemKind::Text { .. } => {
                let size = self.bytes.map(size_label).unwrap_or_default();
                match self.image {
                    Some((w, h)) if size.is_empty() => format!("{w}x{h}"),
                    Some((w, h)) => format!("{size}  {w}x{h}"),
                    None => size,
                }
            }
            ItemKind::Link { url, .. } => host_of(url).unwrap_or_default().to_owned(),
        }
    }
}

fn host_of(url: &str) -> Option<&str> {
    let rest = url.split_once("//").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next()?;
    if host.is_empty() { None } else { Some(host) }
}

pub fn size_label(bytes: u64) -> String {
    match bytes {
        0..=1023 => format!("{bytes} B"),
        1024..=1048575 => format!("{:.0} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1048576.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_names_itself_by_label_and_details_its_host() {
        let link = Item {
            id: 1,
            kind: ItemKind::Link {
                url: "https://example.org/a/page?utm=1".into(),
                label: Some("A page".into()),
            },
            bytes: None,
            image: None,
        };

        assert_eq!(link.name(), "A page");
        assert_eq!(link.detail(), "example.org");
        assert!(link.alive());
    }

    #[test]
    fn sizes_read_like_a_person_wrote_them() {
        assert_eq!(size_label(900), "900 B");
        assert_eq!(size_label(200 * 1024), "200 KB");
        assert_eq!(size_label(3 * 1024 * 1024 + 200 * 1024), "3.2 MB");
    }

    #[test]
    fn only_staged_blobs_are_owned() {
        let borrowed = Item {
            id: 1,
            kind: ItemKind::File { path: "/home/someone/photo.png".into(), staged: false },
            bytes: None,
            image: None,
        };
        let staged = Item {
            id: 2,
            kind: ItemKind::File { path: "/data/items/abc.png".into(), staged: true },
            bytes: None,
            image: None,
        };

        assert_eq!(borrowed.owned_blob(), None);
        assert_eq!(
            staged.owned_blob().map(|p| p.display().to_string()).as_deref(),
            Some("/data/items/abc.png")
        );
    }
}
