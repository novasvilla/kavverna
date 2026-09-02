//! Where the panel opens, as pure geometry. Everything here works on numbers the interface
//! reports, so every placement rule is testable without a compositor. The results are
//! layer-shell terms: which two edges the surface anchors to, and the margins from them, in
//! the screen's logical pixels. KWin arranges layer surfaces inside the area left by other
//! panels' exclusive zones, so a margin of `gap` from the bottom already clears the task bar.

/// The panel's fixed width. The interface reads it from here so the number exists once.
pub const WIDTH: i32 = 360;

/// The panel's height cap, restated from the interface's own formula. Only the vertical
/// clamp uses it, so a drift of a few pixels moves nothing visible.
pub fn tallest(screen_height: i32) -> i32 {
    720.min(screen_height - 24)
}

/// A vertical bar's icon sits at the click; the panel's header opens level with it rather
/// than centred, the way a menu opens at a cursor.
const HEADER_REACH: i32 = 24;

/// One connected screen, in global logical coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The two layer-shell anchors and the margins from them. A margin on an edge the surface is
/// not anchored to does nothing, so those stay zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub at_bottom: bool,
    pub at_right: bool,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// The corner the panel has always used. With the default gap this is the pre-placement
/// behaviour byte for byte.
pub fn corner(gap: i32) -> Placement {
    Placement { at_bottom: true, at_right: true, left: 0, top: 0, right: gap, bottom: gap }
}

pub fn screen_containing(point: (i32, i32), screens: &[Screen]) -> Option<&Screen> {
    screens.iter().find(|screen| {
        point.0 >= screen.x
            && point.0 < screen.x + screen.width
            && point.1 >= screen.y
            && point.1 < screen.y + screen.height
    })
}

/// Where the surface lands, in its screen's coordinates, since margins are screen-local.
/// `pressed` and `pointer` are both surface-local readings of the same drag: on Wayland a
/// window is never told where it sits, so a global pointer is only ever the surface's own
/// corner plus one of these.
pub fn dragged_local(
    origin: (i32, i32),
    pressed: (i32, i32),
    pointer: (i32, i32),
    screen: &Screen,
) -> (i32, i32) {
    (origin.0 + pointer.0 - pressed.0 - screen.x, origin.1 + pointer.1 - pressed.1 - screen.y)
}

fn held(value: i32, low: i32, high: i32) -> i32 {
    value.max(low).min(high.max(low))
}

/// Beside a tray icon clicked at `point`. The tray lives in a bar hugging a screen edge, so
/// the edge nearest the click is the bar's edge; the panel hangs off it, centred on the icon
/// along a horizontal bar and opening level with it down a vertical one, never past the
/// screen's sides.
pub fn beside_tray(point: (i32, i32), screen: &Screen, gap: i32) -> Placement {
    let lx = point.0 - screen.x;
    let ly = point.1 - screen.y;

    let to_bottom = screen.height - ly;
    let to_top = ly;
    let to_right = screen.width - lx;
    let to_left = lx;

    let across = held(lx - WIDTH / 2, gap, screen.width - WIDTH - gap);
    let down = held(ly - HEADER_REACH, gap, screen.height - tallest(screen.height) - gap);

    // Ties go to the bottom, then the top, because that is where bars live.
    let nearest = to_bottom.min(to_top).min(to_right).min(to_left);
    if nearest == to_bottom {
        Placement { at_bottom: true, at_right: false, left: across, top: 0, right: 0, bottom: gap }
    } else if nearest == to_top {
        Placement { at_bottom: false, at_right: false, left: across, top: gap, right: 0, bottom: 0 }
    } else if nearest == to_right {
        Placement { at_bottom: false, at_right: true, left: 0, top: down, right: gap, bottom: 0 }
    } else {
        Placement { at_bottom: false, at_right: false, left: gap, top: down, right: 0, bottom: 0 }
    }
}

/// A free position, anchored top left with the panel held fully on the screen.
pub fn pinned(position: (i32, i32), screen: &Screen, size: (i32, i32), gap: i32) -> Placement {
    Placement {
        at_bottom: false,
        at_right: false,
        left: held(position.0, gap, screen.width - size.0 - gap),
        top: held(position.1, gap, screen.height - size.1 - gap),
        right: 0,
        bottom: 0,
    }
}

