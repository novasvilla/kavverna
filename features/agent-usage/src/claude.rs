//! Claude Code keeps no file with its allowance. It hands the figures to the status line command
//! on stdin while a session renders its status line (Pro and Max plans, after the first reply),
//! so whatever runs as that command is the only witness. Kavverna's passthrough records them in
//! its own cache; a person who keeps another status line tool can still be read through the copy
//! that tool leaves behind, and the meter takes whichever of the two saw the allowance last.

use crate::{FIVE_HOUR_MINUTES, SEVEN_DAY_MINUTES, Source, Window, epoch_seconds, private_file};
use serde_json::{Value, json};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::time::SystemTime;

pub const CACHE_FILE: &str = "claude-rate-limits.json";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusLineWindow {
    pub used_percent: f64,
    pub resets_at: Option<SystemTime>,
}

/// Either window can be missing on its own: a document has been seen carrying only the weekly
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StatusLineLimits {
    pub five_hour: Option<StatusLineWindow>,
    pub seven_day: Option<StatusLineWindow>,
}

/// `None` when the document is not JSON or carries no allowance, which is what API key sessions
/// and a session before its first reply send.
pub fn parse_status_line(document: &[u8]) -> Option<StatusLineLimits> {
    let root: Value = serde_json::from_slice(document).ok()?;
    let limits = root.get("rate_limits")?;
    let parsed = StatusLineLimits {
        five_hour: limits.get("five_hour").and_then(window),
        seven_day: limits.get("seven_day").and_then(window),
    };
    (parsed.five_hour.is_some() || parsed.seven_day.is_some()).then_some(parsed)
}

fn window(value: &Value) -> Option<StatusLineWindow> {
    let used = value.get("used_percentage")?.as_f64().filter(|u| u.is_finite())?;
    let resets_at = value.get("resets_at").and_then(Value::as_f64).and_then(epoch_seconds);
    Some(StatusLineWindow { used_percent: used, resets_at })
}

/// Records the allowance from one status line document and reports whether it wrote anything.
///
/// Only the two windows and the moment of the call are kept. The rest of the document names the
/// session, its folder and its transcript, none of which the meter needs. A document without an
/// allowance leaves the previous record alone, so one API key session cannot erase what a
/// subscription session saw.
pub fn record_status_line(document: &[u8], cache_dir: &Path, now: SystemTime) -> io::Result<bool> {
    let Some(limits) = parse_status_line(document) else {
        return Ok(false);
    };
    let observed = now.duration_since(SystemTime::UNIX_EPOCH).map_err(io::Error::other)?;

    let mut record = json!({ "observed_at": observed.as_secs_f64() });
    for (key, window) in [("five_hour", limits.five_hour), ("seven_day", limits.seven_day)] {
        if let Some(window) = window {
            record[key] = json!({
                "used_percentage": window.used_percent,
                "resets_at": window.resets_at.and_then(|r| r.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs()),
            });
        }
    }

    let bytes = serde_json::to_vec(&record).map_err(io::Error::other)?;
    private_file::write(&cache_dir.join(CACHE_FILE), &bytes)?;
    Ok(true)
}

/// The `claude-statusbar` tool (run as `cs render`) writes the whole document it was handed to
/// this file, under the user's cache home, on every render.
///
/// It is another tool's file, not a contract: its name, place and shape can change with any
/// release of that tool. It is only ever read, never written or removed, and a missing,
/// truncated or malformed file simply means no reading from it. Only the two windows are taken;
/// the rest of the document names the session, its folder and its model, and none of that is
/// kept or logged.
pub const STATUSBAR_FILE: &str = "claude-statusbar/last_stdin.json";

pub(crate) struct Reading {
    pub source: Source,
    pub observed_at: SystemTime,
    pub limits: StatusLineLimits,
}

impl Reading {
    pub fn windows(&self, now: SystemTime) -> Vec<Window> {
        [(FIVE_HOUR_MINUTES, self.limits.five_hour), (SEVEN_DAY_MINUTES, self.limits.seven_day)]
            .into_iter()
            .filter_map(|(minutes, found)| {
                let found = found?;
                Some(Window::new(
                    Some(minutes),
                    found.used_percent,
                    found.resets_at,
                    self.observed_at,
                    now,
                ))
            })
            .collect()
    }
}

/// The newer of the two sources, taken whole. Windows are never mixed across sources: an older
/// five hour figure beside a newer weekly one would describe a moment nobody saw. A tie goes to
/// Kavverna's own record.
pub(crate) fn latest(cache_dir: &Path, cache_home: &Path) -> Option<Reading> {
    match (read_cache(cache_dir), read_statusbar(cache_home)) {
        (Some(own), Some(other)) if other.observed_at > own.observed_at => Some(other),
        (Some(own), _) => Some(own),
        (None, other) => other,
    }
}

