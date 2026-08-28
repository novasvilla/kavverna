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
}

const SETTLE: Duration = Duration::from_millis(80);

impl MouseJiggle {
    pub fn every(interval: Duration) -> Self {
        Self { interval, last_nudge: None }
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

    /// Call on every tick while a hold is running. Nudges only once the interval is up.
    pub fn tick(&mut self) {
        let due = self.last_nudge.is_none_or(|at| at.elapsed() >= self.interval);
        if !due {
            return;
        }

        self.last_nudge = Some(Instant::now());
        self.nudge();
    }

    pub fn rest(&mut self) {
        self.last_nudge = None;
    }

    fn nudge(&self) {
        if !Self::shift(1) {
            tracing::warn!("mouse jiggle needs ydotool and a running ydotoold");
            return;
        }
        std::thread::sleep(SETTLE);
        Self::shift(-1);
        tracing::debug!("mouse jiggled");
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
    fn resting_forgets_the_last_nudge() {
        let mut jiggle = MouseJiggle::every(Duration::from_secs(300));
        jiggle.last_nudge = Some(Instant::now());
        jiggle.rest();

        assert!(jiggle.last_nudge.is_none());
    }

    #[test]
    fn the_interval_can_change_while_running() {
        let mut jiggle = MouseJiggle::every(Duration::from_secs(300));
        jiggle.set_interval(Duration::from_secs(60));

        assert_eq!(jiggle.interval, Duration::from_secs(60));
    }
}
