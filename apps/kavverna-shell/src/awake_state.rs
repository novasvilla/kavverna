use crate::command::{self, Command};
use crate::settings;
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

/// The one place that decides what toggling means, reached by the panel switch, the tray menu,
/// the middle click and the global shortcut. It was written out four times before, and the
/// middle click held sleep off with no end whatever the default duration was set to.
pub fn toggle() {
    let command = if get().active {
        Command::Release
    } else {
        Command::Engage(settings::default_hold(), settings::scope())
    };
    command::send(command);
}
