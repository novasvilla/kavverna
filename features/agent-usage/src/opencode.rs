//! OpenCode records no allowance: it talks to whichever provider is configured, often a local
//! model, and keeps only what each reply cost in tokens and, where it knows a price, dollars.
//! So it is reported as totals, never as a share of anything.

use crate::TokenTotals;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The span OpenCode totals cover, ending at the moment of the reading.
pub const TOTALS_SPAN: Duration = Duration::from_secs(24 * 60 * 60);

/// Under the XDG data home rather than a fixed folder in home, since OpenCode follows it.
pub(crate) fn database(data_dir: &Path) -> PathBuf {
    data_dir.join("opencode/opencode.db")
}

/// Only the role, token counts and cost are taken out of each message. Prompts and replies live
/// in other tables and are never queried, and neither are the account and credential tables
/// that share this database. `json_valid` keeps one damaged row from failing the whole sum.
const TOTALS: &str = "
    SELECT count(*),
           total(json_extract(data, '$.tokens.input')),
           total(json_extract(data, '$.tokens.output')),
           total(json_extract(data, '$.tokens.reasoning')),
           total(json_extract(data, '$.tokens.cache.read')),
           total(json_extract(data, '$.tokens.cache.write')),
           total(json_extract(data, '$.cost'))
    FROM message
    WHERE time_created >= ?1
      AND json_valid(data)
      AND json_extract(data, '$.role') = 'assistant'";

/// `None` when the database cannot be opened or is not laid out as OpenCode 1 lays it out.
pub(crate) fn totals(database: &Path, now: SystemTime) -> Option<TokenTotals> {
    let since = now.checked_sub(TOTALS_SPAN)?;
    let since_ms = since.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_millis() as i64;

    let read = || -> rusqlite::Result<TokenTotals> {
        let connection = Connection::open_with_flags(
            database,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(Duration::from_secs(1))?;
        connection.query_row(TOTALS, [since_ms], |row| {
            let count = |i| row.get::<_, f64>(i).map(|v| v.max(0.0) as u64);
            Ok(TokenTotals {
                since: Some(since),
                replies: row.get::<_, i64>(0)?.max(0) as u64,
                input: count(1)?,
                output: count(2)?,
                reasoning: count(3)?,
                cache_read: count(4)?,
                cache_write: count(5)?,
                cost: row.get::<_, f64>(6)?,
                observed_at: last_written(database),
            })
        })
    };

    read().inspect_err(|err| tracing::debug!(%err, "OpenCode database could not be read")).ok()
}

/// OpenCode writes through a write-ahead log, so the log changes long before the database does.
fn last_written(database: &Path) -> Option<SystemTime> {
    let mut wal = database.as_os_str().to_owned();
    wal.push("-wal");
    [database.to_path_buf(), PathBuf::from(wal)]
        .iter()
        .filter_map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    const NOW: u64 = 1_800_000_000;

    /// The OpenCode 1 message table, with made-up rows.
    fn fixture(rows: &[(u64, &str)]) -> (tempfile::TempDir, PathBuf) {
        let home = tempfile::tempdir().unwrap();
        let path = database(home.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL,
                 time_created integer NOT NULL, time_updated integer NOT NULL, data text NOT NULL);",
            )
            .unwrap();
        for (i, (seconds, body)) in rows.iter().enumerate() {
            let ms = (*seconds * 1000) as i64;
            connection
                .execute(
                    "INSERT INTO message VALUES (?1, 'ses_example', ?2, ?2, ?3)",
                    rusqlite::params![format!("msg_{i}"), ms, body],
                )
                .unwrap();
        }
        (home, path)
    }

    const REPLY: &str = r#"{"role":"assistant","modelID":"example","providerID":"example","cost":0.25,"tokens":{"input":100,"output":40,"reasoning":5,"cache":{"read":1000,"write":20}},"time":{"created":0}}"#;
    const PROMPT: &str = r#"{"role":"user","time":{"created":0}}"#;

    #[test]
    fn sums_the_replies_within_the_span() {
        let (_home, path) = fixture(&[
            (NOW - 60, REPLY),
            (NOW - 30, REPLY),
            (NOW - 20, PROMPT),
            (NOW - TOTALS_SPAN.as_secs() - 1, REPLY),
        ]);

        let totals = totals(&path, at(NOW)).unwrap();
        assert_eq!(totals.replies, 2);
        assert_eq!((totals.input, totals.output, totals.reasoning), (200, 80, 10));
        assert_eq!((totals.cache_read, totals.cache_write), (2000, 40));
        assert!((totals.cost - 0.5).abs() < 1e-9);
        assert_eq!(totals.since, Some(at(NOW) - TOTALS_SPAN));
        assert!(totals.observed_at.is_some());
    }

    #[test]
    fn a_damaged_row_does_not_spoil_the_rest() {
        let (_home, path) = fixture(&[(NOW - 60, REPLY), (NOW - 50, "{\"role\":\"assist")]);
        assert_eq!(totals(&path, at(NOW)).unwrap().replies, 1);
    }

    #[test]
    fn an_empty_span_is_zero_rather_than_nothing() {
        let (_home, path) = fixture(&[]);
        assert_eq!(totals(&path, at(NOW)).unwrap().replies, 0);
    }

    #[test]
    fn an_unfamiliar_layout_is_no_reading() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("opencode.db");
        Connection::open(&path).unwrap().execute_batch("CREATE TABLE other (x);").unwrap();
        assert_eq!(totals(&path, at(NOW)), None);
        assert_eq!(totals(&home.path().join("missing.db"), at(NOW)), None);
    }
}
