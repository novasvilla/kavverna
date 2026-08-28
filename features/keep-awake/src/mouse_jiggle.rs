use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Moves the pointer somewhere else on an unpredictable schedule, which keeps applications
/// that watch for input rather than power inhibitions from deciding the user has gone away.
///
/// Uses `ydotool` because KWin does not expose the virtual keyboard or pointer protocols,
/// so uinput is the only way to synthesise input on this compositor.
pub struct MouseJiggle {
    shortest: Duration,
    longest: Duration,
    /// Drawn afresh after every nudge, so the rhythm never looks like a timer.
    next_interval: Duration,
    last_nudge: Option<Instant>,
    nudges: u32,
    screen: Option<Screen>,
    activity: Activity,
    keystroke: Keystroke,
}

/// What the nudge actually does. Some idle watchers only count keyboard events, and some
/// only count pointer motion, so neither alone is enough on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Pointer,
    Keyboard,
    Both,
}

impl Activity {
    pub fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Keyboard,
            2 => Self::Both,
            _ => Self::Pointer,
        }
    }

    pub fn id(self) -> i32 {
        match self {
            Self::Pointer => 0,
            Self::Keyboard => 1,
            Self::Both => 2,
        }
    }

    fn moves_pointer(self) -> bool {
        matches!(self, Self::Pointer | Self::Both)
    }

    fn presses_key(self) -> bool {
        matches!(self, Self::Keyboard | Self::Both)
    }
}

/// Linux input event codes. Shift changes nothing anywhere; the arrow pair returns a list
/// selection to where it was but does move a text cursor, so it is not the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keystroke {
    Shift,
    ArrowPair,
}

const KEY_LEFTSHIFT: u16 = 42;
const KEY_DOWN: u16 = 108;
const KEY_UP: u16 = 103;

impl Keystroke {
    pub fn from_id(id: i32) -> Self {
        if id == 1 { Self::ArrowPair } else { Self::Shift }
    }

    pub fn id(self) -> i32 {
        match self {
            Self::Shift => 0,
            Self::ArrowPair => 1,
        }
    }

    fn codes(self) -> &'static [u16] {
        match self {
            Self::Shift => &[KEY_LEFTSHIFT],
            Self::ArrowPair => &[KEY_DOWN, KEY_UP],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub width: i32,
    pub height: i32,
}

/// Kept away from the very edge, where a pointer can land on a panel or an edge action.
const EDGE_MARGIN: i32 = 40;

/// Used until the screen size is known, and when it never becomes known.
const BLIND_JUMP: (i32, i32) = (120, 480);

impl MouseJiggle {
    pub fn between(shortest: Duration, longest: Duration) -> Self {
        let (shortest, longest) = ordered(shortest, longest);

        Self {
            shortest,
            longest,
            next_interval: pick_interval(shortest, longest),
            last_nudge: None,
            nudges: 0,
            screen: None,
            activity: Activity::Pointer,
            keystroke: Keystroke::Shift,
        }
    }

    /// A reversed range is a settings mistake rather than a reason to stop working.
    pub fn set_range(&mut self, shortest: Duration, longest: Duration) {
        let (shortest, longest) = ordered(shortest, longest);
        if (self.shortest, self.longest) == (shortest, longest) {
            return;
        }

        self.shortest = shortest;
        self.longest = longest;
        self.next_interval = pick_interval(shortest, longest);
    }

    /// Without this the pointer can only be moved by a relative jump, since nothing else
    /// here knows how large the desktop is.
    pub fn set_screen(&mut self, screen: Option<Screen>) {
        self.screen = screen;
    }

    pub fn set_activity(&mut self, activity: Activity, keystroke: Keystroke) {
        self.activity = activity;
        self.keystroke = keystroke;
    }

    /// The binary alone is not enough. Without `ydotoold` running and a socket it can reach,
    /// every nudge fails and the switch would sit there looking as if it worked.
    pub fn is_available() -> bool {
        // ydotoold puts its socket in the runtime directory when it has one, and falls back to
        // /tmp when it does not. Both are worth looking at, and YDOTOOL_SOCKET beats either.
        let candidates = std::env::var_os("YDOTOOL_SOCKET")
            .map(|set| vec![std::path::PathBuf::from(set)])
            .unwrap_or_else(|| {
                let mut paths = Vec::new();
                if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
                    paths.push(std::path::PathBuf::from(runtime).join(".ydotool_socket"));
                }
                paths.push(std::path::PathBuf::from("/tmp/.ydotool_socket"));
                paths
            });

