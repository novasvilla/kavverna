//! Emptying the system clipboard on its own.
//!
//! Saved entries are never touched: this is about what is still pasteable, not about what was
//! kept. It works with the history switched off, which is the point for anyone who wants a
//! clipboard that forgets rather than one that remembers.

use std::time::{Duration, Instant};

pub const SHORTEST_DELAY: Duration = Duration::from_secs(5);
pub const LONGEST_DELAY: Duration = Duration::from_secs(3600);
pub const DEFAULT_DELAY: Duration = Duration::from_secs(20);

pub fn sanitized_delay(delay: Duration) -> Duration {
    delay.clamp(SHORTEST_DELAY, LONGEST_DELAY)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trigger {
    Delay,
    Suspend,
    ScreenLock,
}

#[derive(Debug, Default)]
pub struct AutoClear {
    after: Option<Duration>,
    copied_at: Option<Instant>,
}

impl AutoClear {
    pub fn set_delay(&mut self, after: Option<Duration>) {
        self.after = after.map(sanitized_delay);
    }

    /// Whatever was already on the clipboard gets a full delay, so switching the setting on
    /// never wipes something copied a moment earlier.
    pub fn noticed_copy(&mut self, at: Instant) {
        self.copied_at = Some(at);
    }

    pub fn forget(&mut self) {
        self.copied_at = None;
    }

    pub fn due(&self, now: Instant) -> bool {
        match (self.after, self.copied_at) {
            (Some(after), Some(copied_at)) => now.duration_since(copied_at) >= after,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> Instant {
        Instant::now() + Duration::from_secs(seconds)
    }

    #[test]
    fn nothing_is_due_without_a_delay_set() {
        let mut clear = AutoClear::default();
        clear.noticed_copy(Instant::now());
        assert!(!clear.due(at(3600)));
    }

    #[test]
    fn nothing_is_due_until_something_has_been_copied() {
        let mut clear = AutoClear::default();
        clear.set_delay(Some(Duration::from_secs(10)));
        assert!(!clear.due(at(3600)));
    }

    #[test]
    fn the_wait_starts_at_the_copy() {
        let mut clear = AutoClear::default();
        clear.set_delay(Some(Duration::from_secs(30)));
        let copied = Instant::now();
        clear.noticed_copy(copied);

        assert!(!clear.due(copied + Duration::from_secs(29)));
        assert!(clear.due(copied + Duration::from_secs(30)));
    }

    #[test]
    fn a_second_copy_restarts_the_wait() {
        let mut clear = AutoClear::default();
        clear.set_delay(Some(Duration::from_secs(30)));
        let first = Instant::now();
        clear.noticed_copy(first);
        clear.noticed_copy(first + Duration::from_secs(20));

        assert!(!clear.due(first + Duration::from_secs(45)));
        assert!(clear.due(first + Duration::from_secs(50)));
    }

    #[test]
    fn clearing_stops_it_from_clearing_again() {
        let mut clear = AutoClear::default();
        clear.set_delay(Some(Duration::from_secs(10)));
        let copied = Instant::now();
        clear.noticed_copy(copied);
        assert!(clear.due(copied + Duration::from_secs(10)));

        clear.forget();
        assert!(!clear.due(copied + Duration::from_secs(3600)));
    }

    #[test]
    fn a_delay_outside_what_the_interface_offers_is_brought_back_in() {
        assert_eq!(sanitized_delay(Duration::from_secs(1)), SHORTEST_DELAY);
        assert_eq!(sanitized_delay(Duration::from_secs(999_999)), LONGEST_DELAY);
        assert_eq!(sanitized_delay(Duration::from_secs(45)), Duration::from_secs(45));
    }
}
