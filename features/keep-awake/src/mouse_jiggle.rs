use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Nudges the pointer and puts it straight back, which keeps applications that watch for
/// input rather than power inhibitions from deciding the user has gone away.
///
/// Uses `ydotool` because KWin does not expose the virtual keyboard or pointer protocols,
/// so uinput is the only way to synthesise input on this compositor.
pub struct MouseJiggle {
    interval: Duration,
    last_nudge: Option<Instant>,
    nudges: u32,
}

const SETTLE: Duration = Duration::from_millis(80);

impl MouseJiggle {
    pub fn every(interval: Duration) -> Self {
        Self { interval, last_nudge: None, nudges: 0 }
    }

    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    pub fn is_available() -> bool {
        Command::new("ydotool")
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
        Some(self.interval.saturating_sub(self.last_nudge?.elapsed()))
    }

    /// Call on every tick while the tool is on. Nudges only once the interval is up.
    pub fn tick(&mut self) {
        let due = self.last_nudge.is_none_or(|at| at.elapsed() >= self.interval);
        if due {
            self.nudge_now();
        }
    }

    pub fn rest(&mut self) {
        self.last_nudge = None;
    }

    /// Moves the pointer once whatever the schedule says, so the effect can be seen on demand.
    pub fn nudge_now(&mut self) {
        self.last_nudge = Some(Instant::now());
        self.nudges += 1;

        if !Self::shift(1) {
            tracing::warn!("mouse jiggle needs ydotool and a running ydotoold");
            return;
        }
        std::thread::sleep(SETTLE);
        Self::shift(-1);
        tracing::debug!(nudges = self.nudges, "mouse jiggled");
    }

    fn shift(dx: i32) -> bool {
        Command::new("ydotool")
            .args(["mousemove", "--", &dx.to_string(), "0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resting_forgets_the_schedule() {
        let mut jiggle = MouseJiggle::every(Duration::from_secs(300));
        jiggle.last_nudge = Some(Instant::now());
        jiggle.rest();

        assert!(jiggle.last_nudge.is_none());
        assert_eq!(jiggle.until_next(), None);
    }

    #[test]
    fn the_interval_can_change_while_running() {
        let mut jiggle = MouseJiggle::every(Duration::from_secs(300));
        jiggle.set_interval(Duration::from_secs(60));

        assert_eq!(jiggle.interval, Duration::from_secs(60));
    }

    /// The countdown is what the panel shows, so it has to fall as time passes rather than
    /// report the whole interval every time.
    #[test]
    fn the_countdown_shrinks_after_a_nudge() {
        let mut jiggle = MouseJiggle::every(Duration::from_secs(300));
        jiggle.last_nudge = Some(Instant::now() - Duration::from_secs(100));

        let left = jiggle.until_next().expect("scheduled");
        assert!(left <= Duration::from_secs(200), "left was {left:?}");
        assert!(left > Duration::from_secs(199));
    }

    #[test]
    fn an_overdue_countdown_reads_as_zero_rather_than_wrapping() {
        let mut jiggle = MouseJiggle::every(Duration::from_secs(60));
        jiggle.last_nudge = Some(Instant::now() - Duration::from_secs(600));

        assert_eq!(jiggle.until_next(), Some(Duration::ZERO));
    }
}
