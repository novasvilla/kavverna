//! How much of each installed coding agent's allowance is used, from files the agents leave on
//! this machine and, for Claude Code, from the agent itself.
//!
//! Nothing here opens a network connection or reads a credential. Codex and OpenCode are read
//! only from their files. Claude Code can also be asked for its `/usage` figures, which makes
//! Claude Code, not Kavverna, contact its account with its own sign-in; see `claude_ask`.
//!
//! A file reading is only as recent as the last time an agent wrote it down, so every window
//! carries the moment it was observed, and a window whose reset time has passed is reported as
//! expired instead of being guessed forward.

mod claude;
mod claude_ask;
mod codex;
mod opencode;
mod private_file;

pub use claude::{
    CACHE_FILE as CLAUDE_CACHE_FILE, STATUSBAR_FILE as CLAUDE_STATUSBAR_FILE, StatusLineLimits,
    StatusLineWindow, parse_status_line, record_status_line,
};
pub use claude_ask::{
    ASK_EVERY, ASK_TIMEOUT, AskError, ClaudeAnswer, ask_again_after, ask_claude_code, ask_with,
    claude_code_environment, find_claude_code_in, usage_command,
};
pub use opencode::TOTALS_SPAN;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A reading older than this is still the latest truth on disk, but the agent has not been used
/// since, so the meter says so rather than presenting it as current.
pub const STALE_AFTER: Duration = Duration::from_secs(10 * 60);

const FIVE_HOUR_MINUTES: u32 = 5 * 60;
const SEVEN_DAY_MINUTES: u32 = 7 * 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Agent {
    ClaudeCode,
    Codex,
    OpenCode,
}

impl Agent {
    /// Stable across releases, for scripts and agents that read the snapshot; the title is for
    /// people and may change.
    pub fn id(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude-code",
            Agent::Codex => "codex",
            Agent::OpenCode => "opencode",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "Claude Code",
            Agent::Codex => "Codex",
            Agent::OpenCode => "OpenCode",
        }
    }
}

/// Windows are told apart by their length rather than by where a provider put them: an account
/// can report only its weekly window, and in either slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    /// A short rolling window, five hours on every plan seen so far.
    Session,
    Weekly,
    Other,
}

