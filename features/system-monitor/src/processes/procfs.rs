//! The few `/proc/<pid>` files a sample reads, parsed without allocating more than the name.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Set in `/proc/<pid>/stat` flags for a kernel thread, which owns no memory and no program.
const PF_KTHREAD: u32 = 0x0020_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatLine {
    pub command: String,
    pub state: char,
    pub parent: u32,
    pub kernel_thread: bool,
    pub cpu_ticks: u64,
    /// Clock ticks after boot. With the pid it names one process for its whole life, which
    /// is what keeps a reused pid from inheriting the history or the signals of the old one.
    pub started: u64,
}

/// The command sits in parentheses and may itself hold spaces and parentheses, so the fields
/// are counted from the last closing one.
pub(crate) fn parse_stat_line(contents: &str) -> Option<StatLine> {
    let open = contents.find('(')?;
    let close = contents.rfind(')')?;
    let command = contents.get(open + 1..close)?.to_owned();
    let mut fields = contents.get(close + 1..)?.split_ascii_whitespace();
    // Counting from the state, which is field 3 of the documented layout.
    let state = fields.next()?.chars().next()?;
    let parent = fields.next()?.parse().ok()?;
    let mut fields = fields.skip(4);
    let flags: u32 = fields.next()?.parse().ok()?;
    let mut fields = fields.skip(4);
    let user: u64 = fields.next()?.parse().ok()?;
    let system: u64 = fields.next()?.parse().ok()?;
    let started = fields.nth(6)?.parse().ok()?;
    Some(StatLine {
        command,
        state,
        parent,
        kernel_thread: flags & PF_KTHREAD != 0,
        cpu_ticks: user.saturating_add(system),
        started,
    })
}

/// Resident pages and the file-backed or shared part of them, the second and third numbers of
/// `statm`. The kernel keeps both as counters, so this file costs nothing like `smaps` does.
pub(crate) fn parse_statm(contents: &str) -> Option<(u64, u64)> {
    let mut fields = contents.split_ascii_whitespace().skip(1);
    let resident = fields.next()?.parse().ok()?;
    let shared = fields.next()?.parse().ok()?;
    Some((resident, shared))
}

/// Bytes that reached or left the storage layer, `read_bytes` and `write_bytes` of
/// `/proc/<pid>/io`. `rchar` and `wchar` would count pipes, sockets and the page cache too.
pub(crate) fn parse_io(contents: &str) -> Option<(u64, u64)> {
    let field = |name: &str| {
        contents.lines().find_map(|line| line.strip_prefix(name)?.trim().parse::<u64>().ok())
    };
    Some((field("read_bytes:")?, field("write_bytes:")?))
}

/// Proportional set size in bytes: each shared page divided among the processes mapping it,
/// so the figures of a group of processes add up instead of counting a library once each.
pub(crate) fn parse_pss(contents: &str) -> Option<u64> {
    let kib: u64 = contents
        .lines()
        .find_map(|line| line.strip_prefix("Pss:"))?
        .split_ascii_whitespace()
        .next()?
        .parse()
        .ok()?;
    kib.checked_mul(1024)
}

/// Reads into a buffer the caller keeps, so a walk over hundreds of processes reuses one
/// allocation instead of making one per file.
pub(crate) fn read_into(path: &Path, buffer: &mut String) -> bool {
    buffer.clear();
    File::open(path).and_then(|mut file| file.read_to_string(buffer)).is_ok()
}

/// The cgroup v2 path, the line that starts `0::`.
pub(crate) fn control_group(contents: &str) -> Option<&str> {
    contents.lines().find_map(|line| line.strip_prefix("0::"))
}

/// The unit a process runs in, only when it is one of this user's own, since those are the
/// ones the user manager will take a CPUWeight for. `init.scope` is the manager itself.
pub(crate) fn user_unit(control_group: &str, uid: u32) -> Option<String> {
    let inside =
        control_group.strip_prefix(&format!("/user.slice/user-{uid}.slice/user@{uid}.service/"))?;
    inside
        .rsplit('/')
        .find(|part| part.ends_with(".scope") || part.ends_with(".service"))
        .filter(|unit| *unit != "init.scope")
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processes::fixture::stat_text;

    #[test]
    fn io_and_pss_are_read_in_bytes() {
        let io = "rchar: 900\nwchar: 800\nsyscr: 1\nsyscw: 1\nread_bytes: 4096\n\
                  write_bytes: 8192\ncancelled_write_bytes: 0\n";
        assert_eq!(parse_io(io), Some((4096, 8192)));
        let rollup = "55d0-7ffd ---p 00000000 00:00 0 [rollup]\nRss: 2048 kB\nPss: 1024 kB\n\
                      Pss_Anon: 512 kB\n";
        assert_eq!(parse_pss(rollup), Some(1024 * 1024));
        assert_eq!(parse_pss(""), None);
    }

    #[test]
    fn stat_survives_spaces_and_parentheses_in_the_name() {
        let line = parse_stat_line(&stat_text(42, "name with ) a bracket", 7, 50, 900)).unwrap();
        assert_eq!(line.command, "name with ) a bracket");
        assert_eq!(line.parent, 7);
        assert_eq!(line.cpu_ticks, 50);
        assert_eq!(line.started, 900);
        assert_eq!(line.state, 'S');
        assert!(!line.kernel_thread);
    }

    #[test]
    fn a_real_stat_line_is_read_field_for_field() {
        // Taken from a running shell: utime 3, stime 1, starttime 21474.
        let line = parse_stat_line(
            "1813 (plasmashell) S 1510 1813 1813 0 -1 4194560 371155 4108 2 0 3 1 7 3 20 0 33 0 \
             21474 4176515072 112374 18446744073709551615 1 1 0 0 0 0 0 4096 81920 0 0 0 17 9 0 0",
        )
        .unwrap();
        assert_eq!(line.parent, 1510);
        assert_eq!(line.cpu_ticks, 4);
        assert_eq!(line.started, 21474);
    }

    #[test]
    fn a_kernel_thread_is_recognised_by_its_flag() {
        let line =
            parse_stat_line("2 (kthreadd) S 0 0 0 0 -1 2129984 0 0 0 0 0 0 0 0 20 0 1 0 2 0 0")
                .unwrap();
        assert!(line.kernel_thread);
    }

    #[test]
    fn statm_gives_resident_and_shared_pages() {
        assert_eq!(parse_statm("1019798 112374 31560 1 0 140213 0\n"), Some((112374, 31560)));
        assert_eq!(parse_statm(""), None);
    }

    #[test]
    fn the_owning_user_unit_is_found_and_nothing_else() {
        let base = "/user.slice/user-1000.slice/user@1000.service/";
        assert_eq!(
            user_unit(&format!("{base}app.slice/app-org.telegram.desktop-10.scope"), 1000)
                .as_deref(),
            Some("app-org.telegram.desktop-10.scope")
        );
        assert_eq!(
            user_unit(&format!("{base}app.slice/app-firefox@8e8c.service/child"), 1000).as_deref(),
            Some("app-firefox@8e8c.service")
        );
        assert_eq!(user_unit(&format!("{base}init.scope"), 1000), None);
        assert_eq!(user_unit(&format!("{base}app.slice/app-x.scope"), 1001), None);
        assert_eq!(user_unit("/system.slice/sshd.service", 1000), None);
        assert_eq!(user_unit("/user.slice/user-1000.slice/session-2.scope", 1000), None);
        assert_eq!(
            control_group("1:name=systemd:/x\n0::/user.slice/a.scope\n"),
            Some("/user.slice/a.scope")
        );
    }
}
