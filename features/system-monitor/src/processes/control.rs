//! Ending, pausing and reprioritising the current user's processes, each one bound to its pid
//! and start time, and each result read back from `/proc` rather than assumed.

use super::ProcessKey;
use super::procfs::{StatLine, control_group, parse_stat_line, user_unit};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// SIGTERM: asks the program to close, which it may answer slowly or not at all.
    Terminate,
    /// SIGKILL: the kernel ends it.
    ForceStop,
    /// SIGSTOP.
    Pause,
    /// SIGCONT.
    Resume,
}

impl Action {
    fn signal(self) -> libc::c_int {
        match self {
            Self::Terminate => libc::SIGTERM,
            Self::ForceStop => libc::SIGKILL,
            Self::Pause => libc::SIGSTOP,
            Self::Resume => libc::SIGCONT,
        }
    }

    /// How long the read-back waits for the process to reach the state asked for.
    fn settle(self) -> Duration {
        match self {
            Self::Terminate | Self::ForceStop => Duration::from_millis(1500),
            Self::Pause | Self::Resume => Duration::from_millis(300),
        }
    }
}

/// Why nothing was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The process ended before the signal.
    Gone,
    /// The pid now belongs to a different process than the one that was listed.
    Replaced,
    NotYours,
    /// The system's first process.
    Init,
    /// The process asking.
    Itself,
    /// A process the asking one descends from, ending which would end the asker too.
    Ancestor,
    /// Part of the desktop session, which would take the screen or the sound with it.
    Session(String),
    /// Refused because another member of the same application is protected.
    Member {
        pid: u32,
        reason: Box<Refusal>,
    },
}

/// What a process was found doing after the signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Gone from `/proc`, or a zombie waiting for its parent to collect it.
    Ended,
    /// The signal was delivered and the process is still there.
    StillRunning,
    Paused,
    Running,
    Refused(Refusal),
    /// The kernel rejected the signal.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionReport {
    pub action: Action,
    pub outcomes: Vec<(ProcessKey, Outcome)>,
}

impl ActionReport {
    /// What is still there after asking it to end, so the interface can say so.
    pub fn still_running(&self) -> Vec<ProcessKey> {
        self.outcomes
            .iter()
            .filter(|(_, outcome)| matches!(outcome, Outcome::StillRunning))
            .map(|(key, _)| *key)
            .collect()
    }

    /// Whether every process reached what the action asked for.
    pub fn complete(&self) -> bool {
        let wanted = match self.action {
            Action::Terminate | Action::ForceStop => Outcome::Ended,
            Action::Pause => Outcome::Paused,
            Action::Resume => Outcome::Running,
        };
        !self.outcomes.is_empty() && self.outcomes.iter().all(|(_, outcome)| *outcome == wanted)
    }
}

