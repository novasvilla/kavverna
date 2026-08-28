//! Checks the watcher against the running compositor rather than against a fake, because every
//! interesting failure here lives in the protocol: an offer read after it was destroyed, a
//! selection we set coming back at us as a fresh copy, a read that never sees end of file.
//!
//! These tests take over the clipboard, so they run one at a time and put back what they found.
//! Ignored by default for the same reason: run them with `-- --include-ignored` on a desktop, and
//! stop Kavverna first or their copies land in your real history.

use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Mutex, MutexGuard, OnceLock};
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

fn watching() -> (SelectionWatcher, Receiver<SelectionEvent>) {
    watching_with(CapturePolicy::default())
}

fn watching_with(policy: CapturePolicy) -> (SelectionWatcher, Receiver<SelectionEvent>) {
    let (out, events) = channel();
    let watcher = SelectionWatcher::start(
        policy.into(),
        Box::new(move |event| {
            let _ = out.send(event);
        }),
    )
    .expect("the compositor should offer ext-data-control");
    (watcher, events)
}

fn next_copy(events: &Receiver<SelectionEvent>) -> Payload {
    loop {
        match events.recv_timeout(PATIENCE) {
            Ok(SelectionEvent::Copied { selection, payload, .. }) => {
                assert_eq!(selection, Selection::Clipboard);
                return payload;
            }
            Ok(SelectionEvent::Emptied(_) | SelectionEvent::Changed(_)) => {}
            Err(RecvTimeoutError::Timeout) => panic!("the compositor reported no copy"),
            Err(RecvTimeoutError::Disconnected) => panic!("the watcher thread stopped"),
        }
    }
}

#[test]
#[ignore = "needs a live compositor offering ext-data-control"]
fn a_copy_from_another_application_arrives() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let (_watcher, events) = watching();

    copy("kavverna sees this");

    assert_eq!(next_copy(&events), Payload::Text("kavverna sees this".into()));
}

#[test]
#[ignore = "needs a live compositor offering ext-data-control"]
fn what_was_already_there_is_not_captured() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    copy("copied before the watcher existed");
    let (_watcher, events) = watching();

    assert!(
        matches!(events.recv_timeout(Duration::from_millis(800)), Err(RecvTimeoutError::Timeout)),
        "switching capture on is not a copy"
    );

    copy("copied after");
    assert_eq!(next_copy(&events), Payload::Text("copied after".into()));
}

#[test]
#[ignore = "needs a live compositor offering ext-data-control"]
fn what_we_put_back_is_readable_and_is_not_a_new_copy() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let (watcher, events) = watching();

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

    // A watcher that died at startup would also report nothing, so the silence above only means
    // something once an ordinary copy proves it was listening the whole time.
    copy("an ordinary copy afterwards");
    assert_eq!(next_copy(&events), Payload::Text("an ordinary copy afterwards".into()));
}

#[test]
#[ignore = "needs a live compositor offering ext-data-control"]
fn a_copy_marked_as_a_secret_is_noticed_but_never_read() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let (_watcher, events) = watching();

    let hinted = detached("wl-copy")
        .args(["--type", clipboard_history::CONCEALED_HINT, "--", "secret"])
        .status();
    assert!(matches!(hinted, Ok(code) if code.success()), "wl-copy failed");

    // Unread, but not unnoticed: a password is the last thing that should sit on the clipboard
    // until something else replaces it, so the clear timer still has to start.
    match events.recv_timeout(PATIENCE) {
        Ok(SelectionEvent::Changed(selection)) => assert_eq!(selection, Selection::Clipboard),
        Ok(other) => panic!("the secret was read: {other:?}"),
        Err(_) => panic!("a concealed copy has to start the clear timer"),
    }
}

#[test]
#[ignore = "needs a live compositor offering ext-data-control"]
fn a_copy_reaches_the_history_and_can_be_put_back() {
    use clipboard_history::history::{Command, History, Settings, Snapshot};

    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let room = tempfile::tempdir().expect("a temporary directory");
    let (history, snapshots) =
        History::start(room.path(), Settings::default()).expect("the history should start");

    copy("saved by the history");

    let holding =
        |snapshot: &Snapshot| snapshot.rows.iter().any(|row| row.preview == "saved by the history");
    let mut arrived = None;
    let patience = std::time::Instant::now() + PATIENCE;
    while std::time::Instant::now() < patience {
        match snapshots.recv_timeout(Duration::from_millis(200)) {
            Ok(snapshot) if holding(&snapshot) => {
                arrived = Some(snapshot);
                break;
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
    let arrived = arrived.expect("the copy should have reached the history");
    assert_eq!(arrived.recent, 1);
    assert_eq!(arrived.pinned, 0);

    copy("something else entirely");
    let id = arrived.rows.iter().find(|row| row.preview == "saved by the history").unwrap().id;
    history.send(Command::PutBack(id));

    let mut back = None;
    for _ in 0..25 {
        std::thread::sleep(Duration::from_millis(100));
        if paste().as_deref() == Some("saved by the history") {
            back = Some(());
            break;
        }
    }
    assert!(back.is_some(), "the entry should be on the clipboard again");
}

/// Plasma puts the content back the moment anything empties the selection. Reading its
/// re-assertion as a fresh copy restarts the auto clear timer, and the two then fight once a
/// second until one of them stops.
#[test]
#[ignore = "needs a live compositor offering ext-data-control"]
fn plasma_putting_the_clipboard_back_is_not_a_new_copy() {
    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let (_watcher, events) = watching();

    let replaced = detached("wl-copy")
        .args(["--type", "application/x-kde-onlyReplaceEmpty", "--", "back again"])
        .status();
    assert!(matches!(replaced, Ok(code) if code.success()), "wl-copy failed");

    assert!(
        matches!(events.recv_timeout(Duration::from_millis(800)), Err(RecvTimeoutError::Timeout)),
        "an offer Plasma re-asserted is not a copy anyone made"
    );

    // The same control: without it a watcher that never connected passes this test.
    copy("an ordinary copy afterwards");
    assert_eq!(next_copy(&events), Payload::Text("an ordinary copy afterwards".into()));
}

/// The claim that switching the history off stops the content reaching this process at all is
/// only worth making if it is true, and it was not: an edit meant to gate the read never landed
/// in the file and nothing noticed.
#[test]
#[ignore = "needs a live compositor offering ext-data-control"]
fn with_reading_off_the_content_is_never_taken() {
    use std::sync::atomic::AtomicBool;

    let _guard = one_at_a_time();
    let _restore = RestoredClipboard::save();

    let policy = CapturePolicy { read_content: AtomicBool::new(false), ..Default::default() };
    let (_watcher, events) = watching_with(policy);

    copy("nobody should read this");

    match events.recv_timeout(PATIENCE) {
        Ok(SelectionEvent::Changed(selection)) => assert_eq!(selection, Selection::Clipboard),
        Ok(other) => panic!("the content was read: {other:?}"),
        Err(_) => panic!("the copy went unnoticed"),
    }
}
