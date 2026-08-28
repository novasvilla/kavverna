//! Checks the watcher against the running compositor rather than against a fake, because every
//! interesting failure here lives in the protocol: an offer read after it was destroyed, a
//! selection we set coming back at us as a fresh copy, a read that never sees end of file.
//!
//! These tests take over the clipboard, so they run one at a time and put back what they found.

use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use clipboard_history::selection::{
    CapturePolicy, Payload, Selection, SelectionEvent, SelectionWatcher,
};

const PATIENCE: Duration = Duration::from_secs(3);

fn one_at_a_time() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|held| held.into_inner())
}

struct RestoredClipboard(Option<String>);

impl RestoredClipboard {
    fn save() -> Self {
        Self(paste())
    }
}

impl Drop for RestoredClipboard {
    fn drop(&mut self) {
        match self.0.take() {
            Some(text) => copy(&text),
            None => {
                let _ = detached("wl-copy").arg("--clear").status();
            }
        }
    }
}

/// `wl-copy` stays alive in the background owning the selection. Left attached it holds the
/// test's stdout open, so `cargo test | tail` never sees end of file and the run appears to hang.
fn detached(program: &str) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    command
}

fn copy(text: &str) {
    let status = detached("wl-copy").arg("--").arg(text).status();
    assert!(matches!(status, Ok(code) if code.success()), "wl-copy failed");
}

fn paste() -> Option<String> {
    let out = Command::new("wl-paste")
        .arg("--no-newline")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn next_copy(events: &Receiver<SelectionEvent>) -> Payload {
    loop {
        match events.recv_timeout(PATIENCE) {
            Ok(SelectionEvent::Copied { selection, payload }) => {
                assert_eq!(selection, Selection::Clipboard);
                return payload;
            }
            Ok(SelectionEvent::Emptied(_)) => {}
            Err(RecvTimeoutError::Timeout) => panic!("the compositor reported no copy"),
            Err(RecvTimeoutError::Disconnected) => panic!("the watcher thread stopped"),
        }
    }
}

#[test]
fn a_copy_from_another_application_arrives() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let (_watcher, events) = SelectionWatcher::start(CapturePolicy::default().into())
        .expect("the compositor should offer ext-data-control");

    copy("kavverna sees this");

    assert_eq!(next_copy(&events), Payload::Text("kavverna sees this".into()));
}

#[test]
fn what_was_already_there_is_not_captured() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    copy("copied before the watcher existed");
    let (_watcher, events) = SelectionWatcher::start(CapturePolicy::default().into())
        .expect("the compositor should offer ext-data-control");

    assert!(
        matches!(events.recv_timeout(Duration::from_millis(800)), Err(RecvTimeoutError::Timeout)),
        "switching capture on is not a copy"
    );

    copy("copied after");
    assert_eq!(next_copy(&events), Payload::Text("copied after".into()));
}

#[test]
fn what_we_put_back_is_readable_and_is_not_a_new_copy() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let (watcher, events) = SelectionWatcher::start(CapturePolicy::default().into())
        .expect("the compositor should offer ext-data-control");

    copy("something else entirely");
    next_copy(&events);

    watcher.offer(Selection::Clipboard, Payload::Text("put back by kavverna".into()));

    let mut seen = None;
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(text) = paste() {
            if text == "put back by kavverna" {
                seen = Some(text);
                break;
            }
        }
    }
    assert_eq!(seen.as_deref(), Some("put back by kavverna"), "the paste target read our offer");

    assert!(
        matches!(events.recv_timeout(Duration::from_millis(800)), Err(RecvTimeoutError::Timeout)),
        "our own write must not come back as a copy"
    );
}

#[test]
fn a_copy_marked_as_a_secret_is_never_read() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let (_watcher, events) = SelectionWatcher::start(CapturePolicy::default().into())
        .expect("the compositor should offer ext-data-control");

    let hinted = detached("wl-copy")
        .args(["--type", clipboard_history::CONCEALED_HINT, "--", "secret"])
        .status();
    assert!(matches!(hinted, Ok(code) if code.success()), "wl-copy failed");

    assert!(
        matches!(events.recv_timeout(Duration::from_millis(800)), Err(RecvTimeoutError::Timeout)),
        "a concealed copy is left unread"
    );
}