/// Session processes that are never signalled, whoever asks: ending any of them takes the
/// display, the sound or the session itself with it. Matched on the kernel's command name.
const SESSION: [&str; 11] = [
    "kwin_wayland",
    "kwin_x11",
    "plasmashell",
    "Xwayland",
    "pipewire",
    "pipewire-pulse",
    "wireplumber",
    "kavverna-shell",
    "ksmserver",
    "systemd",
    "dbus-broker",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuWeight(u64);

impl CpuWeight {
    pub const LOW: Self = Self(10);
    pub const BELOW_NORMAL: Self = Self(50);
    pub const NORMAL: Self = Self(100);
    pub const ABOVE_NORMAL: Self = Self(1_000);
    pub const HIGH: Self = Self(10_000);

    pub fn new(value: u64) -> Option<Self> {
        (1..=10_000).contains(&value).then_some(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PriorityError {
    #[error("refused: {0:?}")]
    Refused(Refusal),
    #[error("{0} runs in no unit of this user's, so systemd has nothing to weigh")]
    NoUnit(u32),
    #[error("the user systemd manager did not answer: {0}")]
    Systemd(String),
    #[error("the change could not be verified: {0}")]
    Unverified(String),
    #[error("systemd accepted {asked} but the kernel shows {shown}")]
    KernelDisagrees { asked: u64, shown: u64 },
}

pub struct ProcessControl {
    proc_root: PathBuf,
    cgroup_root: PathBuf,
    uid: u32,
    own_pid: u32,
}

impl ProcessControl {
    pub fn for_this_session() -> Self {
        Self::in_root(
            Path::new("/proc"),
            Path::new("/sys/fs/cgroup"),
            nix::unistd::geteuid().as_raw(),
            std::process::id(),
        )
    }

    /// Every check reads under `proc_root` and every read-back under `cgroup_root`. Signals
    /// still go to real pids, so a fixture tree can test refusals and nothing else.
    pub fn in_root(proc_root: &Path, cgroup_root: &Path, uid: u32, own_pid: u32) -> Self {
        Self {
            proc_root: proc_root.to_path_buf(),
            cgroup_root: cgroup_root.to_path_buf(),
            uid,
            own_pid,
        }
    }

    fn stat(&self, pid: u32) -> Option<StatLine> {
        let text =
            std::fs::read_to_string(self.proc_root.join(pid.to_string()).join("stat")).ok()?;
        parse_stat_line(&text)
    }

    /// Whether `target` may be signalled, read now rather than when it was listed.
    pub fn check(&self, target: ProcessKey) -> Result<(), Refusal> {
        if target.pid <= 1 {
            return Err(Refusal::Init);
        }
        if target.pid == self.own_pid {
            return Err(Refusal::Itself);
        }
        let dir = self.proc_root.join(target.pid.to_string());
        let owner = std::fs::metadata(&dir).map_err(|_| Refusal::Gone)?.uid();
        let line = self.stat(target.pid).ok_or(Refusal::Gone)?;
        if line.started != target.started {
            return Err(Refusal::Replaced);
        }
        if owner != self.uid {
            return Err(Refusal::NotYours);
        }
        if SESSION.contains(&line.command.as_str()) {
            return Err(Refusal::Session(line.command));
        }
        if self.ancestors().contains(&target.pid) {
            return Err(Refusal::Ancestor);
        }
        Ok(())
    }

    fn ancestors(&self) -> Vec<u32> {
        let mut found = Vec::new();
        let mut pid = self.own_pid;
        for _ in 0..256 {
            let Some(parent) = self.stat(pid).map(|line| line.parent).filter(|parent| *parent > 0)
            else {
                break;
            };
            found.push(parent);
            pid = parent;
        }
        found
    }

    pub fn act(&self, target: ProcessKey, action: Action) -> ActionReport {
        self.act_on_application(&[target], action)
    }

    /// The whole application or nothing: one protected member refuses every member, since
    /// ending a group with its session process carved out is not what was asked. Members that
    /// ended or were replaced since the listing are reported and skipped.
    pub fn act_on_application(&self, members: &[ProcessKey], action: Action) -> ActionReport {
        let checks: Vec<(ProcessKey, Result<(), Refusal>)> =
            members.iter().map(|key| (*key, self.check(*key))).collect();
        let blocking = checks.iter().find_map(|(key, check)| match check {
            Ok(()) | Err(Refusal::Gone | Refusal::Replaced) => None,
            Err(reason) => Some((key.pid, reason.clone())),
        });
        if let Some((pid, reason)) = blocking {
            let outcomes = checks
                .into_iter()
                .map(|(key, check)| {
                    let refusal = match check {
                        Err(own) if key.pid == pid => own,
                        Err(own @ (Refusal::Gone | Refusal::Replaced)) => own,
                        _ => Refusal::Member { pid, reason: Box::new(reason.clone()) },
                    };
                    (key, Outcome::Refused(refusal))
                })
                .collect();
            return ActionReport { action, outcomes };
        }

        let mut outcomes: Vec<(ProcessKey, Option<Outcome>)> = checks
            .into_iter()
            .map(|(key, check)| match check {
                Err(reason) => (key, Some(Outcome::Refused(reason))),
                Ok(()) => (key, self.send(key, action).err()),
            })
            .collect();

        let deadline = Instant::now() + action.settle();
        loop {
            let mut waiting = false;
            for (key, outcome) in outcomes.iter_mut().filter(|(_, outcome)| outcome.is_none()) {
                let now = self.observe(*key);
                if reached(action, &now) {
                    *outcome = Some(now);
                } else {
                    waiting = true;
                }
            }
            if !waiting || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let outcomes = outcomes
            .into_iter()
            .map(|(key, outcome)| {
                let outcome = outcome.unwrap_or_else(|| match (action, self.observe(key)) {
                    (Action::Terminate | Action::ForceStop, Outcome::Running | Outcome::Paused) => {
                        Outcome::StillRunning
                    }
                    (_, seen) => seen,
                });
                (key, outcome)
            })
            .collect();
        ActionReport { action, outcomes }
    }

    /// Opens the process as a pidfd, checks its start time again through that handle's pid,
    /// then signals the handle. Once the pidfd is open the pid cannot be handed to another
    /// process under it, so the check and the signal see the same process.
    fn send(&self, key: ProcessKey, action: Action) -> Result<(), Outcome> {
        // SAFETY: pidfd_open takes a pid and flags and returns a new descriptor or -1.
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, key.pid as libc::pid_t, 0) };
        if raw < 0 {
            return Err(Outcome::Refused(Refusal::Gone));
        }
        // SAFETY: the descriptor was just returned to us and is owned by nothing else.
        let pidfd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
        match self.stat(key.pid) {
            Some(line) if line.started == key.started => {}
            Some(_) => return Err(Outcome::Refused(Refusal::Replaced)),
            None => return Err(Outcome::Refused(Refusal::Gone)),
        }
        // SAFETY: a valid pidfd, a signal number, no siginfo, no flags.
        let sent = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                pidfd.as_raw_fd(),
                action.signal(),
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if sent < 0 {
            let error = std::io::Error::last_os_error();
            return Err(match error.raw_os_error() {
                Some(libc::ESRCH) => Outcome::Ended,
                _ => Outcome::Failed(error.to_string()),
            });
        }
        Ok(())
    }

    fn observe(&self, key: ProcessKey) -> Outcome {
        match self.stat(key.pid) {
            None => Outcome::Ended,
            Some(line) if line.started != key.started => Outcome::Ended,
            Some(line) => match line.state {
                'Z' | 'X' | 'x' => Outcome::Ended,
                'T' | 't' => Outcome::Paused,
                _ => Outcome::Running,
            },
        }
    }

    /// The user unit `target` runs in and the weight the kernel holds for it now.
    pub fn cpu_weight(&self, target: ProcessKey) -> Option<(String, u64)> {
        let group = self.control_group(target.pid)?;
        let unit = user_unit(&group, self.uid)?;
        Some((unit, read_cpu_weight(&self.cgroup_root, &unit_group(&group)).ok()?))
    }

    fn control_group(&self, pid: u32) -> Option<String> {
        let text =
            std::fs::read_to_string(self.proc_root.join(pid.to_string()).join("cgroup")).ok()?;
        control_group(&text).map(str::to_owned)
    }

    /// Sets the weight on every unit the members run in, through the user's systemd so it
    /// survives the unit being reloaded, and answers with what the kernel shows afterwards for
    /// each unit rather than with what systemd accepted.
    pub fn set_cpu_weight(
        &self,
        members: &[ProcessKey],
        weight: CpuWeight,
    ) -> Result<Vec<(String, CpuWeight)>, PriorityError> {
        let mut units: Vec<(String, String)> = Vec::new();
        for key in members {
            match self.check(*key) {
                Ok(()) => {}
                Err(Refusal::Gone | Refusal::Replaced) => continue,
                Err(reason) => return Err(PriorityError::Refused(reason)),
            }
            let group = self.control_group(key.pid).ok_or(PriorityError::NoUnit(key.pid))?;
            let unit = user_unit(&group, self.uid).ok_or(PriorityError::NoUnit(key.pid))?;
            if !units.iter().any(|(known, _)| *known == unit) {
                units.push((unit, unit_group(&group)));
            }
        }
        if units.is_empty() {
            return Err(PriorityError::Refused(Refusal::Gone));
        }
        let bus = zbus::blocking::Connection::session().map_err(systemd_error)?;
        let manager = zbus::blocking::Proxy::new(
            &bus,
            "org.freedesktop.systemd1",
            "/org/freedesktop/systemd1",
            "org.freedesktop.systemd1.Manager",
        )
        .map_err(systemd_error)?;
        let mut applied = Vec::new();
        for (unit, group) in units {
            let properties = vec![("CPUWeight", zbus::zvariant::Value::from(weight.get()))];
            manager
                .call::<_, _, ()>("SetUnitProperties", &(unit.as_str(), true, properties))
                .map_err(systemd_error)?;
            let shown = read_cpu_weight(&self.cgroup_root, &group)?;
            if shown != weight.get() {
                return Err(PriorityError::KernelDisagrees { asked: weight.get(), shown });
            }
            applied.push((unit, weight));
        }
        Ok(applied)
    }
}

fn reached(action: Action, now: &Outcome) -> bool {
    match action {
        Action::Terminate | Action::ForceStop => *now == Outcome::Ended,
        Action::Pause => matches!(now, Outcome::Paused | Outcome::Ended),
        Action::Resume => matches!(now, Outcome::Running | Outcome::Ended),
    }
}

fn systemd_error(error: impl std::fmt::Display) -> PriorityError {
    PriorityError::Systemd(error.to_string())
}

/// A process can sit in a child cgroup of its unit, and the weight lives on the unit's own.
fn unit_group(group: &str) -> String {
    let mut path = String::new();
    for part in group.split('/').filter(|part| !part.is_empty()) {
        path.push('/');
        path.push_str(part);
        if (part.ends_with(".scope") || part.ends_with(".service")) && !part.starts_with("user@") {
            break;
        }
    }
    path
}

fn read_cpu_weight(cgroup_root: &Path, group: &str) -> Result<u64, PriorityError> {
    let path = cgroup_root.join(group.trim_start_matches('/')).join("cpu.weight");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|contents| contents.trim().parse().ok())
        .ok_or_else(|| PriorityError::Unverified(format!("{} is not readable", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processes::fixture::{ProcTree, own_uid};

    const ME: u32 = 900;

    /// A session: init, the user manager, a shell that started the asker, and the asker.
    fn session() -> ProcTree {
        let tree = ProcTree::new();
        tree.process(1, "systemd", 0, 0, 1);
        tree.process(500, "plasmashell", 1, 0, 50);
        tree.process(800, "zsh", 1, 0, 80);
        tree.process(ME, "kavverna-shell", 800, 0, 90);
        tree.process(1200, "game", 1, 0, 120);
        tree.process(1201, "game-helper", 1200, 0, 121);
        tree
    }

    fn control(tree: &ProcTree) -> ProcessControl {
        ProcessControl::in_root(tree.root(), tree.root(), own_uid(), ME)
    }

    fn key(pid: u32, started: u64) -> ProcessKey {
        ProcessKey { pid, started }
    }

    #[test]
    fn the_session_the_asker_and_its_ancestors_are_refused() {
        let tree = session();
        let control = control(&tree);
        assert_eq!(control.check(key(1, 1)), Err(Refusal::Init));
        assert_eq!(control.check(key(ME, 90)), Err(Refusal::Itself));
        assert_eq!(control.check(key(800, 80)), Err(Refusal::Ancestor));
        assert_eq!(control.check(key(500, 50)), Err(Refusal::Session("plasmashell".into())));
        assert_eq!(control.check(key(1200, 120)), Ok(()));
    }

    #[test]
    fn a_reused_pid_or_an_ended_process_is_not_signalled() {
        let tree = session();
        let control = control(&tree);
        assert_eq!(control.check(key(1200, 119)), Err(Refusal::Replaced));
        assert_eq!(control.check(key(4242, 1)), Err(Refusal::Gone));
    }

    #[test]
    fn another_users_process_is_refused() {
        let tree = session();
        let control = ProcessControl::in_root(tree.root(), tree.root(), own_uid() + 1, ME);
        assert_eq!(control.check(key(1200, 120)), Err(Refusal::NotYours));
    }

    #[test]
    fn one_protected_member_refuses_the_whole_application() {
        let tree = session();
        tree.process(1202, "plasmashell", 1200, 0, 122);
        let report = control(&tree).act_on_application(
            &[key(1200, 120), key(1201, 121), key(1202, 122), key(1203, 5)],
            Action::ForceStop,
        );
        let blocked =
            Refusal::Member { pid: 1202, reason: Box::new(Refusal::Session("plasmashell".into())) };
        assert_eq!(report.outcomes[0].1, Outcome::Refused(blocked.clone()));
        assert_eq!(report.outcomes[1].1, Outcome::Refused(blocked));
        assert_eq!(report.outcomes[2].1, Outcome::Refused(Refusal::Session("plasmashell".into())));
        assert_eq!(report.outcomes[3].1, Outcome::Refused(Refusal::Gone));
        assert!(!report.complete());
    }

    /// A real child, signalled through the real `/proc`: nothing about the machine is read
    /// but the process this test started.
    #[test]
    fn a_child_is_paused_resumed_and_ended_and_each_is_read_back() {
        let mut child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        let control = ProcessControl::for_this_session();
        let started = control.stat(child.id()).unwrap().started;
        let target = key(child.id(), started);

        let paused = control.act(target, Action::Pause);
        assert_eq!(paused.outcomes, vec![(target, Outcome::Paused)]);
        let resumed = control.act(target, Action::Resume);
        assert_eq!(resumed.outcomes, vec![(target, Outcome::Running)]);
        assert!(resumed.complete());

        let stale = control.act(key(child.id(), started + 1), Action::ForceStop);
        assert_eq!(stale.outcomes[0].1, Outcome::Refused(Refusal::Replaced));

        let ended = control.act(target, Action::Terminate);
        assert_eq!(ended.outcomes, vec![(target, Outcome::Ended)], "a zombie counts as ended");
        assert!(ended.still_running().is_empty());
        child.wait().unwrap();
    }

    #[test]
    fn a_process_ignoring_terminate_is_reported_as_still_running() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "trap '' TERM; sleep 30"])
            .spawn()
            .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let control = ProcessControl::for_this_session();
        let target = key(child.id(), control.stat(child.id()).unwrap().started);
        let report = control.act(target, Action::Terminate);
        assert_eq!(report.still_running(), vec![target], "{report:?}");
        let forced = control.act(target, Action::ForceStop);
        assert!(forced.complete());
        child.wait().unwrap();
    }

    #[test]
    fn the_weight_is_read_from_the_units_own_cgroup() {
        let tree = ProcTree::new();
        let uid = own_uid();
        let group = format!("user.slice/user-{uid}.slice/user@{uid}.service/app.slice/app-x.scope");
        tree.file(&format!("{group}/cpu.weight"), "1000\n");
        tree.process(1200, "game", 1, 0, 120);
        tree.cgroup(1200, uid, "app.slice/app-x.scope/inner");
        let control = control(&tree);
        assert_eq!(control.cpu_weight(key(1200, 120)), Some(("app-x.scope".into(), 1000)));
        assert_eq!(unit_group(&format!("/{group}/inner")), format!("/{group}"));
        assert_eq!(read_cpu_weight(tree.root(), &format!("/{group}")).unwrap(), 1000);
        assert!(matches!(
            read_cpu_weight(tree.root(), "/user.slice/missing.scope"),
            Err(PriorityError::Unverified(_))
        ));
    }

    #[test]
    fn weights_are_bounded_the_way_the_kernel_bounds_them() {
        assert!(CpuWeight::new(0).is_none());
        assert!(CpuWeight::new(10_001).is_none());
        assert_eq!(CpuWeight::new(10_000), Some(CpuWeight::HIGH));
    }
}
