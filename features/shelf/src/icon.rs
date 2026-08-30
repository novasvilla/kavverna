//! The freedesktop icon a shelved thing is drawn with when it has no thumbnail.

use crate::item::{Item, ItemKind};
use std::path::Path;

pub fn icon_for(item: &Item) -> &'static str {
    match &item.kind {
        ItemKind::Link { .. } => "internet-web-browser",
        ItemKind::Text { .. } => "text-x-generic",
        ItemKind::File { path, .. } => {
            if path.is_dir() {
                return "folder";
            }
            by_extension(path)
        }
    }
}

fn by_extension(path: &Path) -> &'static str {
    let extension =
        path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "avif" => "image-x-generic",
        "mp4" | "mkv" | "webm" | "avi" | "mov" => "video-x-generic",
        "mp3" | "flac" | "ogg" | "opus" | "wav" | "m4a" => "audio-x-generic",
        "pdf" => "application-pdf",
        "zip" | "tar" | "gz" | "xz" | "zst" | "7z" | "rar" => "application-x-archive",
        "txt" | "md" | "rst" | "log" => "text-x-generic",
        _ => "application-x-generic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extension_maps_to_its_freedesktop_icon() {
        let photo = Item {
            id: 1,
            kind: ItemKind::File { path: "/tmp/x.JPG".into(), staged: false },
            bytes: None,
            image: None,
        };
        assert_eq!(icon_for(&photo), "image-x-generic");

        let mystery = Item {
            id: 2,
            kind: ItemKind::File { path: "/tmp/x.xyz".into(), staged: false },
            bytes: None,
            image: None,
        };
        assert_eq!(icon_for(&mystery), "application-x-generic");
    }
}
