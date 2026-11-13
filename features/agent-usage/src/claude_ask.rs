//! Asks the person's own installed Claude Code for the figures it shows under `/usage`.
//!
//! Kavverna reads no credential and makes no network request here. Claude Code is started with
//! no tools, no MCP servers, no hooks and no session file, is sent the two control requests of
//! its structured protocol and is stopped as soon as it answers. It signs in and asks Anthropic
//! itself, exactly as when the person types `/usage`; no prompt is sent, so no model runs.
//!
//! Only the windows are kept from the answer: percentage, reset time, the model a window is
//! scoped to and how much of the weekly limit that scope may take. The answer also describes the
//! account, its plan and its spending, and none of that is kept or logged.

use serde_json::{Value, json};
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime};

use crate::{FIVE_HOUR_MINUTES, SEVEN_DAY_MINUTES, Window};

/// Claude Code takes a few seconds to start and to hear back from its account. Anything slower
/// than this is treated as no answer rather than left to hold the reader.
pub const ASK_TIMEOUT: Duration = Duration::from_secs(25);

/// While the panel is open, Claude Code is asked again at most this often.
pub const ASK_EVERY: Duration = Duration::from_secs(60);

const RETRY_UNREACHABLE: Duration = Duration::from_secs(5 * 60);
/// Only the person can install, update or sign in to Claude Code, so asking sooner changes
/// nothing.
const RETRY_SETTLED: Duration = Duration::from_secs(30 * 60);

const ARGUMENTS: [&str; 16] = [
    "--print",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--no-session-persistence",
    "--tools",
    "",
    "--strict-mcp-config",
    "--mcp-config",
    r#"{"mcpServers":{}}"#,
    "--setting-sources",
    "",
    "--settings",
    r#"{"disableAllHooks":true}"#,
];

/// Opt-outs and network settings a person may keep in Claude Code's own `settings.json`, which
/// `--setting-sources ""` would otherwise leave out of the request.
const CARRIED_SETTINGS: [&str; 7] = [
    "DISABLE_TELEMETRY",
    "DISABLE_ERROR_REPORTING",
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "NODE_EXTRA_CA_CERTS",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskError {
    /// No `claude` executable where its installer or PATH would put it.
    Missing,
    /// Claude Code answered without an allowance, or said it is not signed in.
    NeedsSignIn,
    /// Claude Code does not know the usage request.
    Outdated,
    /// It did not start, exited, timed out or answered something unreadable.
    Unreachable,
}

impl AskError {
    /// Stable across releases, for scripts and agents that read the snapshot.
    pub fn id(self) -> &'static str {
        match self {
            AskError::Missing => "claude-code-missing",
            AskError::NeedsSignIn => "claude-code-signed-out",
            AskError::Outdated => "claude-code-outdated",
            AskError::Unreachable => "claude-code-unreachable",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            AskError::Missing => "Claude Code not found.",
            AskError::NeedsSignIn => "Sign in to Claude Code to see its allowance.",
            AskError::Outdated => "Update Claude Code to see its allowance.",
            AskError::Unreachable => "Claude Code did not answer. Kavverna will ask again.",
        }
    }
}

