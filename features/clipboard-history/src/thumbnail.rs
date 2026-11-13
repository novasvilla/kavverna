//! Small copies of saved pictures, so a list of screenshots costs kilobytes to draw.

use std::io::Cursor;

/// The longest side of a thumbnail. Twice what the panel draws, so it stays sharp at a
/// scale factor of two.
pub const EDGE: u32 = 256;

/// A picture this small is its own thumbnail; a copy would only cost disk.
pub fn needed(width: u32, height: u32) -> bool {
    width > EDGE || height > EDGE
}

/// The size a picture is drawn at in the list, keeping its proportions. Worked out from the
/// stored dimensions so the shell can lay a row out before anything is decoded.
pub fn size(width: u32, height: u32) -> (u32, u32) {
    if !needed(width, height) {
        return (width, height);
    }
    let longest = width.max(height) as u64;
    let scaled = |side: u32| ((side as u64 * EDGE as u64).div_ceil(longest) as u32).max(1);
    (scaled(width), scaled(height))
}

/// Decodes the picture once and encodes the downscaled copy as PNG. `None` for anything that
/// does not decode, which leaves the entry without a thumbnail rather than without its picture.
pub fn downscale(png: &[u8]) -> Option<Vec<u8>> {
    let picture = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let (width, height) = size(picture.width(), picture.height());
    let small = picture.thumbnail_exact(width, height);
    let mut bytes = Vec::new();
    small.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png).ok()?;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::from_pixel(width, height, image::Rgba([9, 8, 7, 255]))
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("a picture we just made should encode");
        bytes
    }

    #[test]
    fn a_small_picture_needs_no_copy() {
        assert!(!needed(EDGE, 10));
        assert_eq!(size(40, 30), (40, 30));
    }

    #[test]
    fn a_wide_picture_keeps_its_proportions() {
        assert_eq!(size(2560, 1440), (EDGE, 144));
        assert_eq!(size(10, 5000), (1, EDGE));
    }

    #[test]
    fn the_copy_is_the_size_it_promised() {
        let small = downscale(&png(1024, 512)).expect("a real picture shrinks");
        let decoded = image::load_from_memory(&small).unwrap();
        assert_eq!((decoded.width(), decoded.height()), size(1024, 512));
    }

    #[test]
    fn something_that_is_not_a_picture_has_no_thumbnail() {
        assert!(downscale(b"not a png").is_none());
    }
}
