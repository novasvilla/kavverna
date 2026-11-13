use crate::{agent_usage_view, panel};
use agent_usage::{Freshness, Snapshot, Usage, UsageMeter, Window, WindowKind};
use serde_json::{Value, json};
use std::io::{IsTerminal, Read, Write};
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

/// The agents write their figures down at most once a reply, so reading more often finds the
/// same files. Nothing is read at all while the panel is hidden.
const EVERY: Duration = Duration::from_secs(60);

/// A status line document is a few kilobytes. Anything far beyond that is not one, and reading
/// it whole would only slow the status line down.
const LONGEST_DOCUMENT: u64 = 1024 * 1024;

enum Request {
    /// The panel opened or asked again. Ignored if it has closed in the meantime.
    Read,
    /// A script on the bus wants a reading now, open panel or not.
    Answer(tokio::sync::oneshot::Sender<Snapshot>),
}

static REQUESTS: Mutex<Option<Sender<Request>>> = Mutex::new(None);
static LATEST: Mutex<Option<Snapshot>> = Mutex::new(None);

fn lock() -> MutexGuard<'static, Option<Snapshot>> {
    LATEST.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `None` until the first reading, which is not the same as a reading that found no agent.
pub fn latest() -> Option<Snapshot> {
    lock().clone()
}

fn send(request: Request) -> bool {
    REQUESTS
        .lock()
        .ok()
        .and_then(|held| held.as_ref().map(|sender| sender.send(request).is_ok()))
        .unwrap_or(false)
}

/// Called as the panel opens, and by the view when asked to read again. Does nothing when the
/// feature is not installed, since then no reader is running.
pub fn wake() {
    send(Request::Read);
}

