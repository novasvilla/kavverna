//! The priority write goes through the user's systemd and is read back from the kernel, which
//! needs a live session bus and a user manager with the cpu controller delegated. Run with
//! `cargo test -p system-monitor -- --include-ignored` on a desktop.

use std::process::Command;
use std::time::Duration;
use system_monitor::{Action, CpuWeight, ProcessControl, ProcessKey};

fn started(pid: u32) -> u64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let after = stat.rsplit_once(')').unwrap().1;
    after.split_whitespace().nth(19).unwrap().parse().unwrap()
}

#[test]
#[ignore = "needs the user's systemd on a live session bus"]
fn a_throwaway_scope_takes_the_weight_and_the_kernel_shows_it() {
    let unit = format!("kavverna-weight-check-{}.scope", std::process::id());
    let mut scope = Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", &format!("--unit={unit}"), "sleep", "30"])
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    // `systemd-run --scope` execs the command, so the child is the sleep itself.
    let target = ProcessKey { pid: scope.id(), started: started(scope.id()) };
    let control = ProcessControl::for_this_session();

    let applied = control.set_cpu_weight(&[target], CpuWeight::LOW).unwrap();
    assert_eq!(applied, vec![(unit.clone(), CpuWeight::LOW)]);
    assert_eq!(control.cpu_weight(target), Some((unit, 10)));

    assert!(control.act(target, Action::ForceStop).complete());
    scope.wait().unwrap();
}