/// How long to wait before asking again. Each failure in a row without an answer doubles the
/// wait, up to the one for what only the person can fix.
pub fn ask_again_after(failure: Option<AskError>, failures_in_a_row: u32) -> Duration {
    match failure {
        None => ASK_EVERY,
        Some(AskError::Unreachable) => {
            let doublings = failures_in_a_row.saturating_sub(1).min(3);
            (RETRY_UNREACHABLE * (1 << doublings)).min(RETRY_SETTLED)
        }
        Some(AskError::Missing | AskError::NeedsSignIn | AskError::Outdated) => RETRY_SETTLED,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeAnswer {
    /// When the answer arrived.
    pub observed_at: SystemTime,
    limits: Vec<AnsweredLimit>,
}

#[derive(Debug, Clone, PartialEq)]
struct AnsweredLimit {
    minutes: Option<u32>,
    used_percent: f64,
    resets_at: Option<SystemTime>,
    scope: Option<String>,
    note: Option<String>,
}

impl ClaudeAnswer {
    pub(crate) fn windows(&self, now: SystemTime) -> Vec<Window> {
        self.limits
            .iter()
            .map(|limit| Window {
                scope: limit.scope.clone(),
                note: limit.note.clone(),
                ..Window::new(
                    limit.minutes,
                    limit.used_percent,
                    limit.resets_at,
                    self.observed_at,
                    now,
                )
            })
            .collect()
    }
}

/// `None` when the answer carries no window at all.
///
/// `rate_limits.limits` is the list `/usage` itself draws, including the weekly caps scoped to
/// one model. A scoped cap's share of the weekly limit sits in `weekly_scoped_shares`, matched
/// by its position in that list. Answers without the list still carry the plain five hour and
/// weekly windows, which fill in whatever the list left out.
pub(crate) fn parse_answer(answer: &Value, observed_at: SystemTime) -> Option<ClaudeAnswer> {
    let rate = answer.get("rate_limits").filter(|r| r.is_object())?;
    let listed = |key: &str| {
        rate.get(key)
            .and_then(Value::as_array)
            .or_else(|| answer.get(key).and_then(Value::as_array))
    };
    let shares = listed("weekly_scoped_shares").cloned().unwrap_or_default();

    let mut limits: Vec<AnsweredLimit> = listed("limits")
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, entry)| {
            let used_percent = percent(entry.get("percent")?)?;
            let scope = entry
                .pointer("/scope/model/display_name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned);
            let note = shares
                .iter()
                .find(|share| {
                    share.get("limit_index").and_then(Value::as_u64) == Some(index as u64)
                })
                .and_then(|share| share.get("allowance_percent_of_weekly")?.as_f64())
                .filter(|share| share.is_finite() && *share > 0.0)
                .map(|share| format!("Up to {} of your weekly limit", whole_percent(share)));
            Some(AnsweredLimit {
                minutes: minutes_of(entry),
                used_percent,
                resets_at: entry.get("resets_at").and_then(Value::as_str).and_then(timestamp),
                scope,
                note,
            })
        })
        .collect();

    for (key, minutes) in [("five_hour", FIVE_HOUR_MINUTES), ("seven_day", SEVEN_DAY_MINUTES)] {
        let covered = limits.iter().any(|l| l.scope.is_none() && l.minutes == Some(minutes));
        if covered {
            continue;
        }
        let Some(window) = rate.get(key).filter(|w| w.is_object()) else {
            continue;
        };
        let Some(used_percent) = window.get("utilization").and_then(percent) else {
            continue;
        };
        limits.push(AnsweredLimit {
            minutes: Some(minutes),
            used_percent,
            resets_at: window.get("resets_at").and_then(Value::as_str).and_then(timestamp),
            scope: None,
            note: None,
        });
    }

    // Account wide windows first, shortest first, so a scoped weekly cap is never the first
    // weekly figure a reader comes to.
    limits.sort_by_key(|l| {
        let length = match l.minutes {
            Some(FIVE_HOUR_MINUTES) => 0,
            Some(SEVEN_DAY_MINUTES) => 1,
            _ => 2,
        };
        (l.scope.is_some(), length)
    });
    (!limits.is_empty()).then_some(ClaudeAnswer { observed_at, limits })
}

/// Only the weekly group has been seen in the list so far, so the session window is recognised
/// by every name it plausibly carries and anything else is left without a length.
fn minutes_of(entry: &Value) -> Option<u32> {
    let named = |key: &str| entry.get(key).and_then(Value::as_str).unwrap_or_default();
    let (group, kind) = (named("group"), named("kind"));
    if group == "weekly" || kind.starts_with("weekly") || kind.starts_with("seven_day") {
        Some(SEVEN_DAY_MINUTES)
    } else if ["session", "five_hour"].iter().any(|s| group == *s || kind.starts_with(s)) {
        Some(FIVE_HOUR_MINUTES)
    } else {
        None
    }
}

fn percent(value: &Value) -> Option<f64> {
    value.as_f64().filter(|p| p.is_finite())
}

fn whole_percent(value: f64) -> String {
    if value.fract() == 0.0 { format!("{value:.0}%") } else { format!("{value:.1}%") }
}

/// RFC 3339 as Claude Code writes it, `2026-11-18T15:00:00.308037+00:00`, with or without a
/// fraction and with `Z` or a numeric offset.
pub(crate) fn timestamp(text: &str) -> Option<SystemTime> {
    let bytes = text.as_bytes();
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = text.get(range)?;
        digits.bytes().all(|b| b.is_ascii_digit()).then(|| digits.parse().ok())?
    };
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't' | b' ')
    {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
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

    let mut rest = &text[19..];
    let mut nanos = 0u32;
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let kept = &fraction[..digits.min(9)];
        nanos = kept.parse::<u32>().ok()? * 10u32.pow(9 - kept.len() as u32);
        rest = &fraction[digits..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ if rest.len() == 6 && rest.as_bytes()[3] == b':' => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let hours: i64 = rest.get(1..3)?.parse().ok()?;
            let minutes: i64 = rest.get(4..6)?.parse().ok()?;
            sign * (hours * 3600 + minutes * 60)
        }
        _ => return None,
    };

    let seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset;
    let seconds = u64::try_from(seconds).ok()?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::new(seconds, nanos))
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Where Claude Code's installer puts it, then along `search_path`.
pub fn find_claude_code_in(home: &Path, search_path: Option<&OsStr>) -> Option<PathBuf> {
    let installed = [home.join(".local/bin/claude"), home.join(".claude/local/claude")];
    let along_path = search_path
        .map(|path| std::env::split_paths(path).filter(|dir| dir.is_absolute()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|dir| dir.join("claude"));
    installed.into_iter().chain(along_path).find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// The environment Claude Code is started with.
///
/// Kavverna may itself have been started from inside a Claude Code session, whose markers would
/// make the child think it is nested, and whose tokens or provider routing would replace the
/// person's own sign-in. Those are dropped; the network settings are kept, and the opt-outs and
/// proxy the person keeps in Claude Code's `settings.json` are carried over, since the request
/// is started without its settings.
pub fn claude_code_environment(
    inherited: impl IntoIterator<Item = (String, String)>,
    settings: Option<&Value>,
) -> Vec<(String, String)> {
    let mut environment: Vec<(String, String)> = inherited
        .into_iter()
        .filter(|(key, _)| {
            CARRIED_SETTINGS.contains(&key.as_str())
                || key == "CLAUDE_CONFIG_DIR"
                || !(key.starts_with("CLAUDE") || key.starts_with("ANTHROPIC_"))
        })
        .collect();
    let configured = settings.and_then(|s| s.get("env"));
    for key in CARRIED_SETTINGS {
        let value = match configured.and_then(|env| env.get(key)) {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Number(number)) => number.to_string(),
            Some(Value::Bool(switch)) => switch.to_string(),
            _ => continue,
        };
        environment.retain(|(existing, _)| existing != key);
        environment.push((key.to_owned(), value));
    }
    environment.retain(|(key, _)| key != "DISABLE_AUTOUPDATER");
    // A check must never be the moment Claude Code replaces itself.
    environment.push(("DISABLE_AUTOUPDATER".into(), "1".into()));
    environment
}

/// The command for one usage request, with the environment already prepared.
pub fn usage_command(executable: &Path, environment: &[(String, String)]) -> Command {
    let mut command = Command::new(executable);
    command.args(ARGUMENTS).env_clear().envs(environment.iter().map(|(k, v)| (k, v)));
    command
}

/// Runs one usage request and stops the child however it ends. Blocks for at most `timeout`.
pub fn ask_with(mut command: Command, timeout: Duration) -> Result<ClaudeAnswer, AskError> {
    let deadline = Instant::now() + timeout;
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => AskError::Missing,
            _ => AskError::Unreachable,
        })?;
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AskError::Unreachable);
    };

    // Never joined: a grandchild holding the pipe open would hold the join too. It ends when
    // the pipe closes or when nobody is listening any more.
    let (lines, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if lines.send(line).is_err() {
                break;
            }
        }
    });

    let outcome =
        request(&mut stdin, &received, "initialize", json!({"subtype": "initialize"}), deadline)
            .and_then(|_| {
                request(
                    &mut stdin,
                    &received,
                    "usage",
                    json!({"subtype": "get_usage", "skip_behaviors": true}),
                    deadline,
                )
            });
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    let answer = outcome?;
    if answer.get("rate_limits_available").and_then(Value::as_bool) != Some(true) {
        return Err(AskError::NeedsSignIn);
    }
    parse_answer(&answer, SystemTime::now()).ok_or(AskError::Unreachable)
}

