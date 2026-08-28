use ksni::Icon;

/// Drawn rather than named because `caffeine-cup-*` only ships with some icon themes, and
/// a name the host cannot resolve renders as an invisible tray entry.
const CUP: [&str; SIZE as usize] = [
    "......................",
    "......................",
    "..##################..",
    "..#****************#..",
    "..#****************#..",
    "..#****************####",
    "..#****************#..#",
    "..#****************#..#",
    "..#****************#..#",
    "..#****************####",
    "..#****************#..",
    "..#****************#..",
    "..#****************#..",
    "..#****************#..",
    "...#**************#...",
    "....#************#....",
    ".....############.....",
    "......................",
    "..##################..",
    "..##################..",
    "......................",
    "......................",
];

const SIZE: i32 = 22;

const STROKE: [u8; 4] = [0xFF, 0xDC, 0xDC, 0xDC];
const FILL: [u8; 4] = [0xFF, 0x3D, 0xAE, 0xE9];
const CLEAR: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

fn draw(filled: bool, scale: i32) -> Icon {
    let edge = SIZE * scale;
    let mut data = Vec::with_capacity((edge * edge * 4) as usize);

    for y in 0..edge {
        let row = CUP[(y / scale) as usize].as_bytes();
        for x in 0..edge {
            let pixel = match row[(x / scale) as usize] {
                b'#' => STROKE,
                b'*' if filled => FILL,
                _ => CLEAR,
            };
            data.extend_from_slice(&pixel);
        }
    }

    Icon { width: edge, height: edge, data }
}

/// Several sizes so the panel can pick one without resampling on a HiDPI screen.
pub fn cup(filled: bool) -> Vec<Icon> {
    [1, 2].into_iter().map(|scale| draw(filled, scale)).collect()
}
