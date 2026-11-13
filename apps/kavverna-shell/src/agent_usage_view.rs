use crate::agent_usage_state;
use agent_usage::{Agent, Snapshot, TokenTotals, Usage, Window, WindowKind};
use cxx_qt::Threading;
use cxx_qt_lib::{QList, QString, QStringList};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
        type QList_f64 = cxx_qt_lib::QList<f64>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, has_reading)]
        #[qproperty(i32, agent_count)]
        #[qproperty(QString, empty_text)]
        #[qproperty(QStringList, agent_ids)]
        #[qproperty(QStringList, agent_titles)]
        #[qproperty(QStringList, agent_kinds)]
        #[qproperty(QStringList, agent_notes)]
        #[qproperty(QStringList, agent_ages)]
        #[qproperty(QStringList, agent_totals)]
        #[qproperty(QStringList, agent_costs)]
        #[qproperty(QList_i32, window_agents)]
        #[qproperty(QStringList, window_labels)]
        #[qproperty(QList_f64, window_used)]
        #[qproperty(QStringList, window_used_texts)]
        #[qproperty(QStringList, window_reset_texts)]
        #[qproperty(QStringList, window_age_texts)]
        #[qproperty(QStringList, window_states)]
        type AgentUsageView = super::AgentUsageViewRust;
    }

    impl cxx_qt::Threading for AgentUsageView {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut AgentUsageView>);
        #[qinvokable]
        fn refresh(self: Pin<&mut AgentUsageView>);
    }
}

use core::pin::Pin;

static VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::AgentUsageView>>> = Mutex::new(None);

pub const EMPTY_TEXT: &str = "No coding agent found. Kavverna looks for Claude Code, Codex and OpenCode in your home folder.";

pub const TOTALS_NOTE: &str = "Totals over the last 24 hours, not an allowance.";

#[derive(Default)]
pub struct AgentUsageViewRust {
    has_reading: bool,
    agent_count: i32,
    empty_text: QString,
    agent_ids: QStringList,
    agent_titles: QStringList,
    agent_kinds: QStringList,
    agent_notes: QStringList,
    agent_ages: QStringList,
    agent_totals: QStringList,
    agent_costs: QStringList,
    window_agents: QList<i32>,
    window_labels: QStringList,
    window_used: QList<f64>,
    window_used_texts: QStringList,
    window_reset_texts: QStringList,
    window_age_texts: QStringList,
    window_states: QStringList,
}

/// Named by length, since that is what a person knows the window by.
pub fn window_label(kind: WindowKind, minutes: Option<u32>) -> String {
    match (kind, minutes) {
        (WindowKind::Weekly, _) => "Weekly".into(),
        (_, Some(m)) if m > 0 && m % 1440 == 0 => format!("{}-day", m / 1440),
        (_, Some(m)) if m > 0 && m % 60 == 0 => format!("{}-hour", m / 60),
        (_, Some(m)) if m > 0 => format!("{m}-minute"),
        _ => "Allowance".into(),
    }
}

/// Rounded down, so a window never looks further from its reset than it is.
fn span(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    match minutes {
        0..=59 => format!("{minutes}m"),
        60..=1439 if minutes % 60 == 0 => format!("{}h", minutes / 60),
        60..=1439 => format!("{}h {}m", minutes / 60, minutes % 60),
        _ if minutes / 60 % 24 == 0 => format!("{}d", minutes / 1440),
        _ => format!("{}d {}h", minutes / 1440, minutes / 60 % 24),
    }
}

/// A reset within the last day is named by the clock, which is how a person would check it;
/// one longer ago by how long, since a bare time would read as today.
pub fn reset_text(
    resets_at: Option<SystemTime>,
    now: SystemTime,
    clock: impl Fn(SystemTime) -> Option<String>,
) -> String {
    let Some(reset) = resets_at else {
        return "reset time unknown".into();
    };
    match reset.duration_since(now) {
        Ok(left) if left < Duration::from_secs(60) => "resets in under a minute".into(),
        Ok(left) => format!("resets in {}", span(left)),
        Err(past) if past.duration() < Duration::from_secs(24 * 3600) => match clock(reset) {
            Some(time) => format!("reset at {time}"),
            None => format!("reset {} ago", span(past.duration())),
        },
        Err(past) => format!("reset {} ago", span(past.duration())),
    }
}

pub fn age_text(observed_at: SystemTime, now: SystemTime) -> String {
    match now.duration_since(observed_at) {
        Ok(age) if age >= Duration::from_secs(60) => format!("updated {} ago", span(age)),
        _ => "updated just now".into(),
    }
}

