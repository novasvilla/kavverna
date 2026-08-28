use keep_awake::{Hold, KeepAwake, Scope};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// logind's inhibitor list is process-wide state, so these tests cannot overlap.
static LOGIND: Mutex<()> = Mutex::new(());

fn exclusive() -> MutexGuard<'static, ()> {
    LOGIND.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn logind_reports_kavverna() -> bool {
    Command::new("systemd-inhibit")
        .arg("--list")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains("Kavverna"))
        .unwrap_or(false)
}

#[tokio::test]
async fn the_inhibitor_appears_and_disappears_with_the_hold() {
    let _exclusive = exclusive();

    let Ok(mut keep_awake) = KeepAwake::connect().await else {
        eprintln!("skipped: no session bus");
        return;
    };

    assert!(!logind_reports_kavverna(), "a previous run left an inhibitor behind");

    keep_awake
        .engage(Hold::For(Duration::from_secs(60)), Scope::SystemOnly)
        .await
        .expect("engage");

    assert!(keep_awake.is_active());
    assert!(logind_reports_kavverna(), "logind never saw the inhibitor");

    keep_awake.release().await;

    assert!(!keep_awake.is_active());
    assert!(!logind_reports_kavverna(), "the inhibitor outlived its hold");
}

#[tokio::test]
async fn a_lapsed_hold_releases_itself() {
    let _exclusive = exclusive();

    let Ok(mut keep_awake) = KeepAwake::connect().await else {
        eprintln!("skipped: no session bus");
        return;
    };

    keep_awake
        .engage(Hold::For(Duration::from_millis(50)), Scope::SystemOnly)
        .await
        .expect("engage");

    tokio::time::sleep(Duration::from_millis(120)).await;

    assert!(keep_awake.expire_if_due().await, "the hold should have lapsed");
    assert!(!keep_awake.is_active());
}
