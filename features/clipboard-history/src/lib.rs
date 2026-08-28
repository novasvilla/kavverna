//! The clipboard: what was copied, what is kept, and what goes back.

pub mod entry;
pub mod selection;
pub mod sensitivity;
pub mod store;

pub use entry::{Entry, Kind, StoredImage};
pub use selection::{
    CONCEALED_HINT, CapturePolicy, Payload, Selection, SelectionEvent, SelectionWatcher, WatchError,
};
pub use sensitivity::looks_sensitive;
pub use store::{Captured, Store, StoreError, Summary};
