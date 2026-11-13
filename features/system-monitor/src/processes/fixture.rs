//! A `/proc` written by hand, so every walk is tested against a tree the test controls rather
//! than against whatever the machine running it happens to be doing.

use std::path::{Path, PathBuf};

pub fn stat_text(pid: u32, command: &str, parent: u32, ticks: u64, started: u64) -> String {
    stat_with_state(pid, command, 'S', parent, ticks, started)
}

pub fn stat_with_state(
    pid: u32,
    command: &str,
    state: char,
    parent: u32,
    ticks: u64,
    started: u64,
) -> String {
    format!(
        "{pid} ({command}) {state} {parent} {pid} {pid} 0 -1 4194304 10 0 0 0 {ticks} 0 0 0 20 0 1 \
         0 {started} 1000 100 18446744073709551615"
    )
}

pub struct ProcTree {
    room: tempfile::TempDir,
}

impl ProcTree {
    pub fn new() -> Self {
        let tree = Self { room: tempfile::tempdir().unwrap() };
        tree.machine(0, 0);
        tree
    }

    pub fn root(&self) -> &Path {
        self.room.path()
    }

    /// `/proc/stat` with two cores, `busy` and `idle` split evenly between them.
    pub fn machine(&self, busy: u64, idle: u64) {
        std::fs::write(
            self.root().join("stat"),
            format!(
                "cpu  {busy} 0 0 {idle} 0 0 0 0 0 0\ncpu0 {} 0 0 {} 0 0 0 0 0 0\n\
                 cpu1 {} 0 0 {} 0 0 0 0 0 0\nintr 0\n",
                busy / 2,
                idle / 2,
                busy - busy / 2,
                idle - idle / 2
            ),
        )
        .unwrap();
    }

    pub fn dir(&self, pid: u32) -> PathBuf {
        self.root().join(pid.to_string())
    }

    pub fn process(&self, pid: u32, command: &str, parent: u32, ticks: u64, started: u64) {
        let dir = self.dir(pid);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("stat"), stat_text(pid, command, parent, ticks, started)).unwrap();
        if !dir.join("statm").exists() {
            self.pages(pid, 256, 64);
        }
    }

    pub fn pages(&self, pid: u32, resident: u64, shared: u64) {
        std::fs::write(self.dir(pid).join("statm"), format!("9999 {resident} {shared} 1 0 9 0\n"))
            .unwrap();
    }

    pub fn pss_kib(&self, pid: u32, kib: u64) {
        std::fs::write(
            self.dir(pid).join("smaps_rollup"),
            format!("0-1 ---p 00000000 00:00 0 [rollup]\nRss: {} kB\nPss: {kib} kB\n", kib * 2),
        )
        .unwrap();
    }

    pub fn io(&self, pid: u32, read: u64, written: u64) {
        std::fs::write(
            self.dir(pid).join("io"),
            format!("rchar: 1\nwchar: 1\nread_bytes: {read}\nwrite_bytes: {written}\n"),
        )
        .unwrap();
    }

    pub fn cgroup(&self, pid: u32, uid: u32, unit_path: &str) {
        std::fs::write(
            self.dir(pid).join("cgroup"),
            format!("0::/user.slice/user-{uid}.slice/user@{uid}.service/{unit_path}\n"),
        )
        .unwrap();
    }

    pub fn exe(&self, pid: u32, target: &str) {
        std::os::unix::fs::symlink(target, self.dir(pid).join("exe")).unwrap();
    }

    pub fn descriptor(&self, pid: u32, fd: u32, target: &str, info: &str) {
        let dir = self.dir(pid);
        std::fs::create_dir_all(dir.join("fd")).unwrap();
        std::fs::create_dir_all(dir.join("fdinfo")).unwrap();
        let link = dir.join("fd").join(fd.to_string());
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(target, link).unwrap();
        std::fs::write(dir.join("fdinfo").join(fd.to_string()), info).unwrap();
    }

    pub fn file(&self, relative: &str, contents: &str) {
        let path = self.root().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
}

/// The uid that owns every directory the fixture writes.
pub fn own_uid() -> u32 {
    nix::unistd::geteuid().as_raw()
}
