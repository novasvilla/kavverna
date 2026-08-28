use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use system_monitor::{Vitals, Vitalsigns};

static LATEST: Mutex<Option<Vitals>> = Mutex::new(None);

fn lock() -> MutexGuard<'static, Option<Vitals>> {
    LATEST.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn get() -> Vitals {
    lock().clone().unwrap_or_default()
}

/// Samples on a fixed tick. Load is a difference between readings, so the first tick only
/// establishes a baseline.
pub fn run(interval: Duration, on_change: impl Fn()) {
    let mut vitals = Vitalsigns::open();
    tracing::info!("vitals sampler started");

    loop {
        let reading = vitals.sample();
        *lock() = Some(reading);
        on_change();
        std::thread::sleep(interval);
    }
}