impl WindowKind {
    fn from_minutes(minutes: Option<u32>) -> Self {
        match minutes {
            Some(m) if m > 0 && m <= 12 * 60 => WindowKind::Session,
            Some(m) if (6 * 1440..=8 * 1440).contains(&m) => WindowKind::Weekly,
            _ => WindowKind::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    /// Observed longer ago than [`STALE_AFTER`], with the window still running.
    Stale,
    /// The window has reset since the reading. Its figure describes a window that is over and
    /// says nothing about the current one.
    Expired,
}

impl Freshness {
    pub fn judge(observed_at: SystemTime, resets_at: Option<SystemTime>, now: SystemTime) -> Self {
        if resets_at.is_some_and(|reset| reset <= now) {
            return Freshness::Expired;
        }
        // A reading stamped in the future comes from a clock that disagrees with ours; it is
        // treated as just made rather than refused.
        match now.duration_since(observed_at) {
            Ok(age) if age > STALE_AFTER => Freshness::Stale,
            _ => Freshness::Fresh,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub kind: WindowKind,
    pub minutes: Option<u32>,
    /// 0 to 100.
    pub used_percent: f64,
    pub resets_at: Option<SystemTime>,
    pub observed_at: SystemTime,
    pub state: Freshness,
    /// The model a cap applies to, as the provider names it, for a window that counts only that
    /// model's use. `None` for a window over everything.
    pub scope: Option<String>,
    /// What the provider says about the window beyond its figure, such as how much of the
    /// weekly limit a scoped cap may take.
    pub note: Option<String>,
}

impl Window {
    fn new(
        minutes: Option<u32>,
        used_percent: f64,
        resets_at: Option<SystemTime>,
        observed_at: SystemTime,
        now: SystemTime,
    ) -> Self {
        Self {
            kind: WindowKind::from_minutes(minutes),
            minutes,
            used_percent: used_percent.clamp(0.0, 100.0),
            resets_at,
            observed_at,
            state: Freshness::judge(observed_at, resets_at, now),
            scope: None,
            note: None,
        }
    }

    pub fn age(&self, now: SystemTime) -> Duration {
        now.duration_since(self.observed_at).unwrap_or_default()
    }
}

/// What an agent spent, for an agent that records no allowance. These are counts the agent wrote
/// down about its own replies, not a share of any limit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TokenTotals {
    /// The totals cover replies created from this moment until the reading.
    pub since: Option<SystemTime>,
    pub replies: u64,
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// In US dollars, as the agent itself priced the replies. Zero for local models.
    pub cost: f64,
    /// When the agent last wrote to its store.
    pub observed_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Usage {
    Allowance(Vec<Window>),
    Totals(TokenTotals),
    /// The agent is installed but has left nothing readable yet. For Claude Code that means it
    /// has not answered, and neither Kavverna's passthrough nor `claude-statusbar` has seen an
    /// allowance.
    NoReading,
}

/// Where a reading came from, for an agent that can be read in more than one way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The installed Claude Code, asked for its `/usage` figures.
    ClaudeCode,
    /// Kavverna's own status line passthrough.
    Kavverna,
    /// The copy the `claude-statusbar` tool keeps of the last status line document.
    ClaudeStatusbar,
}

impl Source {
    /// Stable across releases, like [`Agent::id`].
    pub fn id(self) -> &'static str {
        match self {
            Source::ClaudeCode => "claude-code-usage",
            Source::Kavverna => "kavverna",
            Source::ClaudeStatusbar => "claude-statusbar",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentUsage {
    pub agent: Agent,
    pub usage: Usage,
    /// `None` for an agent read from its own files, which have only one place to be, and for
    /// [`Usage::NoReading`].
    pub source: Option<Source>,
    /// Why asking the agent itself last failed, while it keeps failing. A reading from a file
    /// can still stand beside it.
    pub problem: Option<AskError>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub taken_at: SystemTime,
    /// Only agents found on this machine, in a fixed order.
    pub agents: Vec<AgentUsage>,
}

/// Keeps what it learned from files that have not changed since, so reading every minute does
/// not decompress the same closed Codex sessions each time, and keeps Claude Code's last answer
/// between the times it is asked.
#[derive(Default)]
pub struct UsageMeter {
    codex: codex::Rollouts,
    claude_answer: Option<ClaudeAnswer>,
    claude_problem: Option<AskError>,
}

impl UsageMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the outcome of [`ask_claude_code`] into the readings that follow. A failure leaves
    /// the previous answer in place, to age like any other reading.
    pub fn keep_claude_answer(&mut self, outcome: Result<ClaudeAnswer, AskError>) {
        match outcome {
            Ok(answer) => {
                self.claude_answer = Some(answer);
                self.claude_problem = None;
            }
            Err(problem) => self.claude_problem = Some(problem),
        }
    }

    /// The answer is the fuller reading, with the caps scoped to one model, so it is preferred
    /// while it is current, even over a status line file rewritten seconds later. Once it has
    /// aged, whichever reading is newest wins.
    fn claude_usage(&self, cache_dir: &Path, cache_home: &Path, now: SystemTime) -> AgentUsage {
        let answer = self.claude_answer.as_ref();
        let current =
            answer.filter(|a| Freshness::judge(a.observed_at, None, now) == Freshness::Fresh);
        let file = claude::latest(cache_dir, cache_home);
        let (usage, source) = match (current, answer, file) {
            (Some(answer), _, _) => {
                (Usage::Allowance(answer.windows(now)), Some(Source::ClaudeCode))
            }
            (None, Some(answer), Some(file)) if answer.observed_at >= file.observed_at => {
                (Usage::Allowance(answer.windows(now)), Some(Source::ClaudeCode))
            }
            (None, _, Some(file)) => (Usage::Allowance(file.windows(now)), Some(file.source)),
            (None, Some(answer), None) => {
                (Usage::Allowance(answer.windows(now)), Some(Source::ClaudeCode))
            }
            (None, None, None) => (Usage::NoReading, None),
        };
        AgentUsage { agent: Agent::ClaudeCode, usage, source, problem: self.claude_problem }
    }

    /// `data_dir` is the user's data home, where OpenCode keeps its database, `cache_home` is the
    /// user's cache home, where `claude-statusbar` keeps its copy, and `cache_dir` is Kavverna's
    /// own cache directory, where the Claude Code passthrough writes.
    pub fn read_in(
        &mut self,
        home: &Path,
        data_dir: &Path,
        cache_home: &Path,
        cache_dir: &Path,
        now: SystemTime,
    ) -> Snapshot {
        let mut agents = Vec::new();

        if home.join(".claude").is_dir() {
            agents.push(self.claude_usage(cache_dir, cache_home, now));
        }

        let codex_home = home.join(".codex");
        if codex_home.is_dir() {
            let usage = match self.codex.latest(&codex_home, now) {
                Some(windows) => Usage::Allowance(windows),
                None => Usage::NoReading,
            };
            agents.push(AgentUsage { agent: Agent::Codex, usage, source: None, problem: None });
        }

        let opencode_db = opencode::database(data_dir);
        if opencode_db.is_file() {
            let usage = match opencode::totals(&opencode_db, now) {
                Some(totals) => Usage::Totals(totals),
                None => Usage::NoReading,
            };
            agents.push(AgentUsage { agent: Agent::OpenCode, usage, source: None, problem: None });
        }

        Snapshot { taken_at: now, agents }
    }

    pub fn read(&mut self, now: SystemTime) -> Snapshot {
        match (directories::BaseDirs::new(), cache_dir()) {
            (Some(base), Some(cache)) => {
                self.read_in(base.home_dir(), base.data_dir(), base.cache_dir(), &cache, now)
            }
            _ => Snapshot { taken_at: now, agents: Vec::new() },
        }
    }
}

pub fn discover_in(
    home: &Path,
    data_dir: &Path,
    cache_home: &Path,
    cache_dir: &Path,
    now: SystemTime,
) -> Snapshot {
    UsageMeter::new().read_in(home, data_dir, cache_home, cache_dir, now)
}

pub fn discover(now: SystemTime) -> Snapshot {
    UsageMeter::new().read(now)
}

/// Kavverna's own cache directory, the same one the settings store's project names.
pub fn cache_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "", "kavverna").map(|dirs| dirs.cache_dir().to_path_buf())
}

fn epoch_seconds(seconds: f64) -> Option<SystemTime> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs_f64(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn a_window_past_its_reset_is_expired_whatever_its_age() {
        let now = at(10_000);
        assert_eq!(Freshness::judge(at(9_990), Some(at(10_000)), now), Freshness::Expired);
        assert_eq!(Freshness::judge(at(1_000), Some(at(5_000)), now), Freshness::Expired);
    }

    #[test]
    fn an_old_reading_of_a_running_window_is_stale() {
        let now = at(10_000);
        let old = now - STALE_AFTER - Duration::from_secs(1);
        assert_eq!(Freshness::judge(old, Some(at(20_000)), now), Freshness::Stale);
        assert_eq!(Freshness::judge(old, None, now), Freshness::Stale);
        assert_eq!(Freshness::judge(now - STALE_AFTER, None, now), Freshness::Fresh);
    }

    #[test]
    fn a_reading_from_the_future_counts_as_fresh() {
        assert_eq!(Freshness::judge(at(10_100), Some(at(20_000)), at(10_000)), Freshness::Fresh);
    }

    #[test]
    fn windows_are_named_by_length() {
        assert_eq!(WindowKind::from_minutes(Some(300)), WindowKind::Session);
        assert_eq!(WindowKind::from_minutes(Some(10_080)), WindowKind::Weekly);
        assert_eq!(WindowKind::from_minutes(Some(43_200)), WindowKind::Other);
        assert_eq!(WindowKind::from_minutes(None), WindowKind::Other);
    }

    #[test]
    fn percentages_are_kept_within_bounds() {
        let now = at(10_000);
        assert_eq!(Window::new(None, 140.0, None, now, now).used_percent, 100.0);
        assert_eq!(Window::new(None, -3.0, None, now, now).used_percent, 0.0);
    }

    #[test]
    fn only_agents_present_in_home_are_listed() {
        let home = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        assert!(
            discover_in(home.path(), home.path(), home.path(), cache.path(), at(1))
                .agents
                .is_empty()
        );

        std::fs::create_dir(home.path().join(".codex")).unwrap();
        std::fs::create_dir(home.path().join(".claude")).unwrap();
        let snapshot = discover_in(home.path(), home.path(), home.path(), cache.path(), at(1));
        let listed: Vec<_> = snapshot.agents.iter().map(|a| (a.agent, a.usage.clone())).collect();
        assert_eq!(
            listed,
            vec![(Agent::ClaudeCode, Usage::NoReading), (Agent::Codex, Usage::NoReading)]
        );
    }

    /// OpenCode follows the XDG data home, so a database at the default location is not the one
    /// in use once the data home has been moved.
    #[test]
    fn opencode_is_looked_for_in_the_data_home_given() {
        let home = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let default = home.path().join(".local/share/opencode");
        std::fs::create_dir_all(&default).unwrap();
        std::fs::write(default.join("opencode.db"), b"").unwrap();

        let snapshot = discover_in(home.path(), data.path(), home.path(), cache.path(), at(1));
        assert!(snapshot.agents.is_empty(), "found a database outside the data home");

        std::fs::create_dir_all(data.path().join("opencode")).unwrap();
        std::fs::write(data.path().join("opencode/opencode.db"), b"").unwrap();
        let snapshot = discover_in(home.path(), data.path(), home.path(), cache.path(), at(1));
        let listed: Vec<_> = snapshot.agents.iter().map(|a| a.agent).collect();
        assert_eq!(listed, vec![Agent::OpenCode]);
    }

    /// A statusbar copy carrying everything a real status line document does, with made-up
    /// values that must not survive into the reading.
    const STATUSBAR_DOCUMENT: &str = r#"{
        "session_id": "11111111-2222-3333-4444-555555555555",
        "cwd": "/tmp/private-folder",
        "transcript_path": "/tmp/private-folder/private-transcript.jsonl",
        "model": {"id": "private-model-id", "display_name": "Private Model Name"},
        "workspace": {"current_dir": "/tmp/private-workspace"},
        "rate_limits": {"seven_day": {"used_percentage": 12.5, "resets_at": 1800300000}}
    }"#;

    #[test]
    fn claude_code_is_read_through_the_statusbar_copy_and_keeps_only_the_limits() {
        let home = tempfile::tempdir().unwrap();
        let cache_home = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".claude")).unwrap();
        let copy = cache_home.path().join(CLAUDE_STATUSBAR_FILE);
        std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
        std::fs::write(&copy, STATUSBAR_DOCUMENT).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&copy)
            .unwrap()
            .set_modified(at(1_799_990_000))
            .unwrap();

        let snapshot = discover_in(
            home.path(),
            home.path(),
            cache_home.path(),
            cache.path(),
            at(1_799_990_060),
        );
        let found = &snapshot.agents[0];
        assert_eq!(found.agent, Agent::ClaudeCode);
        assert_eq!(found.source, Some(Source::ClaudeStatusbar));
        let Usage::Allowance(windows) = &found.usage else { panic!("no allowance read") };
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].kind, WindowKind::Weekly);
        assert_eq!(windows[0].used_percent, 12.5);
        assert_eq!(windows[0].observed_at, at(1_799_990_000));

        let kept = format!("{snapshot:?}");
        for private in [
            "11111111",
            "private-folder",
            "private-transcript",
            "private-model",
            "Private Model",
            "private-workspace",
        ] {
            assert!(!kept.contains(private), "{private} survived into the reading");
        }
        assert!(cache.path().read_dir().unwrap().next().is_none(), "the copy was recorded");
    }

    #[test]
    fn a_missing_reading_names_no_source() {
        let home = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".claude")).unwrap();
        let snapshot = discover_in(home.path(), home.path(), home.path(), cache.path(), at(1));
        assert_eq!(snapshot.agents[0].source, None);
    }

    fn answer_at(observed: u64) -> ClaudeAnswer {
        let body = serde_json::json!({"rate_limits": {"limits": [
            {"kind": "weekly_all", "group": "weekly", "percent": 70},
            {"kind": "weekly_scoped", "group": "weekly", "percent": 95,
             "scope": {"model": {"display_name": "Example Model"}}}
        ]}});
        claude_ask::parse_answer(&body, at(observed)).unwrap()
    }

    /// Home with Claude Code present and a statusbar copy stamped at `statusbar_at`.
    fn claude_home(statusbar_at: u64) -> (tempfile::TempDir, tempfile::TempDir) {
        let home = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".claude")).unwrap();
        let copy = home.path().join(CLAUDE_STATUSBAR_FILE);
        std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
        std::fs::write(&copy, STATUSBAR_DOCUMENT).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&copy)
            .unwrap()
            .set_modified(at(statusbar_at))
            .unwrap();
        (home, cache)
    }

    fn claude_reading(meter: &mut UsageMeter, home: &Path, cache: &Path, now: u64) -> AgentUsage {
        meter.read_in(home, home, home, cache, at(now)).agents.remove(0)
    }

    #[test]
    fn a_current_answer_wins_over_a_newer_status_line_file() {
        let (home, cache) = claude_home(1_799_990_029);
        let mut meter = UsageMeter::new();
        meter.keep_claude_answer(Ok(answer_at(1_799_990_000)));

        let found = claude_reading(&mut meter, home.path(), cache.path(), 1_799_990_030);
        assert_eq!(found.source, Some(Source::ClaudeCode));
        let Usage::Allowance(windows) = found.usage else { panic!("no allowance") };
        assert_eq!(windows[1].scope.as_deref(), Some("Example Model"));
    }

    #[test]
    fn an_aged_answer_gives_way_to_a_newer_file() {
        let (home, cache) = claude_home(1_799_990_900);
        let mut meter = UsageMeter::new();
        meter.keep_claude_answer(Ok(answer_at(1_799_990_000)));
        let now = 1_799_990_000 + STALE_AFTER.as_secs() + 60;

        let found = claude_reading(&mut meter, home.path(), cache.path(), now);
        assert_eq!(found.source, Some(Source::ClaudeStatusbar));

        // A file older than the aged answer does not replace it.
        let (home, cache) = claude_home(1_799_989_000);
        let found = claude_reading(&mut meter, home.path(), cache.path(), now);
        assert_eq!(found.source, Some(Source::ClaudeCode));
    }

    #[test]
    fn a_failure_is_reported_beside_the_last_answer() {
        let home = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".claude")).unwrap();
        let mut meter = UsageMeter::new();

        meter.keep_claude_answer(Err(AskError::NeedsSignIn));
        let found = claude_reading(&mut meter, home.path(), cache.path(), 1_799_990_000);
        assert_eq!((found.usage, found.problem), (Usage::NoReading, Some(AskError::NeedsSignIn)));

        meter.keep_claude_answer(Ok(answer_at(1_799_990_000)));
        meter.keep_claude_answer(Err(AskError::Unreachable));
        let found = claude_reading(&mut meter, home.path(), cache.path(), 1_799_990_010);
        assert_eq!(found.source, Some(Source::ClaudeCode));
        assert_eq!(found.problem, Some(AskError::Unreachable));

        meter.keep_claude_answer(Ok(answer_at(1_799_990_020)));
        let found = claude_reading(&mut meter, home.path(), cache.path(), 1_799_990_030);
        assert_eq!(found.problem, None);
    }
}
