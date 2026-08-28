//! What Kavverna asks of KDE, over D-Bus rather than by linking, since KF6 ships no
//! pkg-config files and this build has no CMake.

pub mod shortcuts;

pub use shortcuts::{ALT, CONTROL, META, SHIFT, Shortcut, ShortcutError};
