//! Codex writes its allowance into the session rollout after every reply, as a `token_count`
//! event carrying `rate_limits`, and the rollout outlives the session. On a desktop install most
//! older rollouts sit compressed as `.jsonl.zst` beside the plain ones, so both forms are read.

use crate::{Window, epoch_seconds};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Folders under the Codex home that hold rollouts. Nothing else there is opened: the same home
/// keeps the sign-in.
const ROLLOUT_FOLDERS: [&str; 2] = ["sessions", "archived_sessions"];

/// A live rollout can run to tens of megabytes, mostly tool output. The allowance is written
/// after every reply, so one turns up near the end of any rollout that has one at all.
const TAIL_BUDGET: u64 = 8 << 20;
const TAIL_CHUNK: u64 = 1 << 20;

/// A bound on files opened per read, for a home where many recent rollouts carry no allowance.
const MAX_FILES: usize = 32;

#[derive(Debug, Clone, PartialEq)]
struct Reading {
    minutes: Option<u32>,
    used_percent: f64,
    resets_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq)]
struct Record {
    observed_at: SystemTime,
    readings: Vec<Reading>,
}

struct Rollout {
    path: PathBuf,
    len: u64,
    modified: SystemTime,
}

struct Known {
    len: u64,
    modified: SystemTime,
    record: Option<Record>,
}

#[derive(Default)]
pub(crate) struct Rollouts {
    known: HashMap<PathBuf, Known>,
}

impl Rollouts {
    /// The newest allowance across rollouts, judged by the time written on the record rather
    /// than by which file changed last: a compressed or imported rollout can be newer on disk
    /// than what it records, and a newer session can carry `"rate_limits": null` throughout.
    pub(crate) fn latest(&mut self, codex_home: &Path, now: SystemTime) -> Option<Vec<Window>> {
        let mut rollouts = rollout_files(codex_home);
        rollouts.sort_by_key(|r| std::cmp::Reverse(r.modified));

        let mut best: Option<Record> = None;
        for rollout in rollouts.iter().take(MAX_FILES) {
            // A file last written before the best record cannot hold a newer one.
            if best.as_ref().is_some_and(|b| rollout.modified < b.observed_at) {
                break;
            }
            if let Some(record) = self.record_of(rollout)
                && best.as_ref().is_none_or(|b| record.observed_at > b.observed_at)
            {
                best = Some(record);
            }
        }

        let present: HashSet<&Path> = rollouts.iter().map(|r| r.path.as_path()).collect();
        self.known.retain(|path, _| present.contains(path.as_path()));

        let record = best?;
        let mut windows: Vec<Window> = record
            .readings
            .iter()
            .map(|r| Window::new(r.minutes, r.used_percent, r.resets_at, record.observed_at, now))
            .collect();
        windows.sort_by_key(|w| w.minutes.unwrap_or(u32::MAX));
        Some(windows)
    }

    fn record_of(&mut self, rollout: &Rollout) -> Option<Record> {
        if let Some(known) = self.known.get(&rollout.path)
            && known.len == rollout.len
            && known.modified == rollout.modified
        {
            return known.record.clone();
        }

        let read = if is_compressed(&rollout.path) {
            last_record_compressed(&rollout.path, rollout.modified)
        } else {
            last_record_plain(&rollout.path, rollout.modified)
        };
        let record = read.unwrap_or_else(|err| {
            tracing::debug!(%err, path = %rollout.path.display(), "rollout could not be read");
            None
        });
        self.known.insert(
            rollout.path.clone(),
            Known { len: rollout.len, modified: rollout.modified, record: record.clone() },
        );
        record
    }
}

fn is_compressed(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "zst")
}

