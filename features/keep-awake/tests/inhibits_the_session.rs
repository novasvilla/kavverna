use keep_awake::{Hold, KeepAwake, Scope, Trigger};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// The power daemon's inhibition list is session-wide state, so these tests take turns. Each
/// holds the guard across its awaits on purpose: releasing it early is what would let them
/// interfere, which is why they carry `allow(clippy::await_holding_lock)`.
static POWER_DAEMON: Mutex<()> = Mutex::new(());

fn exclusive() -> MutexGuard<'static, ()> {
    POWER_DAEMON.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Its own identity, so the suite can run while the real application holds an inhibition.
const TEST_WHO: &str = "Kavverna test suite";

/// Asks the daemon that actually performs the suspend, rather than trusting our own state.
fn power_daemon_lists_kavverna() -> bool {
    Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.kde.Solid.PowerManagement",
            "/org/kde/Solid/PowerManagement/PolicyAgent",
            "org.kde.Solid.PowerManagement.PolicyAgent",
            "ListInhibitions",
        ])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains(TEST_WHO))
        .unwrap_or(false)
}

/// The daemon publishes its list a moment after accepting a change, so a single read right
/// after the call is a race rather than an answer.
fn settles_on(expected: bool, probe: fn() -> bool) -> bool {
    for _ in 0..120 {
        if probe() == expected {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a live session bus with PowerDevil on it"]
async fn the_power_daemon_honours_the_hold() {
    let _exclusive = exclusive();

    let Ok(mut keep_awake) = KeepAwake::connect_as(TEST_WHO).await else {
        eprintln!("skipped: no session bus");
        return;
    };

    assert!(!power_daemon_lists_kavverna(), "a previous run left an inhibition behind");

    keep_awake
        .engage(Hold::For(Duration::from_secs(60)), Scope::SystemOnly, Trigger::Manual)
        .await
        .expect("engage");

    assert!(keep_awake.is_active());
    assert!(
        keep_awake.power_daemon_holds(),
        "the power daemon refused the inhibition, so the machine would still suspend"
    );
    assert!(
        settles_on(true, power_daemon_lists_kavverna),
        "the power daemon never listed the hold"
    );

    keep_awake.release().await;

    assert!(!keep_awake.is_active());
    assert!(settles_on(false, power_daemon_lists_kavverna), "the inhibition outlived its hold");
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a live session bus with PowerDevil on it"]
async fn a_lapsed_hold_releases_itself() {
    let _exclusive = exclusive();

    let Ok(mut keep_awake) = KeepAwake::connect_as(TEST_WHO).await else {
        eprintln!("skipped: no session bus");
        return;
    };

    keep_awake
        .engage(Hold::For(Duration::from_millis(50)), Scope::SystemOnly, Trigger::Manual)
        .await
        .expect("engage");

    tokio::time::sleep(Duration::from_millis(120)).await;

    assert!(keep_awake.expire_if_due().await, "the hold should have lapsed");
    assert!(!keep_awake.is_active());
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a live session bus with PowerDevil on it"]
async fn extending_pushes_the_deadline_out() {
    let _exclusive = exclusive();

    let Ok(mut keep_awake) = KeepAwake::connect_as(TEST_WHO).await else {
        eprintln!("skipped: no session bus");
        return;
    };

    keep_awake
        .engage(Hold::For(Duration::from_secs(60)), Scope::SystemOnly, Trigger::Manual)
        .await
        .expect("engage");

    let before = keep_awake.remaining().expect("timed hold");
    assert!(keep_awake.extend(Duration::from_secs(900)));
    let after = keep_awake.remaining().expect("still timed");

    assert!(after > before + Duration::from_secs(880), "extend did not add the time");

    keep_awake.release().await;
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a live session bus with PowerDevil on it"]
async fn an_indefinite_hold_has_nothing_to_extend() {
    let _exclusive = exclusive();

    let Ok(mut keep_awake) = KeepAwake::connect_as(TEST_WHO).await else {
        eprintln!("skipped: no session bus");
        return;
    };

    keep_awake.engage(Hold::Indefinite, Scope::SystemOnly, Trigger::Manual).await.expect("engage");

    assert!(!keep_awake.is_timed());
    assert!(!keep_awake.extend(Duration::from_secs(900)));
    assert!(keep_awake.remaining().is_none());

    keep_awake.release().await;
}
