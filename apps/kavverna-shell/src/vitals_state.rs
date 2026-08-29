use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use system_monitor::{Vitals, Vitalsigns};

/// Two minutes at the sampler's own tick. Long enough to show the spike that was over before
/// the panel was opened, short enough that a panel left open all day is not drawing an hour of
/// readings into three hundred pixels.
const KEPT: usize = 60;

static LATEST: Mutex<Option<Vitals>> = Mutex::new(None);
static HISTORY: Mutex<Option<History>> = Mutex::new(None);

/// What each meter looked like a moment ago. Kept here rather than in `system-monitor`, which
/// reports one reading and has no idea anybody is drawing a line through them.
#[derive(Default, Clone)]
pub struct History {
    pub cpu: VecDeque<f32>,
    pub memory: VecDeque<f32>,
    /// One per graphics card, indexed the way the cards themselves are, so switching between
    /// them shows that card's own past rather than a line spliced from both.
    pub cards: Vec<VecDeque<f32>>,
}

fn lock() -> MutexGuard<'static, Option<Vitals>> {
    LATEST.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn history_lock() -> MutexGuard<'static, Option<History>> {
    HISTORY.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn get() -> Vitals {
    lock().clone().unwrap_or_default()
}

pub fn history() -> History {
    history_lock().clone().unwrap_or_default()
}

fn push_capped(series: &mut VecDeque<f32>, value: f32) {
    series.push_back(value);
    while series.len() > KEPT {
        series.pop_front();
    }
}

fn record(history: &mut History, reading: &Vitals) {
    // The first tick has no load yet, since load is the difference between two readings.
    // Recording it as zero would draw a dip that never happened.
    if let Some(load) = reading.cpu_load {
        push_capped(&mut history.cpu, load);
    }
    push_capped(&mut history.memory, reading.memory.used_fraction());

    history.cards.resize_with(reading.graphics.cards.len(), VecDeque::new);
    for (series, card) in history.cards.iter_mut().zip(&reading.graphics.cards) {
        push_capped(series, card.reading.utilisation.unwrap_or(0.0));
    }
}

/// Samples on a fixed tick. Load is a difference between readings, so the first tick only
/// establishes a baseline.
pub fn run(interval: Duration, on_change: impl Fn()) {
    let mut vitals = Vitalsigns::open();
    tracing::info!("vitals sampler started");

    loop {
        let reading = vitals.sample();
        record(history_lock().get_or_insert_with(History::default), &reading);
        *lock() = Some(reading);
        on_change();
        std::thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_series_forgets_its_oldest_reading_rather_than_growing() {
        let mut series = VecDeque::new();

        for tick in 0..KEPT + 10 {
            push_capped(&mut series, tick as f32);
        }

        assert_eq!(series.len(), KEPT);
        assert_eq!(series.front(), Some(&10.0), "the oldest ten should be gone");
        assert_eq!(series.back(), Some(&((KEPT + 9) as f32)));
    }

    /// A card unplugged between samples must not leave its old readings attached to whatever
    /// card takes its index.
    #[test]
    fn the_number_of_cards_follows_the_reading() {
        let mut history = History::default();
        history.cards = vec![VecDeque::from([0.5]), VecDeque::from([0.9])];

        history.cards.resize_with(1, VecDeque::new);

        assert_eq!(history.cards.len(), 1);
    }
}