fn rollout_files(codex_home: &Path) -> Vec<Rollout> {
    let mut found = Vec::new();
    let mut pending: Vec<(PathBuf, u8)> =
        ROLLOUT_FOLDERS.iter().map(|f| (codex_home.join(f), 0)).collect();

    // sessions/YYYY/MM/DD/rollout-*.jsonl, and archived_sessions holds them flat.
    while let Some((dir, depth)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() && depth < 3 {
                pending.push((entry.path(), depth + 1));
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("rollout-")
                || !(name.ends_with(".jsonl") || name.ends_with(".jsonl.zst"))
            {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(modified) = meta.modified() else { continue };
            found.push(Rollout { path: entry.path(), len: meta.len(), modified });
        }
    }
    found
}

/// Reads backwards from the end, so the cost follows how far back the last record sits rather
/// than how long the session ran. The final line may be one Codex is still writing; it fails to
/// parse and the line before it is tried.
fn last_record_plain(path: &Path, modified: SystemTime) -> io::Result<Option<Record>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let floor = len.saturating_sub(TAIL_BUDGET);

    let mut end = len;
    let mut carried: Vec<u8> = Vec::new();
    while end > floor {
        let start = end.saturating_sub(TAIL_CHUNK).max(floor);
        let mut block = vec![0; (end - start) as usize];
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut block)?;
        block.extend_from_slice(&carried);

        let mut cut = block.len();
        while let Some(newline) = block[..cut].iter().rposition(|&b| b == b'\n') {
            if let Some(record) = parse_line(&block[newline + 1..cut], modified) {
                return Ok(Some(record));
            }
            cut = newline;
        }
        block.truncate(cut);
        carried = block;
        end = start;
    }

    // What is left is the first line of the file, whole only if the budget reached the start.
    Ok(if floor == 0 { parse_line(&carried, modified) } else { None })
}

/// A compressed rollout cannot be entered from the end, so it is decoded in full. The meter
/// remembers the answer until the file's size or modification time changes.
fn last_record_compressed(path: &Path, modified: SystemTime) -> io::Result<Option<Record>> {
    let file = BufReader::new(File::open(path)?);
    let decoder = ruzstd::decoding::StreamingDecoder::new(file).map_err(io::Error::other)?;
    let mut lines = BufReader::new(decoder);

    let mut last = None;
    let mut line = Vec::new();
    loop {
        line.clear();
        match lines.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(record) = parse_line(&line, modified) {
                    last = Some(record);
                }
            }
            // A damaged frame still leaves whatever came before it.
            Err(err) => {
                tracing::debug!(%err, path = %path.display(), "compressed rollout ends early");
                break;
            }
        }
    }
    Ok(last)
}

const MARKER: &[u8] = b"\"rate_limits\"";

fn parse_line(line: &[u8], fallback_time: SystemTime) -> Option<Record> {
    if !line.windows(MARKER.len()).any(|w| w == MARKER) {
        return None;
    }
    let event: Value = serde_json::from_slice(line).ok()?;
    let limits = event.get("payload")?.get("rate_limits")?.as_object()?;

    // Codex reports one allowance per bucket: the main one, which older logs leave unnamed, and
    // one for each model that has its own. Only the main one is the plan's allowance.
    let bucket = limits.get("limit_id").and_then(Value::as_str).unwrap_or("");
    if !(bucket.is_empty() || bucket.eq_ignore_ascii_case("codex")) {
        return None;
    }

    let observed_at =
        event.get("timestamp").and_then(Value::as_str).and_then(parse_utc).unwrap_or(fallback_time);

    let readings: Vec<Reading> = ["primary", "secondary"]
        .iter()
        .filter_map(|slot| reading(limits.get(*slot)?, observed_at))
        .collect();
    (!readings.is_empty()).then_some(Record { observed_at, readings })
}

fn reading(window: &Value, observed_at: SystemTime) -> Option<Reading> {
    let used_percent = window.get("used_percent")?.as_f64().filter(|u| u.is_finite())?;
    let minutes =
        window.get("window_minutes").and_then(Value::as_u64).and_then(|m| u32::try_from(m).ok());
    // Older rollouts gave the time left instead of the moment of the reset.
    let resets_at =
        window.get("resets_at").and_then(Value::as_f64).and_then(epoch_seconds).or_else(|| {
            let delay = window.get("resets_in_seconds")?.as_f64().filter(|d| d.is_finite())?;
            observed_at.checked_add(Duration::from_secs_f64(delay.max(0.0)))
        });
    Some(Reading { minutes, used_percent, resets_at })
}

