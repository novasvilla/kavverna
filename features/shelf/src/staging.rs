//! What a drop turns into. `classify` is pure and decides; `stage_blob` is the only write.
//! The acceptance order was verified against real drags: a uri list wins, raw image bytes
//! next (Firefox offers them, Chromium never does), then Mozilla's url flavour, then plain
//! text. A web address is always a link and never a download.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Everything the interface pulled out of one drop, already flattened to plain data.
#[derive(Debug, Default, Clone)]
pub struct Dropped {
    /// Stringified `drop.urls`, percent-encoded as the wire carries them.
    pub urls: Vec<String>,
    pub text: String,
    /// `text/x-moz-url` as raw bytes: UTF-16LE, url on the first line, label on the second.
    pub moz_url: Vec<u8>,
    pub image_bytes: Vec<u8>,
    pub image_format: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// A file already on this machine: its path is kept, nothing is copied.
    LocalFile(PathBuf),
    Link {
        url: String,
        label: Option<String>,
    },
    /// Raw bytes that must become a staged file to be draggable later.
    ImageBytes {
        extension: &'static str,
    },
    Text(String),
}

/// The one raw-bytes format worth pulling from the offer, preferring the lossless one.
pub fn wanted_image_format(formats: &[String]) -> Option<&'static str> {
    ["image/png", "image/jpeg", "image/gif"]
        .into_iter()
        .find(|wanted| formats.iter().any(|offered| offered == wanted))
}

pub fn extension_of(format: &str) -> &'static str {
    match format {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        _ => "png",
    }
}

pub fn classify(dropped: &Dropped) -> Vec<Incoming> {
    if !dropped.urls.is_empty() {
        return dropped
            .urls
            .iter()
            .filter_map(|url| {
                if let Some(path) = url.strip_prefix("file://") {
                    Some(Incoming::LocalFile(PathBuf::from(percent_decoded(path))))
                } else if is_web(url) {
                    Some(Incoming::Link { url: url.clone(), label: moz_label(dropped, url) })
                } else {
                    None
                }
            })
            .collect();
    }

    if !dropped.image_bytes.is_empty() {
        return vec![Incoming::ImageBytes { extension: extension_of(&dropped.image_format) }];
    }

    if let Some((url, label)) = decode_moz_url(&dropped.moz_url) {
        return vec![Incoming::Link { url, label }];
    }

    let text = dropped.text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if is_web(text) && !text.contains(char::is_whitespace) {
        return vec![Incoming::Link { url: text.to_owned(), label: None }];
    }
    vec![Incoming::Text(text.to_owned())]
}

fn is_web(candidate: &str) -> bool {
    candidate.starts_with("https://") || candidate.starts_with("http://")
}

/// The moz flavour carries the page title beside the url; a uri-list drop of the same url
/// gets to keep it.
fn moz_label(dropped: &Dropped, url: &str) -> Option<String> {
    let (moz, label) = decode_moz_url(&dropped.moz_url)?;
    (moz == url).then_some(label).flatten()
}

/// UTF-16LE, decoded by hand rather than trusted to a string conversion that would garble it.
pub fn decode_moz_url(bytes: &[u8]) -> Option<(String, Option<String>)> {
    if bytes.len() < 2 {
        return None;
    }
    let units: Vec<u16> =
        bytes.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
    let text = String::from_utf16_lossy(&units);

    let mut lines = text.lines().map(str::trim);
    let url = lines.next()?.trim_start_matches('\u{feff}').to_owned();
    if !is_web(&url) {
        return None;
    }
    let label = lines.next().filter(|line| !line.is_empty()).map(str::to_owned);
    Some((url, label))
}

/// A drop's uri list arrives percent-encoded; the filesystem wants bytes back.
pub fn percent_decoded(text: &str) -> String {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next().and_then(hex);
            let low = bytes.next().and_then(hex);
            match high.zip(low) {
                Some((high, low)) => out.push(high * 16 + low),
                None => out.push(byte),
            }
        } else {
            out.push(byte);
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|digit| digit as u8)
}

/// Writes one payload into the items directory, named by its digest so the same bytes never
/// exist twice, readable by nobody else.
pub fn stage_blob(items_dir: &Path, bytes: &[u8], extension: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(items_dir)?;
    let path = items_dir.join(format!("{}.{extension}", blake3::hash(bytes).to_hex()));
    if path.exists() {
        return Ok(path);
    }
    let mut file = std::fs::File::create(&path)?;
    file.write_all(bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn a_file_url_drop_becomes_a_local_file_and_spaces_survive() {
        let dropped = Dropped {
            urls: vec!["file:///home/someone/a%20photo.png".into()],
            ..Default::default()
        };

        assert_eq!(
            classify(&dropped),
            vec![Incoming::LocalFile(PathBuf::from("/home/someone/a photo.png"))]
        );
    }

    #[test]
    fn a_web_address_becomes_a_link_not_a_download() {
        let dropped =
            Dropped { urls: vec!["https://example.org/image.png".into()], ..Default::default() };

        assert_eq!(
            classify(&dropped),
            vec![Incoming::Link { url: "https://example.org/image.png".into(), label: None }]
        );
    }

    #[test]
    fn a_uri_list_wins_over_raw_image_bytes() {
        let dropped = Dropped {
            urls: vec!["file:///tmp/x.png".into()],
            image_bytes: vec![1, 2, 3],
            image_format: "image/png".into(),
            ..Default::default()
        };

        assert_eq!(classify(&dropped), vec![Incoming::LocalFile(PathBuf::from("/tmp/x.png"))]);
    }

    #[test]
    fn image_bytes_with_no_uri_are_staged() {
        let dropped = Dropped {
            image_bytes: vec![1, 2, 3],
            image_format: "image/jpeg".into(),
            ..Default::default()
        };

        assert_eq!(classify(&dropped), vec![Incoming::ImageBytes { extension: "jpg" }]);
    }

    #[test]
    fn moz_url_utf16_is_decoded_and_its_second_line_is_the_label() {
        let dropped = Dropped {
            moz_url: utf16le("https://example.org/page\nThe page title"),
            ..Default::default()
        };

        assert_eq!(
            classify(&dropped),
            vec![Incoming::Link {
                url: "https://example.org/page".into(),
                label: Some("The page title".into()),
            }]
        );
    }

    #[test]
    fn plain_text_stays_text_and_a_bare_address_becomes_a_link() {
        let text = Dropped { text: "two words".into(), ..Default::default() };
        assert_eq!(classify(&text), vec![Incoming::Text("two words".into())]);

        let address = Dropped { text: " https://example.org \n".into(), ..Default::default() };
        assert_eq!(
            classify(&address),
            vec![Incoming::Link { url: "https://example.org".into(), label: None }]
        );
    }

    #[test]
    fn an_empty_drop_stages_nothing() {
        assert!(classify(&Dropped::default()).is_empty());
    }

    #[test]
    fn wanted_image_format_prefers_png() {
        let offered = vec!["text/html".to_owned(), "image/jpeg".to_owned(), "image/png".to_owned()];
        assert_eq!(wanted_image_format(&offered), Some("image/png"));
        assert_eq!(wanted_image_format(&["text/plain".to_owned()]), None);
    }

    #[test]
    fn the_same_bytes_stage_to_the_same_file_once() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let first = stage_blob(dir.path(), b"payload", "txt").expect("staged");
        let second = stage_blob(dir.path(), b"payload", "txt").expect("staged again");

        assert_eq!(first, second);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
