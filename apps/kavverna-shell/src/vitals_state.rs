use crate::{panel, settings};
use feature_catalog::Feature;
use std::collections::VecDeque;
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use system_monitor::{Processor, Vitals, Vitalsigns};

/// Two minutes of the tick the sampler keeps while the panel is open. Long enough to show the
/// spike that was over before the panel was opened, short enough that a panel left open all day
/// is not drawing an hour of readings into three hundred pixels.
const KEPT: usize = 60;

/// The tick while nobody is looking. The graphics cards are not read at all at this cadence, and
/// it stays under the ten seconds beyond which `system-monitor` refuses to work out a network or disk
/// rate, so the reading shown the moment the panel opens still carries its rates.
const HIDDEN: Duration = Duration::from_secs(8);

/// Lets an opening panel cut a hidden wait short.
static WAKE: Mutex<Option<Sender<()>>> = Mutex::new(None);

static LATEST: Mutex<Option<Vitals>> = Mutex::new(None);
/// What the machine calls its processor, read once when the sampler opens and never again.
static PROCESSOR: Mutex<Option<Processor>> = Mutex::new(None);
static HISTORY: Mutex<Option<History>> = Mutex::new(None);

/// What each meter looked like a moment ago. Kept here rather than in `system-monitor`, which
/// reports one reading and has no idea anybody is drawing a line through them.
#[derive(Default, Clone)]
pub struct History {
    pub cpu: VecDeque<f32>,
    pub memory: VecDeque<f32>,
    /// One per graphics card, indexed the way the cards themselves are, so switching between
    /// them shows that card's own past rather than a line spliced from both. Emptied while the
    /// panel is hidden, since the cards are not read then.
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

pub fn processor() -> Processor {
    PROCESSOR.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone().unwrap_or_default()
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

/// How many open ticks a reading stands for. A reading taken while the panel was hidden covers
/// several, and recording it once for each keeps the trace two minutes wide whichever cadence
/// drew it, instead of squeezing the hidden stretch into a few points. Processor load is
/// measured across the whole gap, so a flat step over it is what the machine averaged; memory
/// and the cards are one moment held flat, which is coarser but never invented.
fn ticks_spanned(gap: Duration, tick: Duration) -> usize {
    let ticks = gap.as_secs_f64() / tick.as_secs_f64().max(f64::EPSILON);
    (ticks.round() as usize).clamp(1, KEPT)
}

fn record(history: &mut History, reading: &Vitals, ticks: usize, cards_read: bool) {
    if !cards_read {
        history.cards.clear();
    }
    for _ in 0..ticks {
        // The first tick has no load yet, since load is the difference between two readings.
        // Recording it as zero would draw a dip that never happened.
        if let Some(load) = reading.cpu_load {
            push_capped(&mut history.cpu, load);
        }
        push_capped(&mut history.memory, reading.memory.used_fraction());
    }
    if !cards_read {
        return;
    }
    // A card's trace that restarted after the panel was hidden begins at this reading, once:
    // stretching it back across the hidden stretch would draw minutes nobody measured.
    history.cards.resize_with(reading.graphics.cards.len(), VecDeque::new);
    for (series, card) in history.cards.iter_mut().zip(&reading.graphics.cards) {
        let times = if series.is_empty() { 1 } else { ticks };
        for _ in 0..times {
            push_capped(series, card.reading.utilisation.unwrap_or(0.0));
        }
    }
}

/// Called when the panel opens. The next reading is taken at once if the last one is at least
/// a tick old; a sooner one would measure load over a sliver of time and draw noise.
pub fn wake() {
    if let Some(sender) = WAKE.lock().ok().and_then(|held| held.clone()) {
        let _ = sender.send(());
    }
}

/// Samples every `tick` while the panel is open and every `HIDDEN` while it is not, and pushes
/// to the interface only while it is open; the panel takes the latest reading as it opens.
/// Load is a difference between readings, so the first sample only establishes a baseline.
pub fn run(tick: Duration, on_change: impl Fn()) {
    let mut vitals = Vitalsigns::for_features(
        settings::is_installed(Feature::SystemMonitor),
        settings::is_installed(Feature::NetworkMonitor),
    );
    *PROCESSOR.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
        Some(vitals.processor().clone());
    tracing::info!(processor = %vitals.processor().name, "vitals sampler started");

    let (sender, wakes) = std::sync::mpsc::channel();
    if let Ok(mut held) = WAKE.lock() {
        *held = Some(sender);
    }

    let mut last: Option<Instant> = None;
    loop {
        let now = Instant::now();
        if let Some(at) = last {
            let due = at + if panel::is_open() { tick } else { HIDDEN };
            if now < due {
                // A wake only sends the loop round again to look at the panel afresh.
                if let Err(RecvTimeoutError::Disconnected) = wakes.recv_timeout(due - now) {
                    std::thread::sleep(due - now);
                }
                continue;
            }
        }

        let open = panel::is_open();
        let reading = if open { vitals.sample() } else { vitals.sample_without_graphics() };
        let ticks = last.map_or(1, |at| ticks_spanned(now.duration_since(at), tick));
        record(history_lock().get_or_insert_with(History::default), &reading, ticks, open);
        last = Some(now);
        *lock() = Some(reading);
        if panel::is_open() {
            on_change();
        }
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

    #[test]
    fn a_hidden_reading_fills_the_ticks_it_covered() {
        let tick = Duration::from_secs(2);

        assert_eq!(ticks_spanned(Duration::from_millis(2100), tick), 1);
        assert_eq!(ticks_spanned(HIDDEN, tick), 4);
        assert_eq!(ticks_spanned(Duration::from_millis(300), tick), 1, "never less than one");
        assert_eq!(ticks_spanned(Duration::from_secs(3600), tick), KEPT, "never more than kept");
    }

    #[test]
    fn a_reading_is_recorded_once_for_every_tick_it_covered() {
        let mut history = History::default();
        let reading = Vitals { cpu_load: Some(0.5), ..Default::default() };

        record(&mut history, &reading, 4, true);

        assert_eq!(history.cpu.len(), 4);
        assert_eq!(history.memory.len(), 4);
    }

    fn with_one_card(utilisation: f32) -> Vitals {
        Vitals {
            cpu_load: Some(0.5),
            graphics: system_monitor::GraphicsReading {
                cards: vec![system_monitor::Gpu {
                    role: system_monitor::GpuRole::Discrete,
                    reading: system_monitor::GpuReading {
                        name: "card".into(),
                        utilisation: Some(utilisation),
                        ..Default::default()
                    },
                }],
            },
            ..Default::default()
        }
    }

    /// The cards are not read while hidden, so their traces restart rather than carrying a
    /// flat line, or zeros, across minutes nobody measured.
    #[test]
    fn a_hidden_reading_restarts_the_card_traces_and_keeps_the_rest() {
        let mut history = History::default();
        record(&mut history, &with_one_card(0.4), 1, true);
        record(&mut history, &with_one_card(0.4), 1, true);

        record(&mut history, &with_one_card(0.0), 4, false);
        assert!(history.cards.is_empty());
        assert_eq!(history.cpu.len(), 6);

        record(&mut history, &with_one_card(0.7), 4, true);
        assert_eq!(history.cards[0], VecDeque::from([0.7]), "a restarted trace begins once");
        record(&mut history, &with_one_card(0.6), 2, true);
        assert_eq!(history.cards[0].len(), 3);
    }

    /// A card unplugged between samples must not leave its old readings attached to whatever
    /// card takes its index.
    #[test]
    fn the_number_of_cards_follows_the_reading() {
        let mut history = History {
            cards: vec![VecDeque::from([0.5]), VecDeque::from([0.9])],
            ..Default::default()
        };

        history.cards.resize_with(1, VecDeque::new);

        assert_eq!(history.cards.len(), 1);
    }
}
