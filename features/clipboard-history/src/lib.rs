//! The clipboard: what was copied, what is kept, and what goes back.

pub mod selection;

pub use selection::{
    CONCEALED_HINT, CapturePolicy, Payload, Selection, SelectionEvent, SelectionWatcher, WatchError,
};