fn read_cache(cache_dir: &Path) -> Option<Reading> {
    let bytes = std::fs::read(cache_dir.join(CACHE_FILE)).ok()?;
    let record: Value = serde_json::from_slice(&bytes).ok()?;
    let observed_at = record.get("observed_at").and_then(Value::as_f64).and_then(epoch_seconds)?;
    let limits = StatusLineLimits {
        five_hour: record.get("five_hour").and_then(window),
        seven_day: record.get("seven_day").and_then(window),
    };
    (limits.five_hour.is_some() || limits.seven_day.is_some()).then_some(Reading {
        source: Source::Kavverna,
        observed_at,
        limits,
    })
}

/// The file is rewritten on every render, so its modification time is when the allowance was
/// seen. Contents and time come from one open handle, so they describe the same file even if the
/// tool replaces it in between.
fn read_statusbar(cache_home: &Path) -> Option<Reading> {
    let mut file = File::open(cache_home.join(STATUSBAR_FILE)).ok()?;
    let observed_at = file.metadata().ok()?.modified().ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let limits = parse_status_line(&bytes)?;
    Some(Reading { source: Source::ClaudeStatusbar, observed_at, limits })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Freshness, STALE_AFTER, WindowKind};
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn read_back(cache_dir: &Path, now: SystemTime) -> Option<Vec<Window>> {
        read_cache(cache_dir).map(|reading| reading.windows(now))
    }

    /// Places a statusbar copy under `cache_home` and stamps it, since the stamp is the reading's
    /// time and a real one would always lose to the fixtures' dates.
    fn write_statusbar(cache_home: &Path, document: &[u8], modified: SystemTime) {
        let path = cache_home.join(STATUSBAR_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, document).unwrap();
        File::options().write(true).open(&path).unwrap().set_modified(modified).unwrap();
    }

    /// Shaped like the documented status line input, with made-up values.
    const BOTH: &str = r#"{
        "session_id": "00000000-0000-0000-0000-000000000000",
        "cwd": "/tmp/example",
        "transcript_path": "/tmp/example/transcript.jsonl",
        "model": {"id": "example-model", "display_name": "Example"},
        "rate_limits": {
            "five_hour": {"used_percentage": 23.5, "resets_at": 1800000000},
            "seven_day": {"used_percentage": 41, "resets_at": 1800300000}
        }
    }"#;

    const WEEKLY_ONLY: &str =
        r#"{"rate_limits": {"seven_day": {"used_percentage": 65, "resets_at": 1800300000}}}"#;

    #[test]
    fn parses_both_windows() {
        let limits = parse_status_line(BOTH.as_bytes()).unwrap();
        assert_eq!(
            limits.five_hour,
            Some(StatusLineWindow { used_percent: 23.5, resets_at: Some(at(1_800_000_000)) })
        );
        assert_eq!(limits.seven_day.unwrap().used_percent, 41.0);
    }

    #[test]
    fn a_window_can_be_missing_on_its_own() {
        let limits = parse_status_line(WEEKLY_ONLY.as_bytes()).unwrap();
        assert_eq!(limits.five_hour, None);
        assert_eq!(limits.seven_day.unwrap().resets_at, Some(at(1_800_300_000)));
    }

    #[test]
    fn documents_without_an_allowance_are_not_readings() {
        assert_eq!(parse_status_line(br#"{"model": {"id": "x"}}"#), None);
        assert_eq!(parse_status_line(br#"{"rate_limits": null}"#), None);
        assert_eq!(parse_status_line(br#"{"rate_limits": {}}"#), None);
        assert_eq!(parse_status_line(b"{\"rate_limits\": {\"five_h"), None);
    }

    #[test]
    fn records_only_the_windows_privately() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("kavverna");
        assert!(record_status_line(BOTH.as_bytes(), &cache, at(1_799_990_000)).unwrap());

        let file = cache.join(CACHE_FILE);
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&cache), 0o700);
        assert_eq!(mode(&file), 0o600);

        let written = std::fs::read_to_string(&file).unwrap();
        for private in ["session_id", "cwd", "transcript_path", "model"] {
            assert!(!written.contains(private), "{private} leaked into the cache");
        }
        let leftovers: Vec<_> = std::fs::read_dir(&cache).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "staging file left behind");
    }

    #[test]
    fn a_document_without_an_allowance_keeps_the_previous_record() {
        let cache = tempfile::tempdir().unwrap();
        record_status_line(BOTH.as_bytes(), cache.path(), at(1_799_990_000)).unwrap();
        assert!(
            !record_status_line(br#"{"cwd": "/tmp"}"#, cache.path(), at(1_799_990_100)).unwrap()
        );

        let windows = read_back(cache.path(), at(1_799_990_000)).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].observed_at, at(1_799_990_000));
    }

    #[test]
    fn reads_back_with_age_and_expiry() {
        let cache = tempfile::tempdir().unwrap();
        let observed = at(1_799_990_000);
        record_status_line(BOTH.as_bytes(), cache.path(), observed).unwrap();

        let windows = read_back(cache.path(), observed + Duration::from_secs(60)).unwrap();
        assert_eq!(windows[0].kind, WindowKind::Session);
        assert_eq!(windows[1].kind, WindowKind::Weekly);
        assert!(windows.iter().all(|w| w.state == Freshness::Fresh));

        let later = observed + STALE_AFTER + Duration::from_secs(1);
        assert!(
            read_back(cache.path(), later).unwrap().iter().all(|w| w.state == Freshness::Stale)
        );

        // Past the five hour reset but before the weekly one.
        let windows = read_back(cache.path(), at(1_800_000_001)).unwrap();
        assert_eq!(windows[0].state, Freshness::Expired);
        assert_eq!(windows[1].state, Freshness::Stale);
    }

    #[test]
    fn a_missing_or_damaged_record_is_no_reading() {
        let cache = tempfile::tempdir().unwrap();
        assert_eq!(read_back(cache.path(), at(1)), None);
        std::fs::write(cache.path().join(CACHE_FILE), b"{\"observed_at\": 1, \"five").unwrap();
        assert_eq!(read_back(cache.path(), at(1)), None);
    }

    /// Kavverna's record at one moment and the statusbar copy at another, in separate roots as
    /// they are on a real machine.
    fn both_sources(own_at: u64, statusbar_at: u64) -> (tempfile::TempDir, tempfile::TempDir) {
        let cache = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        record_status_line(BOTH.as_bytes(), cache.path(), at(own_at)).unwrap();
        write_statusbar(home.path(), WEEKLY_ONLY.as_bytes(), at(statusbar_at));
        (cache, home)
    }

    #[test]
    fn a_newer_statusbar_copy_wins_whole() {
        let (cache, home) = both_sources(1_799_990_000, 1_799_990_060);
        let reading = latest(cache.path(), home.path()).unwrap();
        assert_eq!(reading.source, Source::ClaudeStatusbar);
        assert_eq!(reading.observed_at, at(1_799_990_060));
        // The older five hour figure is not carried over beside the newer weekly one.
        assert_eq!(reading.limits.five_hour, None);
        assert_eq!(reading.limits.seven_day.unwrap().used_percent, 65.0);
    }

    #[test]
    fn a_newer_own_record_wins() {
        let (cache, home) = both_sources(1_799_990_060, 1_799_990_000);
        let reading = latest(cache.path(), home.path()).unwrap();
        assert_eq!(reading.source, Source::Kavverna);
        assert_eq!(reading.observed_at, at(1_799_990_060));
        assert_eq!(reading.limits.five_hour.unwrap().used_percent, 23.5);
    }

    #[test]
    fn a_tie_goes_to_the_own_record() {
        let (cache, home) = both_sources(1_799_990_000, 1_799_990_000);
        assert_eq!(latest(cache.path(), home.path()).unwrap().source, Source::Kavverna);
    }

    #[test]
    fn the_statusbar_copy_alone_is_read_with_its_modification_time() {
        let cache = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        write_statusbar(home.path(), BOTH.as_bytes(), at(1_799_990_000));

        let reading = latest(cache.path(), home.path()).unwrap();
        assert_eq!(reading.source, Source::ClaudeStatusbar);
        let windows = reading.windows(at(1_799_990_030));
        assert_eq!(windows.len(), 2);
        assert!(windows.iter().all(|w| w.observed_at == at(1_799_990_000)));
        assert!(windows.iter().all(|w| w.state == Freshness::Fresh));
    }

    #[test]
    fn a_damaged_statusbar_copy_is_no_reading() {
        let cache = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        assert!(latest(cache.path(), home.path()).is_none());

        for damaged in [&b"{\"rate_limits\": {\"seven_day\": {\"used_perc"[..], b"", b"not json"] {
            write_statusbar(home.path(), damaged, at(1_799_990_000));
            assert!(latest(cache.path(), home.path()).is_none());
        }
        write_statusbar(home.path(), br#"{"session_id": "x"}"#, at(1_799_990_000));
        assert!(latest(cache.path(), home.path()).is_none());
    }

    #[test]
    fn a_damaged_statusbar_copy_leaves_the_own_record_standing() {
        let (cache, home) = both_sources(1_799_990_000, 1_799_990_000);
        write_statusbar(home.path(), b"{\"rate_limits\": {", at(1_799_999_999));
        assert_eq!(latest(cache.path(), home.path()).unwrap().source, Source::Kavverna);
    }

    #[test]
    fn the_statusbar_copy_is_only_read() {
        let (cache, home) = both_sources(1_799_990_000, 1_799_990_060);
        let path = home.path().join(STATUSBAR_FILE);
        let before = (std::fs::read(&path).unwrap(), std::fs::metadata(&path).unwrap().modified());
        latest(cache.path(), home.path()).unwrap();
        let after = (std::fs::read(&path).unwrap(), std::fs::metadata(&path).unwrap().modified());
        assert_eq!(before.0, after.0);
        assert_eq!(before.1.unwrap(), after.1.unwrap());
    }
}