fn request(
    stdin: &mut ChildStdin,
    received: &Receiver<String>,
    id: &str,
    body: Value,
    deadline: Instant,
) -> Result<Value, AskError> {
    let message = json!({"type": "control_request", "request_id": id, "request": body});
    writeln!(stdin, "{message}").and_then(|_| stdin.flush()).map_err(|_| AskError::Unreachable)?;

    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = match received.recv_timeout(left) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                return Err(AskError::Unreachable);
            }
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        if message.get("type").and_then(Value::as_str) != Some("control_response") {
            continue;
        }
        let Some(response) = message.get("response") else { continue };
        if response.get("request_id").and_then(Value::as_str) != Some(id) {
            continue;
        }
        if response.get("subtype").and_then(Value::as_str) == Some("success") {
            return Ok(response.get("response").cloned().unwrap_or(Value::Null));
        }
        let error =
            response.get("error").and_then(Value::as_str).unwrap_or_default().to_lowercase();
        // A Claude Code from before the request answers "Unsupported control request subtype".
        return Err(if error.contains("supported") || error.contains("unknown") {
            AskError::Outdated
        } else if ["auth", "login", "log in", "sign in"].iter().any(|s| error.contains(s)) {
            AskError::NeedsSignIn
        } else {
            AskError::Unreachable
        });
    }
}

