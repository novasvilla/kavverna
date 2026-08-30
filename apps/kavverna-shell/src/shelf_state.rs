use crate::{settings, shelf_view};
use shelf::Shelf;
use std::sync::{Mutex, MutexGuard};

static SHELF: Mutex<Option<Shelf>> = Mutex::new(None);

fn lock() -> MutexGuard<'static, Option<Shelf>> {
    SHELF.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Loads on its own thread so starting the shell is never held behind a directory scan, and
/// the startup sweep of orphaned blobs happens off the interface's path.
pub fn prepare() {
    std::thread::spawn(|| {
        let Some(dir) = Shelf::usual_dir() else {
            tracing::error!("no data directory, so the shelf cannot keep anything");
            return;
        };
        let keep = settings::bool_at(
            settings::SHELF_KEEP_ACROSS_RESTARTS,
            settings::SHELF_KEEP_ACROSS_RESTARTS_DEFAULT,
        );
        let shelf = Shelf::open(dir, keep);
        tracing::info!(items = shelf.len(), "the shelf is up");
        *lock() = Some(shelf);
        shelf_view::publish();
    });
}

/// Every mutation happens on the Qt thread through here; the mutex only exists because
/// `prepare` fills the shelf in from its own thread.
pub fn with<R>(reach: impl FnOnce(&mut Shelf) -> R) -> Option<R> {
    lock().as_mut().map(reach)
}