/// The one form Codex writes: `2026-11-04T14:23:03.725Z`. Anything else falls back to the file's
/// own modification time.
fn parse_utc(text: &str) -> Option<SystemTime> {
    let b = text.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = text.get(range)?;
        digits.bytes().all(|c| c.is_ascii_digit()).then(|| digits.parse().ok())?
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    let rest = &text[19..];
    let (fraction, zone) = match rest.strip_prefix('.') {
        Some(after) => {
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            (&after[..digits], &after[digits..])
        }
        None => ("", rest),
    };
    if zone != "Z" {
        return None;
    }
    let nanos = if fraction.is_empty() {
        0
    } else {
        let mut padded = fraction.chars().take(9).collect::<String>();
        while padded.len() < 9 {
            padded.push('0');
        }
        padded.parse::<u32>().ok()?
    };

    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second;
    let seconds = u64::try_from(seconds).ok()?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::new(seconds, nanos))
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Freshness, STALE_AFTER, WindowKind};
    use std::io::Write;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    /// Shaped like a Codex `token_count` event, with made-up values.
    fn token_count(timestamp: &str, used: f64, weekly: f64) -> String {
        format!(
            r#"{{"timestamp":"{timestamp}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":10,"output_tokens":5,"total_tokens":15}}}},"rate_limits":{{"limit_id":"codex","limit_name":null,"primary":{{"used_percent":{used},"window_minutes":300,"resets_at":1800018000}},"secondary":{{"used_percent":{weekly},"window_minutes":10080,"resets_at":1800600000}},"credits":null,"plan_type":"plus"}}}}}}"#
        )
    }

    fn no_limits(timestamp: &str) -> String {
        format!(
            r#"{{"timestamp":"{timestamp}","type":"event_msg","payload":{{"type":"token_count","info":null,"rate_limits":null}}}}"#
        )
    }

    const MESSAGE: &str = r#"{"timestamp":"2027-01-15T08:00:05.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}"#;

    struct Home {
        dir: tempfile::TempDir,
    }

    impl Home {
        fn new() -> Self {
            Self { dir: tempfile::tempdir().unwrap() }
        }

        fn codex(&self) -> PathBuf {
            self.dir.path().join(".codex")
        }

        fn rollout(
            &self,
            day: &str,
            name: &str,
            lines: &[String],
            trailing: &str,
            modified: u64,
        ) -> PathBuf {
            let folder = self.codex().join("sessions").join(day);
            std::fs::create_dir_all(&folder).unwrap();
            let path = folder.join(name);
            let mut text = lines.join("\n");
            text.push('\n');
            text.push_str(trailing);
            std::fs::write(&path, text).unwrap();
            File::options().write(true).open(&path).unwrap().set_modified(at(modified)).unwrap();
            path
        }
    }

    // 2027-01-15T08:00:00Z
    const T0: u64 = 1_800_000_000;

    #[test]
    fn parses_the_timestamp_codex_writes() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(at(0)));
        assert_eq!(parse_utc("2027-01-15T08:00:00.000Z"), Some(at(T0)));
        assert_eq!(parse_utc("2027-01-15T08:00:00.5Z"), Some(at(T0) + Duration::from_millis(500)));
        assert_eq!(parse_utc("2024-02-29T12:00:00Z"), Some(at(1_709_208_000)));
        assert_eq!(parse_utc("2027-01-15T08:00:00+01:00"), None);
        assert_eq!(parse_utc("2027-13-15T08:00:00Z"), None);
        assert_eq!(parse_utc("not a time"), None);
    }

    #[test]
    fn reads_the_last_allowance_in_a_rollout() {
        let home = Home::new();
        home.rollout(
            "2027/01/15",
            "rollout-a.jsonl",
            &[
                token_count("2027-01-15T08:00:00.000Z", 10.0, 1.0),
                token_count("2027-01-15T08:00:03.000Z", 12.0, 2.0),
                MESSAGE.to_owned(),
            ],
            "",
            T0 + 10,
        );

        let windows = Rollouts::default().latest(&home.codex(), at(T0 + 60)).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!((windows[0].kind, windows[0].used_percent), (WindowKind::Session, 12.0));
        assert_eq!((windows[1].kind, windows[1].used_percent), (WindowKind::Weekly, 2.0));
        assert_eq!(windows[0].observed_at, at(T0 + 3));
        assert_eq!(windows[0].resets_at, Some(at(1_800_018_000)));
        assert!(windows.iter().all(|w| w.state == Freshness::Fresh));
    }

    #[test]
    fn a_line_still_being_written_is_passed_over() {
        let home = Home::new();
        let whole = token_count("2027-01-15T08:00:03.000Z", 12.0, 2.0);
        let partial = &token_count("2027-01-15T08:00:09.000Z", 50.0, 5.0)[..120];
        home.rollout("2027/01/15", "rollout-a.jsonl", &[whole], partial, T0 + 10);

        let windows = Rollouts::default().latest(&home.codex(), at(T0 + 60)).unwrap();
        assert_eq!(windows[0].used_percent, 12.0);
    }

    #[test]
    fn a_session_without_an_allowance_does_not_hide_an_older_one() {
        let home = Home::new();
        home.rollout(
            "2027/01/14",
            "rollout-old.jsonl",
            &[token_count("2027-01-15T07:00:00.000Z", 30.0, 3.0)],
            "",
            T0 - 3_600,
        );
        home.rollout(
            "2027/01/15",
            "rollout-new.jsonl",
            &[no_limits("2027-01-15T08:00:00.000Z"), MESSAGE.to_owned()],
            "",
            T0 + 10,
        );

        let windows = Rollouts::default().latest(&home.codex(), at(T0 + 60)).unwrap();
        assert_eq!(windows[0].used_percent, 30.0);
        assert_eq!(windows[0].state, Freshness::Stale);
    }

    #[test]
    fn the_record_written_last_wins_over_the_file_touched_last() {
        let home = Home::new();
        home.rollout(
            "2027/01/15",
            "rollout-a.jsonl",
            &[token_count("2027-01-15T08:00:30.000Z", 40.0, 4.0)],
            "",
            T0 + 30,
        );
        // Touched later, but its last allowance is older.
        home.rollout(
            "2027/01/15",
            "rollout-b.jsonl",
            &[token_count("2027-01-15T08:00:10.000Z", 20.0, 2.0), MESSAGE.to_owned()],
            "",
            T0 + 50,
        );

        let windows = Rollouts::default().latest(&home.codex(), at(T0 + 60)).unwrap();
        assert_eq!(windows[0].used_percent, 40.0);
    }

    #[test]
    fn other_buckets_are_not_the_plan_allowance() {
        let home = Home::new();
        let model_bucket = token_count("2027-01-15T08:00:09.000Z", 99.0, 9.0)
            .replace(r#""limit_id":"codex""#, r#""limit_id":"some_model""#);
        home.rollout(
            "2027/01/15",
            "rollout-a.jsonl",
            &[token_count("2027-01-15T08:00:03.000Z", 12.0, 2.0), model_bucket],
            "",
            T0 + 10,
        );

        let windows = Rollouts::default().latest(&home.codex(), at(T0 + 60)).unwrap();
        assert_eq!(windows[0].used_percent, 12.0);
    }

    #[test]
    fn a_reset_that_has_passed_expires_the_window() {
        let home = Home::new();
        home.rollout(
            "2027/01/15",
            "rollout-a.jsonl",
            &[token_count("2027-01-15T08:00:03.000Z", 12.0, 2.0)],
            "",
            T0 + 10,
        );

        let after_session_reset = at(1_800_018_000);
        let windows = Rollouts::default().latest(&home.codex(), after_session_reset).unwrap();
        assert_eq!(windows[0].state, Freshness::Expired);
        assert_eq!(windows[1].state, Freshness::Stale);
        assert!(after_session_reset.duration_since(windows[1].observed_at).unwrap() > STALE_AFTER);
    }

    #[test]
    fn older_rollouts_give_the_time_left_instead() {
        let line = r#"{"timestamp":"2027-01-15T08:00:00Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":7.5,"window_minutes":300,"resets_in_seconds":600}}}}"#;
        let record = parse_line(line.as_bytes(), at(0)).unwrap();
        assert_eq!(record.observed_at, at(T0));
        assert_eq!(record.readings[0].resets_at, Some(at(T0 + 600)));
    }

    #[test]
    fn an_unreadable_timestamp_falls_back_to_the_file_time() {
        let line = token_count("yesterday", 12.0, 2.0);
        assert_eq!(parse_line(line.as_bytes(), at(42)).unwrap().observed_at, at(42));
    }

    #[test]
    fn missing_folders_and_empty_files_are_no_reading() {
        let home = Home::new();
        assert_eq!(Rollouts::default().latest(&home.codex(), at(T0)), None);

        std::fs::create_dir_all(home.codex().join("sessions")).unwrap();
        home.rollout("2027/01/15", "rollout-a.jsonl", &[], "", T0);
        assert_eq!(Rollouts::default().latest(&home.codex(), at(T0)), None);
    }

    #[test]
    fn finds_a_record_further_back_than_one_chunk() {
        let home = Home::new();
        let filler = MESSAGE.replace("done", &"x".repeat(TAIL_CHUNK as usize + 10));
        home.rollout(
            "2027/01/15",
            "rollout-a.jsonl",
            &[token_count("2027-01-15T08:00:03.000Z", 12.0, 2.0), filler.clone(), filler],
            "",
            T0 + 10,
        );

        let windows = Rollouts::default().latest(&home.codex(), at(T0 + 60)).unwrap();
        assert_eq!(windows[0].used_percent, 12.0);
    }

    /// A zstd frame of raw blocks, which is valid zstd that needs no encoder to produce.
    fn zstd_raw(content: &[u8]) -> Vec<u8> {
        let mut frame = vec![0x28, 0xB5, 0x2F, 0xFD];
        // Single segment, eight byte content size, no checksum, no dictionary.
        frame.push(0b1110_0000);
        frame.extend_from_slice(&(content.len() as u64).to_le_bytes());
        let chunks: Vec<&[u8]> = content.chunks(100_000).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            let last = u32::from(i + 1 == chunks.len());
            let header = last | ((chunk.len() as u32) << 3);
            frame.extend_from_slice(&header.to_le_bytes()[..3]);
            frame.extend_from_slice(chunk);
        }
        frame
    }

    #[test]
    fn reads_a_compressed_rollout_and_remembers_it() {
        let home = Home::new();
        let folder = home.codex().join("archived_sessions");
        std::fs::create_dir_all(&folder).unwrap();
        let text =
            [token_count("2027-01-15T08:00:03.000Z", 33.0, 3.0), MESSAGE.to_owned()].join("\n");
        let path = folder.join("rollout-a.jsonl.zst");
        File::create(&path).unwrap().write_all(&zstd_raw(text.as_bytes())).unwrap();
        File::options().write(true).open(&path).unwrap().set_modified(at(T0 + 10)).unwrap();

        let mut rollouts = Rollouts::default();
        let windows = rollouts.latest(&home.codex(), at(T0 + 60)).unwrap();
        assert_eq!(windows[0].used_percent, 33.0);
        assert!(rollouts.known.contains_key(&path));

        std::fs::remove_file(&path).unwrap();
        assert_eq!(rollouts.latest(&home.codex(), at(T0 + 60)), None);
        assert!(rollouts.known.is_empty());
    }

    #[test]
    fn a_damaged_compressed_rollout_is_no_reading() {
        let home = Home::new();
        let folder = home.codex().join("archived_sessions");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("rollout-a.jsonl.zst"), b"not zstd at all").unwrap();
        assert_eq!(Rollouts::default().latest(&home.codex(), at(T0)), None);
    }
}
