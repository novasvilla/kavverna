use std::sync::Mutex;

#[derive(Debug, Clone, Copy, Default)]
pub struct JiggleState {
    pub running: bool,
    pub nudges: u32,
    pub seconds_until_next: Option<u64>,
    pub waiting_seconds: u64,
}

static STATE: Mutex<JiggleState> = Mutex::new(JiggleState {
    running: false,
    nudges: 0,
    seconds_until_next: None,
    waiting_seconds: 0,
});

/// Published by the shell once Qt knows the desktop size, since nothing on the jiggle side
/// can discover it.
static SCREEN: Mutex<Option<keep_awake::Screen>> = Mutex::new(None);

pub fn screen() -> Option<keep_awake::Screen> {
    *SCREEN.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn set_screen(width: i32, height: i32) {
    let known = (width > 0 && height > 0).then_some(keep_awake::Screen { width, height });
    *SCREEN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = known;
}

pub fn get() -> JiggleState {
    *STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn set(state: JiggleState) {
    *STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = state;
}
