use std::sync::Mutex;

#[derive(Debug, Clone, Copy, Default)]
pub struct JiggleState {
    pub running: bool,
    pub nudges: u32,
    pub seconds_until_next: Option<u64>,
}

static STATE: Mutex<JiggleState> = Mutex::new(JiggleState {
    running: false,
    nudges: 0,
    seconds_until_next: None,
});

pub fn get() -> JiggleState {
    *STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn set(state: JiggleState) {
    *STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = state;
}