/// Asks the Claude Code installed for this user, with this process's environment.
pub fn ask_claude_code() -> Result<ClaudeAnswer, AskError> {
    let home = directories::BaseDirs::new().ok_or(AskError::Missing)?.home_dir().to_path_buf();
    let path = std::env::var_os("PATH");
    let executable = find_claude_code_in(&home, path.as_deref()).ok_or(AskError::Missing)?;
    let config = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".claude"));
    let settings = std::fs::read(config.join("settings.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let environment = claude_code_environment(std::env::vars(), settings.as_ref());
    ask_with(usage_command(&executable, &environment), ASK_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowKind;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    /// Shaped like the answer to `get_usage`, written by hand with made-up values. The keys
    /// beside the windows stand in for what must never be kept.
    const ANSWER: &str = r#"{
        "rate_limits_available": true,
        "subscription_type": "example-plan",
        "session": {"id": "example-session-id"},
        "behaviors": {"example_behavior": true},
        "rate_limits": {
            "five_hour": null,
            "seven_day": {"utilization": 70, "resets_at": "2027-01-08T15:00:00.308037+00:00"},
            "seven_day_breakdown": {"rows": [{"key": "example_surface", "percent": 100}]},
            "limits": [
                {"kind": "weekly_all", "group": "weekly", "percent": 70, "severity": "normal",
                 "resets_at": "2027-01-08T15:00:00.308037+00:00", "scope": null},
                {"kind": "weekly_scoped", "group": "weekly", "percent": 95, "severity": "critical",
                 "resets_at": "2027-01-08T15:00:00Z",
                 "scope": {"model": {"id": null, "display_name": "Example Model"}}}
            ],
            "weekly_scoped_shares": [
                {"limit_index": 1, "used_percent_of_weekly": 48, "allowance_percent_of_weekly": 50}
            ]
        },
        "limits": null,
        "weekly_scoped_shares": null
    }"#;

    fn parsed(text: &str) -> ClaudeAnswer {
        parse_answer(&serde_json::from_str(text).unwrap(), at(1_799_000_000)).unwrap()
    }

    #[test]
    fn timestamps_are_read_with_fraction_and_offset() {
        assert_eq!(timestamp("1970-01-01T00:00:00Z"), Some(at(0)));
        assert_eq!(timestamp("2027-01-08T15:00:00+00:00"), Some(at(1_799_420_400)));
        assert_eq!(timestamp("2027-01-08T16:00:00+01:00"), Some(at(1_799_420_400)));
        assert_eq!(timestamp("2027-01-08T10:30:00-04:30"), Some(at(1_799_420_400)));
        assert_eq!(
            timestamp("2027-01-08T15:00:00.25Z"),
            Some(at(1_799_420_400) + Duration::from_millis(250))
        );
        assert_eq!(timestamp("2024-02-29T00:00:00Z"), Some(at(1_709_164_800)));
        for broken in ["", "2027-01-08", "2027-01-08T15:00:00", "2027-13-01T00:00:00Z", "soon"] {
            assert_eq!(timestamp(broken), None, "{broken}");
        }
    }

    #[test]
    fn the_listed_limits_become_windows_with_their_scope_and_share() {
        let windows = parsed(ANSWER).windows(at(1_799_000_030));
        assert_eq!(windows.len(), 2);

        assert_eq!(windows[0].kind, WindowKind::Weekly);
        assert_eq!(windows[0].used_percent, 70.0);
        assert_eq!(windows[0].scope, None);
        assert_eq!(windows[0].note, None);

        assert_eq!(windows[1].kind, WindowKind::Weekly);
        assert_eq!(windows[1].used_percent, 95.0);
        assert_eq!(windows[1].scope.as_deref(), Some("Example Model"));
        assert_eq!(windows[1].note.as_deref(), Some("Up to 50% of your weekly limit"));
        assert_eq!(windows[1].resets_at, Some(at(1_799_420_400)));
        assert_eq!(windows[1].observed_at, at(1_799_000_000));
    }

    #[test]
    fn a_session_entry_is_put_first_and_plain_windows_fill_the_gaps() {
        let answer = r#"{"rate_limits_available": true, "rate_limits": {
            "five_hour": {"utilization": 12.5, "resets_at": "2027-01-08T15:00:00Z"},
            "seven_day": {"utilization": 99, "resets_at": "2027-01-08T15:00:00Z"},
            "limits": [
                {"kind": "weekly_scoped", "group": "weekly", "percent": 40,
                 "scope": {"model": {"display_name": "Example Model"}}},
                {"kind": "weekly_all", "group": "weekly", "percent": 30}
            ]}}"#;
        let windows = parsed(answer).windows(at(1_799_000_000));
        let shown: Vec<_> =
            windows.iter().map(|w| (w.kind, w.used_percent, w.scope.is_some())).collect();
        assert_eq!(
            shown,
            vec![
                (WindowKind::Session, 12.5, false),
                // The listed weekly figure, not the plain one, which the list already covers.
                (WindowKind::Weekly, 30.0, false),
                (WindowKind::Weekly, 40.0, true),
            ]
        );
        assert_eq!(windows[2].note, None);
    }

    #[test]
    fn an_answer_without_the_list_still_has_its_plain_windows() {
        let answer = r#"{"rate_limits": {
            "five_hour": {"utilization": 5, "resets_at": null},
            "seven_day": {"utilization": 41, "resets_at": "2027-01-08T15:00:00Z"}}}"#;
        let windows = parsed(answer).windows(at(1_799_000_000));
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].kind, WindowKind::Session);
        assert_eq!(windows[0].resets_at, None);
        assert_eq!(windows[1].used_percent, 41.0);
    }

    #[test]
    fn an_answer_without_windows_is_not_a_reading() {
        for answer in [
            r#"{"rate_limits": null}"#,
            r#"{"rate_limits": {"five_hour": null, "seven_day": null, "limits": []}}"#,
            r#"{"rate_limits": {"limits": [{"kind": "weekly_all", "percent": "most"}]}}"#,
        ] {
            assert_eq!(parse_answer(&serde_json::from_str(answer).unwrap(), at(1)), None);
        }
    }

    #[test]
    fn nothing_but_the_windows_is_kept() {
        let kept = format!("{:?}", parsed(ANSWER));
        for private in [
            "example-plan",
            "example-session-id",
            "example_behavior",
            "example_surface",
            "critical",
        ] {
            assert!(!kept.contains(private), "{private} survived into the answer");
        }
    }

    #[test]
    fn failures_back_off() {
        assert_eq!(ask_again_after(None, 0), ASK_EVERY);
        let minutes = |failure, n| ask_again_after(Some(failure), n).as_secs() / 60;
        assert_eq!(minutes(AskError::Unreachable, 1), 5);
        assert_eq!(minutes(AskError::Unreachable, 2), 10);
        assert_eq!(minutes(AskError::Unreachable, 3), 20);
        assert_eq!(minutes(AskError::Unreachable, 4), 30);
        assert_eq!(minutes(AskError::Unreachable, 40), 30);
        for settled in [AskError::Missing, AskError::NeedsSignIn, AskError::Outdated] {
            assert_eq!(minutes(settled, 1), 30);
        }
    }

    #[test]
    fn the_environment_drops_session_markers_and_keeps_the_network() {
        let inherited = [
            ("PATH", "/usr/bin"),
            ("HOME", "/home/example"),
            ("HTTPS_PROXY", "http://proxy.example:3128"),
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_ENTRYPOINT", "cli"),
            ("ANTHROPIC_API_KEY", "example-key"),
            ("CLAUDE_CONFIG_DIR", "/home/example/.claude-work"),
            ("DISABLE_AUTOUPDATER", "0"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()));
        let settings =
            json!({"env": {"DISABLE_TELEMETRY": 1, "NO_PROXY": "example.internal", "OTHER": "x"}});

        let environment = claude_code_environment(inherited, Some(&settings));
        let value = |key: &str| environment.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        assert_eq!(value("PATH"), Some("/usr/bin"));
        assert_eq!(value("HTTPS_PROXY"), Some("http://proxy.example:3128"));
        assert_eq!(value("CLAUDE_CONFIG_DIR"), Some("/home/example/.claude-work"));
        assert_eq!(value("DISABLE_TELEMETRY"), Some("1"));
        assert_eq!(value("NO_PROXY"), Some("example.internal"));
        assert_eq!(value("DISABLE_AUTOUPDATER"), Some("1"));
        for dropped in ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT", "ANTHROPIC_API_KEY", "OTHER"] {
            assert_eq!(value(dropped), None, "{dropped}");
        }
        assert_eq!(environment.iter().filter(|(k, _)| k == "DISABLE_AUTOUPDATER").count(), 1);
    }

    #[test]
    fn claude_code_is_found_where_it_is_installed_then_along_the_path() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        let place = |path: &Path, mode: u32| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"").unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        };
        let search = std::env::join_paths([Path::new("relative"), tools.path()]).unwrap();
        assert_eq!(find_claude_code_in(home.path(), Some(&search)), None);

        place(&tools.path().join("claude"), 0o755);
        assert_eq!(
            find_claude_code_in(home.path(), Some(&search)),
            Some(tools.path().join("claude"))
        );

        place(&home.path().join(".local/bin/claude"), 0o644);
        assert_eq!(
            find_claude_code_in(home.path(), Some(&search)),
            Some(tools.path().join("claude")),
            "a file that cannot run was taken"
        );
        place(&home.path().join(".claude/local/claude"), 0o755);
        assert_eq!(
            find_claude_code_in(home.path(), Some(&search)),
            Some(home.path().join(".claude/local/claude"))
        );
    }

    /// A stand-in for Claude Code: a shell script that reads the two requests and answers them
    /// the way the real one does, run through `sh` so no file needs to be executable.
    fn fake(script: &str) -> (tempfile::TempDir, Command) {
        let room = tempfile::tempdir().unwrap();
        let path = room.path().join("claude.sh");
        std::fs::write(&path, script).unwrap();
        let mut command = Command::new("sh");
        command.arg(&path);
        (room, command)
    }

    fn answering(usage_response: &str) -> String {
        format!(
            "read first\n\
             echo '{{\"type\":\"system\",\"subtype\":\"init\"}}'\n\
             echo '{{\"type\":\"control_response\",\"response\":{{\"subtype\":\"success\",\"request_id\":\"initialize\",\"response\":{{}}}}}}'\n\
             read second\n\
             echo 'not json'\n\
             echo '{usage_response}'\n\
             exec sleep 30\n"
        )
    }

    fn compact(text: &str) -> String {
        serde_json::to_string(&serde_json::from_str::<Value>(text).unwrap()).unwrap()
    }

    #[test]
    fn a_fake_claude_code_is_asked_and_stopped() {
        let response = format!(
            r#"{{"type":"control_response","response":{{"subtype":"success","request_id":"usage","response":{}}}}}"#,
            compact(ANSWER)
        );
        let (_room, command) = fake(&answering(&response));
        let started = Instant::now();
        let answer = ask_with(command, Duration::from_secs(10)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(5), "waited for the child to exit");
        assert_eq!(answer.windows(SystemTime::now()).len(), 2);
    }

    #[test]
    fn a_signed_out_answer_asks_for_sign_in() {
        let response = r#"{"type":"control_response","response":{"subtype":"success","request_id":"usage","response":{"rate_limits_available":false,"rate_limits":null}}}"#;
        let (_room, command) = fake(&answering(response));
        assert_eq!(ask_with(command, Duration::from_secs(10)), Err(AskError::NeedsSignIn));
    }

    #[test]
    fn an_unknown_request_means_an_outdated_claude_code() {
        let response = r#"{"type":"control_response","response":{"subtype":"error","request_id":"usage","error":"Unsupported control request subtype: get_usage"}}"#;
        let (_room, command) = fake(&answering(response));
        assert_eq!(ask_with(command, Duration::from_secs(10)), Err(AskError::Outdated));
    }

    #[test]
    fn silence_ends_at_the_deadline() {
        let (_room, command) = fake("exec sleep 30\n");
        let started = Instant::now();
        assert_eq!(ask_with(command, Duration::from_millis(300)), Err(AskError::Unreachable));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn an_early_exit_is_unreachable() {
        let (_room, command) = fake("exit 1\n");
        assert_eq!(ask_with(command, Duration::from_secs(10)), Err(AskError::Unreachable));
    }

    #[test]
    fn a_missing_executable_is_missing() {
        let command = Command::new("/nonexistent/kavverna-test/claude");
        assert_eq!(ask_with(command, Duration::from_secs(1)), Err(AskError::Missing));
    }

    #[test]
    #[ignore = "starts the Claude Code installed for this user, which asks its account"]
    fn the_installed_claude_code_answers() {
        let answer = ask_claude_code().unwrap();
        assert!(!answer.windows(SystemTime::now()).is_empty());
    }
}
