//! What a saved copy is.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const MAX_CHARACTERS: usize = 1_000_000;

pub const PREVIEW_CHARACTERS: usize = 2_000;

/// Beyond this a copy is a folder operation, not an entry.
pub const MAX_FILES: usize = 100;

pub const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

/// Zero means no limit.
pub const ALLOWED_LIMITS: [u32; 8] = [20, 50, 100, 250, 500, 1_000, 10_000, 0];

pub const DEFAULT_LIMIT: u32 = 50;

/// Falls back to the default rather than the nearest neighbour: an unlisted value is a
/// hand-edited file, not a choice.
pub fn sanitized_limit(limit: u32) -> u32 {
    if ALLOWED_LIMITS.contains(&limit) { limit } else { DEFAULT_LIMIT }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Text,
    Image,
    Files,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Files => "files",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "text" => Some(Self::Text),
            "image" => Some(Self::Image),
            "files" => Some(Self::Files),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StoredImage {
    pub digest: String,
    pub width: u32,
    pub height: u32,
}

impl StoredImage {
    pub fn dimensions(&self) -> String {
        format!("{}\u{d7}{}", self.width, self.height)
    }

    pub fn file_name(&self) -> String {
        format!("{}.png", self.digest)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    pub id: i64,
    pub kind: Kind,
    /// Empty for images and file lists, whose display text is derived so it cannot go stale.
    pub text: String,
    pub file_paths: Vec<PathBuf>,
    pub image: Option<StoredImage>,
    pub copied_at: SystemTime,
    pub pinned_at: Option<SystemTime>,
}

impl Entry {
    pub fn is_pinned(&self) -> bool {
        self.pinned_at.is_some()
    }

    pub fn file_names(&self) -> Vec<String> {
        self.file_paths
            .iter()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect()
    }

    pub fn preview(&self) -> String {
        match self.kind {
            Kind::Text => shorten(&self.text),
            Kind::Image => self.image.as_ref().map(StoredImage::dimensions).unwrap_or_default(),
            Kind::Files => self.file_names().join(", "),
        }
    }

    /// An image is indexed by the word and its size, having no text of its own.
    pub fn searchable_text(&self) -> String {
        match self.kind {
            Kind::Text => self.text.clone(),
            Kind::Image => {
                let size = self.image.as_ref().map(StoredImage::dimensions).unwrap_or_default();
                format!("image png {size}")
            }
            Kind::Files => {
                let names = self.file_names().join(" ");
                if self.file_paths.iter().any(|path| looks_like_an_image(path)) {
                    format!("image {names}")
                } else {
                    names
                }
            }
        }
    }
}

fn shorten(text: &str) -> String {
    let flattened: String = text
        .chars()
        .take(PREVIEW_CHARACTERS)
        .map(|character| if character == '\n' || character == '\t' { ' ' } else { character })
        .collect();
    let trimmed = flattened.trim().to_string();
    if text.chars().count() > PREVIEW_CHARACTERS { trimmed + "\u{2026}" } else { trimmed }
}

const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

pub fn looks_like_an_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn storable_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_CHARACTERS {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn text_entry(text: &str) -> Entry {
        Entry {
            id: 1,
            kind: Kind::Text,
            text: text.into(),
            file_paths: Vec::new(),
            image: None,
            copied_at: UNIX_EPOCH,
            pinned_at: None,
        }
    }

    #[test]
    fn a_limit_nobody_offers_falls_back_to_the_default() {
        assert_eq!(sanitized_limit(250), 250);
        assert_eq!(sanitized_limit(0), 0);
        assert_eq!(sanitized_limit(37), DEFAULT_LIMIT);
    }

    #[test]
    fn a_preview_is_one_line() {
        assert_eq!(text_entry("two\nlines\there").preview(), "two lines here");
    }

    #[test]
    fn a_long_preview_says_it_was_cut() {
        let long = "x".repeat(PREVIEW_CHARACTERS + 1);
        let preview = text_entry(&long).preview();
        assert_eq!(preview.chars().count(), PREVIEW_CHARACTERS + 1);
        assert!(preview.ends_with('\u{2026}'));
    }

    #[test]
    fn blank_and_oversized_text_is_not_worth_saving() {
        assert_eq!(storable_text("  hello  ").as_deref(), Some("hello"));
        assert_eq!(storable_text("   \n "), None);
        assert_eq!(storable_text(&"x".repeat(MAX_CHARACTERS + 1)), None);
    }

    #[test]
    fn an_image_file_makes_a_file_list_findable_by_the_word() {
        let mut entry = text_entry("");
        entry.kind = Kind::Files;
        entry.file_paths = vec![PathBuf::from("/tmp/holiday.JPEG")];
        assert_eq!(entry.searchable_text(), "image holiday.JPEG");
    }
}