fn local_clock(at: SystemTime) -> Option<String> {
    let seconds =
        libc::time_t::try_from(at.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs()).ok()?;
    // SAFETY: `localtime_r` writes only into the `tm` it is handed, which lives on this stack.
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    let filled = unsafe { libc::localtime_r(&seconds, &mut local) };
    (!filled.is_null()).then(|| format!("{:02}:{:02}", local.tm_hour, local.tm_min))
}

fn count(value: u64) -> String {
    let scaled = |divisor: f64, unit: &str| {
        let text = format!("{:.1}", value as f64 / divisor);
        format!("{}{unit}", text.strip_suffix(".0").unwrap_or(&text))
    };
    match value {
        0..1_000 => value.to_string(),
        1_000..1_000_000 => scaled(1e3, "k"),
        1_000_000..1_000_000_000 => scaled(1e6, "M"),
        _ => scaled(1e9, "B"),
    }
}

pub fn totals_text(totals: &TokenTotals) -> String {
    let replies = match totals.replies {
        1 => "1 reply".to_owned(),
        n => format!("{n} replies"),
    };
    let mut parts = vec![
        replies,
        format!("{} in", count(totals.input)),
        format!("{} out", count(totals.output)),
    ];
    if totals.reasoning > 0 {
        parts.push(format!("{} reasoning", count(totals.reasoning)));
    }
    if totals.cache_read > 0 {
        parts.push(format!("{} cached", count(totals.cache_read)));
    }
    parts.join(" · ")
}

/// Empty for nothing spent, which is what a local model reports, so no "$0.00" is shown for
/// something that was never priced.
pub fn cost_text(cost: f64) -> String {
    match cost {
        c if !c.is_finite() || c <= 0.0 => String::new(),
        c if c < 0.01 => "under $0.01".into(),
        c => format!("${c:.2}"),
    }
}

/// What to do about an agent that is installed but has left nothing to read.
pub fn waiting_note(agent: Agent) -> &'static str {
    match agent {
        Agent::ClaudeCode => {
            "Set Claude Code's status line command to kavverna-shell --claude-statusline and \
             the allowance appears after its next reply on a subscription plan."
        }
        Agent::Codex => "Codex writes its allowance down after its next reply.",
        Agent::OpenCode => "OpenCode's database could not be read.",
    }
}

fn strings(values: Vec<String>) -> QStringList {
    let mut list = QStringList::default();
    for value in &values {
        list.append(QString::from(value));
    }
    list
}

impl qobject::AgentUsageView {
    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Ok(mut view) = VIEW.lock() {
            *view = Some(thread);
        }
        self.apply(agent_usage_state::latest(), SystemTime::now());
    }

    fn refresh(self: Pin<&mut Self>) {
        agent_usage_state::wake();
    }

    fn apply(mut self: Pin<&mut Self>, snapshot: Option<Snapshot>, now: SystemTime) {
        self.as_mut().set_empty_text(QString::from(EMPTY_TEXT));
        self.as_mut().set_has_reading(snapshot.is_some());
        let agents = snapshot.map(|s| s.agents).unwrap_or_default();

        let (mut ids, mut titles, mut kinds, mut notes, mut ages, mut totals, mut costs) =
            (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
        let mut window_agents = QList::<i32>::default();
        let mut window_used = QList::<f64>::default();
        let (mut labels, mut used, mut resets, mut window_ages, mut states) =
            (vec![], vec![], vec![], vec![], vec![]);

        for (index, found) in agents.iter().enumerate() {
            ids.push(found.agent.id().to_owned());
            titles.push(found.agent.title().to_owned());
            let (kind, note, age, total, cost) = match &found.usage {
                Usage::NoReading => (
                    "no-reading",
                    waiting_note(found.agent).to_owned(),
                    String::new(),
                    "".into(),
                    "".into(),
                ),
                Usage::Allowance(windows) => {
                    for window in windows {
                        window_agents.append(index as i32);
                        window_used.append(window.used_percent);
                        labels.push(window_label(window.kind, window.minutes));
                        used.push(format!("{:.0}%", window.used_percent));
                        resets.push(reset_text(window.resets_at, now, local_clock));
                        window_ages.push(age_text(window.observed_at, now));
                        states.push(
                            agent_usage_state::freshness_name(agent_usage_state::judged(
                                window, now,
                            ))
                            .to_owned(),
                        );
                    }
                    let newest = windows.iter().map(|w: &Window| w.observed_at).max();
                    let age = newest.map(|at| age_text(at, now)).unwrap_or_default();
                    ("allowance", String::new(), age, String::new(), String::new())
                }
                Usage::Totals(spent) => (
                    "totals",
                    TOTALS_NOTE.to_owned(),
                    spent.observed_at.map(|at| age_text(at, now)).unwrap_or_default(),
                    totals_text(spent),
                    cost_text(spent.cost),
                ),
            };
            kinds.push(kind.to_owned());
            notes.push(note);
            ages.push(age);
            totals.push(total);
            costs.push(cost);
        }

        self.as_mut().set_agent_ids(strings(ids));
        self.as_mut().set_agent_titles(strings(titles));
        self.as_mut().set_agent_kinds(strings(kinds));
        self.as_mut().set_agent_notes(strings(notes));
        self.as_mut().set_agent_ages(strings(ages));
        self.as_mut().set_agent_totals(strings(totals));
        self.as_mut().set_agent_costs(strings(costs));
        self.as_mut().set_window_agents(window_agents);
        self.as_mut().set_window_labels(strings(labels));
        self.as_mut().set_window_used(window_used);
        self.as_mut().set_window_used_texts(strings(used));
        self.as_mut().set_window_reset_texts(strings(resets));
        self.as_mut().set_window_age_texts(strings(window_ages));
        self.as_mut().set_window_states(strings(states));
        // Last, so a binding on the count sees every list already filled in.
        self.as_mut().set_agent_count(agents.len() as i32);
    }
}