        candidates.iter().any(|socket| socket.exists())
            && Command::new("ydotool")
                .arg("--help")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok()
    }

    pub fn nudges(&self) -> u32 {
        self.nudges
    }

    /// `None` before the first nudge, when nothing is scheduled yet.
    pub fn until_next(&self) -> Option<Duration> {
        Some(self.next_interval.saturating_sub(self.last_nudge?.elapsed()))
    }

    pub fn next_interval(&self) -> Duration {
        self.next_interval
    }

    /// Call on every tick while the tool is on.
    pub fn tick(&mut self) {
        let due = self.last_nudge.is_none_or(|at| at.elapsed() >= self.next_interval);
        if due {
            self.nudge_now();
        }
    }

    pub fn rest(&mut self) {
        self.last_nudge = None;
    }

    /// Moves the pointer whatever the schedule says, so the effect can be seen on demand.
    pub fn nudge_now(&mut self) {
        self.last_nudge = Some(Instant::now());
        self.next_interval = pick_interval(self.shortest, self.longest);
        self.nudges += 1;

        let mut acted = false;

        if self.activity.moves_pointer() {
            acted |= match self.screen {
                Some(screen) => {
                    let (x, y) = pick_point(screen);
                    Self::move_to(x, y)
                }
                None => {
                    let (dx, dy) = pick_jump();
                    Self::move_by(dx, dy)
                }
            };
        }

        if self.activity.presses_key() {
            acted |= self.press();
        }

        if acted {
            tracing::debug!(nudges = self.nudges, "pointer moved");
        } else {
            tracing::warn!("mouse jiggle needs ydotool and a running ydotoold");
        }
    }

    /// Each key is pressed and released in one call, so an interrupted run cannot leave a
    /// modifier stuck down.
    fn press(&self) -> bool {
        self.keystroke
            .codes()
            .iter()
            .all(|code| Self::run(&["key", &format!("{code}:1"), &format!("{code}:0")]))
    }

    fn move_to(x: i32, y: i32) -> bool {
        Self::run(&["mousemove", "--absolute", "-x", &x.to_string(), "-y", &y.to_string()])
    }

    fn move_by(dx: i32, dy: i32) -> bool {
        Self::run(&["mousemove", "--", &dx.to_string(), &dy.to_string()])
    }

