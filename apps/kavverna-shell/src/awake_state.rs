use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default)]
pub struct AwakeState {
    pub active: bool,
    pub remaining: Option<Duration>,
}

static STATE: Mutex<AwakeState> = Mutex::new(AwakeState { active: false, remaining: None });

fn lock() -> MutexGuard<'static, AwakeState> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn get() -> AwakeState {
    *lock()
}

pub fn set_hold(active: bool, remaining: Option<Duration>) {
    let mut state = lock();
    state.active = active;
    state.remaining = remaining;
}