/// The screen-local top left corner a placement puts the panel at, so a drag can start from
/// wherever the panel already is.
pub fn position_of(placement: &Placement, screen: &Screen, size: (i32, i32)) -> (i32, i32) {
    let x =
        if placement.at_right { screen.width - size.0 - placement.right } else { placement.left };
    let y =
        if placement.at_bottom { screen.height - size.1 - placement.bottom } else { placement.top };
    (x, y)
}

/// The interface reports the connected screens as one line each:
/// "name<TAB>x<TAB>y<TAB>width<TAB>height", in global logical coordinates. A malformed line
/// is dropped rather than guessed at.
pub fn screens_from_report(report: &str) -> Vec<Screen> {
    report
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let name = parts.next()?;
            let x = parts.next()?.parse().ok()?;
            let y = parts.next()?.parse().ok()?;
            let width: i32 = parts.next()?.parse().ok()?;
            let height: i32 = parts.next()?.parse().ok()?;
            if name.is_empty() || width <= 0 || height <= 0 {
                return None;
            }
            Some(Screen { name: name.to_owned(), x, y, width, height })
        })
        .collect()
}

/// One remembered point: "screen name<TAB>x<TAB>y". The same shape serves the tray anchor and
/// the per-screen positions, and a screen name cannot carry a tab.
pub fn entry(screen_name: &str, x: i32, y: i32) -> String {
    format!("{screen_name}\t{x}\t{y}")
}

pub fn parse(text: &str) -> Option<(String, i32, i32)> {
    let mut parts = text.splitn(3, '\t');
    let name = parts.next()?;
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    if name.is_empty() { None } else { Some((name.to_owned(), x, y)) }
}

/// The most recent remembered point whose screen is still connected. Entries for departed
/// screens stay in the list, because the monitor may come back, and place nothing meanwhile.
pub fn last_remembered<'a>(
    entries: &[String],
    screens: &'a [Screen],
) -> Option<((i32, i32), &'a Screen)> {
    entries.iter().rev().find_map(|line| {
        let (name, x, y) = parse(line)?;
        let screen = screens.iter().find(|screen| screen.name == name)?;
        Some(((x, y), screen))
    })
}