/// Owns the one meter for the life of the process, so closed Codex sessions are decoded once
/// rather than on every reading.
pub fn run(on_change: impl Fn()) {
    let (sender, requests) = std::sync::mpsc::channel();
    if let Ok(mut held) = REQUESTS.lock() {
        *held = Some(sender);
    }
    tracing::info!("agent usage reader started");

    let mut meter = UsageMeter::new();
    let mut due = panel::is_open();
    loop {
        if due {
            take(&mut meter);
            on_change();
        }
        let next = if panel::is_open() {
            requests.recv_timeout(EVERY)
        } else {
            requests.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        due = match next {
            Ok(Request::Read) | Err(RecvTimeoutError::Timeout) => panel::is_open(),
            Ok(Request::Answer(reply)) => {
                let _ = reply.send(take(&mut meter));
                if panel::is_open() {
                    on_change();
                }
                false
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
    }
}

fn take(meter: &mut UsageMeter) -> Snapshot {
    let snapshot = meter.read(SystemTime::now());
    *lock() = Some(snapshot.clone());
    snapshot
}

/// Freshness is judged when a window is read, so a reading kept for a while has to be judged
/// again against the time it is shown at, or a window that has since reset would still look
/// current.
pub fn judged(window: &Window, now: SystemTime) -> Freshness {
    Freshness::judge(window.observed_at, window.resets_at, now)
}

pub fn freshness_name(freshness: Freshness) -> &'static str {
    match freshness {
        Freshness::Fresh => "fresh",
        Freshness::Stale => "stale",
        Freshness::Expired => "expired",
    }
}

/// A fresh reading for the bus, taken by the reader thread so the meter stays in one place.
/// Falls back to the last reading if the reader went away mid request.
pub async fn reading_as_json() -> String {
    let (reply, answer) = tokio::sync::oneshot::channel();
    let installed = send(Request::Answer(reply));
    let snapshot = if installed { answer.await.ok().or_else(latest) } else { None };
    as_json(installed, snapshot.as_ref(), SystemTime::now()).to_string()
}

fn epoch(at: SystemTime) -> Option<u64> {
    at.duration_since(SystemTime::UNIX_EPOCH).ok().map(|since| since.as_secs())
}

fn seconds_since(at: SystemTime, now: SystemTime) -> u64 {
    now.duration_since(at).unwrap_or_default().as_secs()
}

fn as_json(installed: bool, snapshot: Option<&Snapshot>, now: SystemTime) -> Value {
    let Some(snapshot) = snapshot else {
        return json!({ "installed": installed, "read": false, "agents": [] });
    };

    let agents: Vec<Value> = snapshot
        .agents
        .iter()
        .map(|found| {
            let mut agent = json!({
                "id": found.agent.id(),
                "title": found.agent.title(),
            });
            match &found.usage {
                Usage::NoReading => agent["usage"] = json!("no-reading"),
                Usage::Allowance(windows) => {
                    agent["usage"] = json!("allowance");
                    agent["windows"] = windows.iter().map(|w| window_json(w, now)).collect();
                }
                Usage::Totals(totals) => {
                    agent["usage"] = json!("totals");
                    agent["totals"] = json!({
                        "span_seconds": agent_usage::TOTALS_SPAN.as_secs(),
                        "since": totals.since.and_then(epoch),
                        "replies": totals.replies,
                        "input": totals.input,
                        "output": totals.output,
                        "reasoning": totals.reasoning,
                        "cache_read": totals.cache_read,
                        "cache_write": totals.cache_write,
                        "cost_usd": totals.cost,
                        "observed_at": totals.observed_at.and_then(epoch),
                        "age_seconds": totals.observed_at.map(|at| seconds_since(at, now)),
                    });
                }
            }
            agent
        })
        .collect();

    json!({
        "installed": installed,
        "read": true,
        "taken_at": epoch(snapshot.taken_at),
        "age_seconds": seconds_since(snapshot.taken_at, now),
        "stale_after_seconds": agent_usage::STALE_AFTER.as_secs(),
        "agents": agents,
    })
}

fn window_json(window: &Window, now: SystemTime) -> Value {
    json!({
        "label": agent_usage_view::window_label(window.kind, window.minutes),
        "kind": match window.kind {
            WindowKind::Session => "session",
            WindowKind::Weekly => "weekly",
            WindowKind::Other => "other",
        },
        "minutes": window.minutes,
        "used_percent": window.used_percent,
        "resets_at": window.resets_at.and_then(epoch),
        "resets_in_seconds": window
            .resets_at
            .and_then(|reset| reset.duration_since(now).ok())
            .map(|left| left.as_secs()),
        "observed_at": epoch(window.observed_at),
        "age_seconds": window.age(now).as_secs(),
        "freshness": freshness_name(judged(window, now)),
    })
}

/// Claude Code's status line command. It keeps the allowance the document carries and prints a
/// short status line in return, and it never fails: an error here would blank the status line
/// of every session, which is worse than a missed reading.
pub fn pass_status_line() {
    let mut document = Vec::new();
    // Run by hand from a terminal there is no document coming, and waiting for one would hang.
    if !std::io::stdin().is_terminal() {
        let _ = std::io::stdin().take(LONGEST_DOCUMENT).read_to_end(&mut document);
    }
    if let Some(cache) = agent_usage::cache_dir() {
        let _ = agent_usage::record_status_line(&document, &cache, SystemTime::now());
    }
    // Not `println!`, which panics when Claude Code has already closed the pipe.
    let _ = writeln!(std::io::stdout().lock(), "{}", status_line_text(&document));
}

pub fn status_line_text(document: &[u8]) -> String {
    let model = serde_json::from_slice::<Value>(document).ok().and_then(|root| {
        root.pointer("/model/display_name").and_then(Value::as_str).map(str::to_owned)
    });

    let mut parts = vec![model.filter(|name| !name.trim().is_empty()).unwrap_or("Claude".into())];
    if let Some(limits) = agent_usage::parse_status_line(document) {
        if let Some(window) = limits.five_hour {
            parts.push(format!("5h {:.0}%", window.used_percent));
        }
        if let Some(window) = limits.seven_day {
            parts.push(format!("week {:.0}%", window.used_percent));
        }
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_usage::{Agent, AgentUsage, TokenTotals};

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    const DOCUMENT: &str = r#"{
        "model": {"id": "example-model", "display_name": "Example"},
        "rate_limits": {
            "five_hour": {"used_percentage": 23.4, "resets_at": 1800000000},
            "seven_day": {"used_percentage": 41, "resets_at": 1800300000}
        }
    }"#;

    #[test]
    fn the_status_line_names_the_model_and_both_windows() {
        assert_eq!(status_line_text(DOCUMENT.as_bytes()), "Example · 5h 23% · week 41%");
    }

    #[test]
    fn a_window_missing_from_the_document_is_left_out() {
        let weekly = r#"{"model": {"display_name": "Example"},
            "rate_limits": {"seven_day": {"used_percentage": 65}}}"#;
        assert_eq!(status_line_text(weekly.as_bytes()), "Example · week 65%");
    }

    #[test]
    fn anything_unreadable_still_prints_a_status_line() {
        assert_eq!(status_line_text(b""), "Claude");
        assert_eq!(status_line_text(b"{\"model\": {\"display_n"), "Claude");
        assert_eq!(status_line_text(br#"{"model": {"display_name": "Example"}}"#), "Example");
        assert_eq!(status_line_text(br#"{"model": {"display_name": "  "}}"#), "Claude");
    }

    fn window(minutes: u32, observed: u64, resets: u64) -> Window {
        Window {
            kind: if minutes == 300 { WindowKind::Session } else { WindowKind::Weekly },
            minutes: Some(minutes),
            used_percent: 23.5,
            resets_at: Some(at(resets)),
            observed_at: at(observed),
            state: Freshness::Fresh,
        }
    }

    #[test]
    fn the_bus_judges_freshness_when_asked_not_when_read() {
        let snapshot = Snapshot {
            taken_at: at(1_000),
            agents: vec![AgentUsage {
                agent: Agent::ClaudeCode,
                usage: Usage::Allowance(vec![
                    window(300, 1_000, 2_000),
                    window(10_080, 1_000, 9_000),
                ]),
                source: None,
            }],
        };

        let reading = as_json(true, Some(&snapshot), at(3_000));
        let windows = &reading["agents"][0]["windows"];
        assert_eq!(reading["agents"][0]["id"], "claude-code");
        assert_eq!(reading["age_seconds"], 2_000);
        assert_eq!(windows[0]["freshness"], "expired");
        assert_eq!(windows[0]["resets_in_seconds"], Value::Null);
        assert_eq!(windows[0]["label"], "5-hour");
        assert_eq!(windows[1]["freshness"], "stale");
        assert_eq!(windows[1]["resets_in_seconds"], 6_000);
        assert_eq!(windows[1]["age_seconds"], 2_000);
    }

    #[test]
    fn totals_and_a_missing_reading_are_told_apart() {
        let snapshot = Snapshot {
            taken_at: at(1_000),
            agents: vec![
                AgentUsage { agent: Agent::Codex, usage: Usage::NoReading, source: None },
                AgentUsage {
                    agent: Agent::OpenCode,
                    usage: Usage::Totals(TokenTotals {
                        replies: 3,
                        cost: 0.5,
                        ..Default::default()
                    }),
                    source: None,
                },
            ],
        };

        let reading = as_json(true, Some(&snapshot), at(1_000));
        assert_eq!(reading["agents"][0]["usage"], "no-reading");
        assert_eq!(reading["agents"][1]["usage"], "totals");
        assert_eq!(reading["agents"][1]["totals"]["replies"], 3);
    }

    #[test]
    fn no_reader_and_no_reading_yet_are_said_plainly() {
        let removed = as_json(false, None, at(1));
        assert_eq!(
            (removed["installed"].clone(), removed["read"].clone()),
            (json!(false), json!(false))
        );
        let waiting = as_json(true, None, at(1));
        assert_eq!(
            (waiting["installed"].clone(), waiting["read"].clone()),
            (json!(true), json!(false))
        );
    }
}
