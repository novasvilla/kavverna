//! How much of each installed coding agent's allowance is used, read only from files the agents
//! leave on this machine.
//!
//! Nothing here opens a network connection, reads a credential or asks a provider anything. The
//! price of that is freshness: a reading is only as recent as the last time an agent wrote it
//! down, so every window carries the moment it was observed, and a window whose reset time has
//! passed is reported as expired instead of being guessed forward.

mod claude;
mod codex;
mod opencode;
mod private_file;

pub use claude::{
    CACHE_FILE as CLAUDE_CACHE_FILE, StatusLineLimits, StatusLineWindow, parse_status_line,
    record_status_line,
};
pub use opencode::TOTALS_SPAN;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A reading older than this is still the latest truth on disk, but the agent has not been used
/// since, so the meter says so rather than presenting it as current.
pub const STALE_AFTER: Duration = Duration::from_secs(10 * 60);

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
    /// The agent is installed but has left nothing readable yet. For Claude Code that means the
    /// status line passthrough has not run.
    NoReading,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentUsage {
    pub agent: Agent,
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub taken_at: SystemTime,
    /// Only agents found on this machine, in a fixed order.
    pub agents: Vec<AgentUsage>,
}

/// Keeps what it learned from files that have not changed since, so reading every half minute
/// does not decompress the same closed Codex sessions each time.
#[derive(Default)]
pub struct UsageMeter {
    codex: codex::Rollouts,
}

impl UsageMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// `data_dir` is the user's data home, where OpenCode keeps its database, and `cache_dir` is
    /// Kavverna's own cache directory, where the Claude Code passthrough writes.
    pub fn read_in(
        &mut self,
        home: &Path,
        data_dir: &Path,
        cache_dir: &Path,
        now: SystemTime,
    ) -> Snapshot {
        let mut agents = Vec::new();

        if home.join(".claude").is_dir() {
            let usage = match claude::read_cache(cache_dir, now) {
                Some(windows) => Usage::Allowance(windows),
                None => Usage::NoReading,
            };
            agents.push(AgentUsage { agent: Agent::ClaudeCode, usage });
        }

        let codex_home = home.join(".codex");
        if codex_home.is_dir() {
            let usage = match self.codex.latest(&codex_home, now) {
                Some(windows) => Usage::Allowance(windows),
                None => Usage::NoReading,
            };
            agents.push(AgentUsage { agent: Agent::Codex, usage });
        }

        let opencode_db = opencode::database(data_dir);
        if opencode_db.is_file() {
            let usage = match opencode::totals(&opencode_db, now) {
                Some(totals) => Usage::Totals(totals),
                None => Usage::NoReading,
            };
            agents.push(AgentUsage { agent: Agent::OpenCode, usage });
        }

        Snapshot { taken_at: now, agents }
    }

    pub fn read(&mut self, now: SystemTime) -> Snapshot {
        match (directories::BaseDirs::new(), cache_dir()) {
            (Some(base), Some(cache)) => {
                self.read_in(base.home_dir(), base.data_dir(), &cache, now)
            }
            _ => Snapshot { taken_at: now, agents: Vec::new() },
        }
    }
}

pub fn discover_in(home: &Path, data_dir: &Path, cache_dir: &Path, now: SystemTime) -> Snapshot {
    UsageMeter::new().read_in(home, data_dir, cache_dir, now)
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
        assert!(discover_in(home.path(), home.path(), cache.path(), at(1)).agents.is_empty());

        std::fs::create_dir(home.path().join(".codex")).unwrap();
        std::fs::create_dir(home.path().join(".claude")).unwrap();
        let snapshot = discover_in(home.path(), home.path(), cache.path(), at(1));
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

        let snapshot = discover_in(home.path(), data.path(), cache.path(), at(1));
        assert!(snapshot.agents.is_empty(), "found a database outside the data home");

        std::fs::create_dir_all(data.path().join("opencode")).unwrap();
        std::fs::write(data.path().join("opencode/opencode.db"), b"").unwrap();
        let snapshot = discover_in(home.path(), data.path(), cache.path(), at(1));
        let listed: Vec<_> = snapshot.agents.iter().map(|a| a.agent).collect();
        assert_eq!(listed, vec![Agent::OpenCode]);
    }
}