/// Replaces the screen's entry and moves it to the end, so recency is the list's order.
pub fn with_position(entries: Vec<String>, screen_name: &str, x: i32, y: i32) -> Vec<String> {
    let mut kept: Vec<String> = entries
        .into_iter()
        .filter(|line| parse(line).is_none_or(|(name, _, _)| name != screen_name))
        .collect();
    kept.push(entry(screen_name, x, y));
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primary() -> Screen {
        Screen { name: "DP-1".into(), x: 0, y: 0, width: 2560, height: 1440 }
    }

    fn second() -> Screen {
        Screen { name: "HDMI-1".into(), x: 2560, y: 0, width: 1920, height: 1080 }
    }

    #[test]
    fn a_click_on_a_bottom_bar_opens_above_it_centred_on_the_icon() {
        let place = beside_tray((2300, 1430), &primary(), 12);
        assert!(place.at_bottom && !place.at_right);
        assert_eq!(place.bottom, 12);
        assert_eq!(place.left, 2300 - WIDTH / 2);
    }

    #[test]
    fn a_click_on_a_left_bar_opens_beside_it_level_with_the_icon() {
        let place = beside_tray((8, 300), &primary(), 12);
        assert!(!place.at_bottom && !place.at_right);
        assert_eq!(place.left, 12);
        assert_eq!(place.top, 300 - 24);
    }

    #[test]
    fn an_icon_low_on_a_vertical_bar_keeps_the_tallest_page_on_screen() {
        // Top at 900 would run a 720 tall page off the bottom, so it holds at the last
        // top that fits.
        let place = beside_tray((8, 900), &primary(), 12);
        assert_eq!(place.top, 1440 - 720 - 12);
    }

    #[test]
    fn a_click_on_a_right_bar_hangs_off_the_right_edge() {
        let place = beside_tray((2552, 700), &primary(), 12);
        assert!(place.at_right && !place.at_bottom);
        assert_eq!(place.right, 12);
    }

    #[test]
    fn a_click_on_a_top_bar_hangs_below_it() {
        let place = beside_tray((1200, 6), &primary(), 12);
        assert!(!place.at_bottom && !place.at_right);
        assert_eq!(place.top, 12);
    }

    #[test]
    fn an_icon_by_the_screen_corner_never_pushes_the_panel_off() {
        let place = beside_tray((2555, 1435), &primary(), 12);
        assert!(place.at_bottom);
        assert_eq!(place.left, 2560 - WIDTH - 12);

        let start = beside_tray((4, 1436), &primary(), 12);
        assert_eq!(start.left, 12);
    }

    #[test]
    fn a_click_on_the_second_screen_uses_that_screens_coordinates() {
        let place = beside_tray((2560 + 900, 1075), &second(), 12);
        assert!(place.at_bottom);
        assert_eq!(place.left, 900 - WIDTH / 2);
    }

    #[test]
    fn the_screen_under_a_point_is_found_and_an_outside_point_is_not() {
        let screens = [primary(), second()];
        assert_eq!(
            screen_containing((2600, 500), &screens).map(|s| s.name.as_str()),
            Some("HDMI-1")
        );
        assert_eq!(screen_containing((0, 0), &screens).map(|s| s.name.as_str()), Some("DP-1"));
        assert!(screen_containing((0, 5000), &screens).is_none());
        assert!(screen_containing((-1, 0), &screens).is_none());
    }

    /// The surface sits at (2200, 300) and the hand presses 100 px into it, then moves 400 px
    /// right: the corner follows by the same 400 px, expressed on its screen.
    #[test]
    fn a_drag_moves_the_surface_by_what_the_hand_moved() {
        let local = dragged_local((2200, 300), (100, 50), (500, 100), &primary());

        assert_eq!(local, (2600, 350));
    }

    #[test]
    fn a_pinned_panel_is_held_fully_on_the_screen() {
        let place = pinned((-50, 9000), &primary(), (WIDTH, 700), 12);
        assert_eq!((place.left, place.top), (12, 1440 - 700 - 12));
    }

    #[test]
    fn a_pinned_position_reads_back_as_itself() {
        let screen = primary();
        let place = pinned((400, 300), &screen, (WIDTH, 600), 12);
        assert_eq!(position_of(&place, &screen, (WIDTH, 600)), (400, 300));
    }

    #[test]
    fn the_corner_reads_back_as_the_bottom_right() {
        let screen = primary();
        let at = position_of(&corner(12), &screen, (WIDTH, 600));
        assert_eq!(at, (2560 - WIDTH - 12, 1440 - 600 - 12));
    }

    #[test]
    fn a_screen_report_reads_and_junk_lines_are_dropped() {
        let read =
            screens_from_report("DP-1\t0\t0\t2560\t1440\nbroken line\nHDMI-1\t2560\t0\t1920\t1080");
        assert_eq!(read.len(), 2);
        assert_eq!(read[1], second());
        assert!(screens_from_report("DP-1\t0\t0\t0\t1440").is_empty());
    }

    #[test]
    fn an_entry_survives_the_trip_and_junk_does_not() {
        assert_eq!(parse(&entry("DP-1", 40, -3)), Some(("DP-1".into(), 40, -3)));
        assert_eq!(parse("no tabs here"), None);
        assert_eq!(parse("\t3\t4"), None);
        assert_eq!(parse("DP-1\tnot a number\t4"), None);
    }

    #[test]
    fn the_last_remembered_point_wins_and_departed_screens_wait() {
        let screens = [primary()];
        let entries = vec![entry("DP-1", 10, 20), entry("HDMI-1", 30, 40)];
        let ((x, y), screen) = last_remembered(&entries, &screens).expect("a point");
        assert_eq!((x, y, screen.name.as_str()), (10, 20, "DP-1"));
    }

    #[test]
    fn remembering_a_screen_again_replaces_its_entry_at_the_end() {
        let entries = vec![entry("DP-1", 1, 1), entry("HDMI-1", 2, 2)];
        let now = with_position(entries, "DP-1", 9, 9);
        assert_eq!(now, vec![entry("HDMI-1", 2, 2), entry("DP-1", 9, 9)]);
    }
}