pub fn publish() {
    let Ok(view) = VIEW.lock() else {
        return;
    };
    if let Some(thread) = view.as_ref() {
        let _ = thread.queue(|view| view.apply(agent_usage_state::latest(), SystemTime::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn noon(_: SystemTime) -> Option<String> {
        Some("14:30".into())
    }

    #[test]
    fn windows_are_labelled_by_their_length() {
        assert_eq!(window_label(WindowKind::Session, Some(300)), "5-hour");
        assert_eq!(window_label(WindowKind::Weekly, Some(10_080)), "Weekly");
        assert_eq!(window_label(WindowKind::Other, Some(43_200)), "30-day");
        assert_eq!(window_label(WindowKind::Session, Some(90)), "90-minute");
        assert_eq!(window_label(WindowKind::Other, None), "Allowance");
    }

    #[test]
    fn a_coming_reset_is_counted_down() {
        let now = at(100_000);
        let reset = |s| reset_text(Some(now + Duration::from_secs(s)), now, noon);
        assert_eq!(reset(2 * 3600 + 10 * 60 + 59), "resets in 2h 10m");
        assert_eq!(reset(3 * 3600), "resets in 3h");
        assert_eq!(reset(45 * 60), "resets in 45m");
        assert_eq!(reset(30), "resets in under a minute");
        assert_eq!(reset(3 * 86_400 + 4 * 3600), "resets in 3d 4h");
        assert_eq!(reset_text(None, now, noon), "reset time unknown");
    }

    #[test]
    fn a_past_reset_is_named_by_the_clock_within_a_day() {
        let now = at(1_000_000);
        assert_eq!(reset_text(Some(now - Duration::from_secs(600)), now, noon), "reset at 14:30");
        assert_eq!(
            reset_text(Some(now - Duration::from_secs(2 * 86_400)), now, noon),
            "reset 2d ago"
        );
        assert_eq!(
            reset_text(Some(now - Duration::from_secs(600)), now, |_| None),
            "reset 10m ago"
        );
    }

    #[test]
    fn the_local_clock_reads_hours_and_minutes() {
        let shown = local_clock(at(1_800_000_000)).unwrap();
        assert_eq!(shown.len(), 5);
        assert_eq!(shown.as_bytes()[2], b':');
    }

    #[test]
    fn ages_say_how_long_ago() {
        let now = at(100_000);
        assert_eq!(age_text(now - Duration::from_secs(20), now), "updated just now");
        assert_eq!(age_text(now - Duration::from_secs(4 * 60 + 30), now), "updated 4m ago");
        assert_eq!(age_text(now - Duration::from_secs(2 * 3600 + 300), now), "updated 2h 5m ago");
        assert_eq!(age_text(now + Duration::from_secs(60), now), "updated just now");
    }

    #[test]
    fn totals_are_short_and_cost_only_shows_when_spent() {
        let spent = TokenTotals {
            replies: 12,
            input: 1_234_567,
            output: 340_000,
            cache_read: 2_000_000,
            ..Default::default()
        };
        assert_eq!(totals_text(&spent), "12 replies · 1.2M in · 340k out · 2M cached");
        assert_eq!(
            totals_text(&TokenTotals { replies: 1, ..Default::default() }),
            "1 reply · 0 in · 0 out"
        );
        assert_eq!(cost_text(0.0), "");
        assert_eq!(cost_text(f64::NAN), "");
        assert_eq!(cost_text(0.004), "under $0.01");
        assert_eq!(cost_text(1.5), "$1.50");
    }

    #[test]
    fn claude_code_waiting_says_how_to_start_the_reading() {
        assert!(waiting_note(Agent::ClaudeCode).contains("--claude-statusline"));
    }
}
