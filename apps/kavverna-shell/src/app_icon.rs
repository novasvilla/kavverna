use ksni::Icon;

/// Drawn rather than named because a theme icon the host cannot resolve renders as an
/// invisible tray entry, and Kavverna ships no icon theme of its own.
const GLYPH: [&str; SIZE as usize] = [
    "......................",
    "......................",
    "......................",
    "....##........##......",
    "....##.......##.......",
    "....##......##........",
    "....##.....##.........",
    "....##....##..........",
    "....##...##...........",
    "....##..##............",
    "....#####.............",
    "....#####.............",
    "....##..##............",
    "....##...##...........",
    "....##....##..........",
    "....##.....##.........",
    "....##......##........",
    "....##.......##.......",
    "....##........##......",
    "......................",
    "......................",
    "......................",
];

const SIZE: i32 = 22;

const RESTING: [u8; 4] = [0xFF, 0xDC, 0xDC, 0xDC];
const AWAKE: [u8; 4] = [0xFF, 0xE9, 0xB4, 0x4C];
const CLEAR: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

fn draw(colour: [u8; 4], scale: i32) -> Icon {
    let edge = SIZE * scale;
    let mut data = Vec::with_capacity((edge * edge * 4) as usize);

    for y in 0..edge {
        let row = GLYPH[(y / scale) as usize].as_bytes();
        for x in 0..edge {
            let pixel = if row[(x / scale) as usize] == b'#' { colour } else { CLEAR };
            data.extend_from_slice(&pixel);
        }
    }

    Icon { width: edge, height: edge, data }
}

/// Several sizes so the panel can pick one without resampling on a HiDPI screen.
pub fn mark(awake: bool) -> Vec<Icon> {
    let colour = if awake { AWAKE } else { RESTING };
    [1, 2].into_iter().map(|scale| draw(colour, scale)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_spans_the_icon() {
        for (index, row) in GLYPH.iter().enumerate() {
            assert_eq!(row.len(), SIZE as usize, "row {index} is the wrong width");
        }
    }

    #[test]
    fn the_mark_carries_its_state_in_colour() {
        let resting = &mark(false)[0].data;
        let awake = &mark(true)[0].data;

        assert_eq!(resting.len(), awake.len());
        assert_ne!(resting, awake, "both states drew the same pixels");
    }
}