    fn run(args: &[&str]) -> bool {
        Command::new("ydotool")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

fn ordered(a: Duration, b: Duration) -> (Duration, Duration) {
    if a <= b { (a, b) } else { (b, a) }
}

fn pick_interval(shortest: Duration, longest: Duration) -> Duration {
    let (low, high) = (shortest.as_secs(), longest.as_secs());
    Duration::from_secs(if high <= low { low } else { fastrand::u64(low..=high) })
}

fn pick_point(screen: Screen) -> (i32, i32) {
    let span = |size: i32| {
        let low = EDGE_MARGIN.min(size / 2);
        let high = (size - EDGE_MARGIN).max(low + 1);
        fastrand::i32(low..high)
    };

    (span(screen.width.max(2)), span(screen.height.max(2)))
}

fn pick_jump() -> (i32, i32) {
    let leg = || {
        let distance = fastrand::i32(BLIND_JUMP.0..=BLIND_JUMP.1);
        if fastrand::bool() { distance } else { -distance }
    };

    (leg(), leg())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(count: u64) -> Duration {
        Duration::from_secs(count * 60)
    }

    #[test]
    fn the_activity_choice_survives_a_round_trip() {
        for activity in [Activity::Pointer, Activity::Keyboard, Activity::Both] {
            assert_eq!(Activity::from_id(activity.id()), activity);
        }
        for stroke in [Keystroke::Shift, Keystroke::ArrowPair] {
            assert_eq!(Keystroke::from_id(stroke.id()), stroke);
        }
    }

    #[test]
    fn an_unknown_choice_falls_back_to_moving_the_pointer() {
        assert_eq!(Activity::from_id(99), Activity::Pointer);
        assert_eq!(Keystroke::from_id(-1), Keystroke::Shift);
    }

    #[test]
    fn each_activity_does_what_its_name_says() {
        assert!(Activity::Pointer.moves_pointer() && !Activity::Pointer.presses_key());
        assert!(!Activity::Keyboard.moves_pointer() && Activity::Keyboard.presses_key());
        assert!(Activity::Both.moves_pointer() && Activity::Both.presses_key());
    }

    /// The arrow pair has to come back to where it started, or a long session would walk a
    /// selection down the screen.
    #[test]
    fn the_arrow_pair_cancels_itself_out() {
        assert_eq!(Keystroke::ArrowPair.codes(), &[KEY_DOWN, KEY_UP]);
        assert_eq!(Keystroke::Shift.codes(), &[KEY_LEFTSHIFT]);
    }

    #[test]
    fn resting_forgets_the_schedule() {
        let mut jiggle = MouseJiggle::between(minutes(1), minutes(5));
        jiggle.last_nudge = Some(Instant::now());
        jiggle.rest();

        assert!(jiggle.last_nudge.is_none());
        assert_eq!(jiggle.until_next(), None);
    }

    /// A fixed rhythm is exactly what an idle watcher can learn to ignore.
    #[test]
    fn the_wait_lands_inside_the_range_and_varies() {
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..200 {
            let picked = pick_interval(minutes(1), minutes(9));
            assert!(picked >= minutes(1) && picked <= minutes(9), "picked {picked:?}");
            seen.insert(picked.as_secs());
        }

        assert!(seen.len() > 5, "the wait barely varied: {} distinct values", seen.len());
    }

    #[test]
    fn a_range_of_one_value_still_works() {
        assert_eq!(pick_interval(minutes(3), minutes(3)), minutes(3));
    }

    /// Someone will set the longest below the shortest, and it must not panic.
    #[test]
    fn a_reversed_range_is_put_back_in_order() {
        let jiggle = MouseJiggle::between(minutes(10), minutes(2));

        assert_eq!(jiggle.shortest, minutes(2));
        assert_eq!(jiggle.longest, minutes(10));
        assert!(jiggle.next_interval >= minutes(2) && jiggle.next_interval <= minutes(10));
    }

    #[test]
    fn changing_the_range_redraws_the_wait() {
        let mut jiggle = MouseJiggle::between(minutes(30), minutes(30));
        assert_eq!(jiggle.next_interval, minutes(30));

        jiggle.set_range(minutes(1), minutes(2));

        assert!(jiggle.next_interval >= minutes(1) && jiggle.next_interval <= minutes(2));
    }

    #[test]
    fn the_point_stays_on_screen_and_off_the_edges() {
        let screen = Screen { width: 5120, height: 1440 };

        for _ in 0..500 {
            let (x, y) = pick_point(screen);
            assert!((EDGE_MARGIN..screen.width - EDGE_MARGIN).contains(&x), "x was {x}");
            assert!((EDGE_MARGIN..screen.height - EDGE_MARGIN).contains(&y), "y was {y}");
        }
    }

    /// A screen smaller than the margins would otherwise build an empty range and panic.
    #[test]
    fn a_tiny_screen_does_not_panic() {
        for size in [1, 2, 40, 79, 80, 81] {
            let (x, y) = pick_point(Screen { width: size, height: size });
            assert!(x >= 0 && y >= 0);
        }
    }

    #[test]
    fn the_point_lands_somewhere_different_each_time() {
        let screen = Screen { width: 1920, height: 1080 };
        let points: std::collections::BTreeSet<_> = (0..100).map(|_| pick_point(screen)).collect();

        assert!(points.len() > 50, "only {} distinct points", points.len());
    }

    /// Without a known screen the jump has to be large enough to be seen, unlike the single
    /// pixel this started as.
    #[test]
    fn a_blind_jump_is_big_enough_to_notice() {
        for _ in 0..200 {
            let (dx, dy) = pick_jump();
            assert!((BLIND_JUMP.0..=BLIND_JUMP.1).contains(&dx.abs()), "dx was {dx}");
            assert!((BLIND_JUMP.0..=BLIND_JUMP.1).contains(&dy.abs()), "dy was {dy}");
        }
    }

    #[test]
    fn a_blind_jump_goes_both_ways() {
        let signs: std::collections::BTreeSet<_> =
            (0..200).map(|_| pick_jump().0.signum()).collect();

        assert_eq!(signs.len(), 2, "the jump only ever went one way");
    }
}
