//! The applications behind each meter of the monitoring page, and the ports the user's programs
//! listen on.
//!
//! One "Apps using …" fold is open at a time, and only that one is measured, only while the
//! panel shows: walking every process every two seconds is the most expensive thing the monitor
//! does, and nobody is reading the answer when the fold is shut. The order of the rows is kept
//! here rather than in QML, so it can be held still under the pointer and changed only when a
//! change is real, and both rules are tested without a display.

use crate::panel;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use system_monitor::{
    Action, ActionReport, ApplicationKey, ApplicationUse, CardUse, CpuShare, DiskRate, Exposure,
    GpuRole, ListeningSocket, MemoryUse, Outcome, ProcessControl, ProcessKey, ProcessSampler,
    Reading, Refusal, Transport,
};

/// Rows a closed fold shows, and an expanded one.
pub const SHOWN: usize = 8;
pub const SHOWN_EXPANDED: usize = 15;

/// Asked of the sampler beyond what is shown, so the row that would come ninth is still known
/// and the eighth place is decided by the same patience as every other.
const ASKED: usize = 20;

/// A lower row takes the place of the one above only when it is ahead by more than a tenth,
/// twice running. Without it two applications at 3.1% and 3.2% trade places every sample and the
/// row under the pointer is never the one that was aimed at.
const OVERTAKE: f64 = 1.10;
const CONFIRMATIONS: u8 = 2;

/// How long what an action did stays on its row. Long enough to read, and to keep an ended
/// application's row in place for the sentence to sit on.
const FEEDBACK_KEPT: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resource {
    Cpu,
    Memory,
    Graphics,
    Disk,
}

impl Resource {
    /// The names QML and the bus use.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "cpu" => Some(Self::Cpu),
            "memory" => Some(Self::Memory),
            "gpu" => Some(Self::Graphics),
            "disk" => Some(Self::Disk),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Graphics => "gpu",
            Self::Disk => "disk",
        }
    }

    fn idle_text(self) -> &'static str {
        match self {
            Self::Cpu => "No app is using the processor now",
            Self::Memory => "No app is holding memory now",
            Self::Graphics => "No app is using the GPU now",
            Self::Disk => "No app is reading or writing now",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Closed,
    /// Rates need two readings and only one has been taken.
    Measuring,
    Ready,
    /// Measured, and nothing is spending this resource.
    Idle,
    /// The card cannot say which processes use it.
    Unavailable,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Measuring => "measuring",
            Self::Ready => "ready",
            Self::Idle => "idle",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackKind {
    Working,
    Ended,
    /// Asked to end and did not: the moment to offer Force end.
    StillRunning,
    Refused,
    Failed,
}

impl FeedbackKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Ended => "ended",
            Self::StillRunning => "still_running",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Feedback {
    pub kind: FeedbackKind,
    pub text: String,
}

/// One application as a fold shows it, every figure already in words.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Row {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub value: String,
    /// Video memory in the graphics fold, empty elsewhere.
    pub secondary: String,
    /// The process the others descend from, which is what Copy PID copies.
    pub pid: u32,
    pub processes: usize,
    /// False for a row kept in place although the latest reading no longer lists it: held
    /// under the pointer, or carrying what an action did.
    pub present: bool,
    pub feedback: Option<Feedback>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shown {
    pub resource: Option<Resource>,
    /// The card asked for when the reading was taken, so rows of the card just left are not
    /// shown under the name of the one just chosen.
    pub card: String,
    pub phase: Phase,
    pub status_text: String,
    /// What the rows leave of the whole, where there is a whole: processor load only.
    pub summary_text: String,
    pub rows: Vec<Row>,
}

/// What decides an order: the figure itself first, and a second one only between rows level
/// on the first, which is video memory between applications not using the card.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Measure {
    pub primary: f64,
    pub secondary: f64,
}

impl Measure {
    fn ahead_of(self, other: Self) -> std::cmp::Ordering {
        self.primary.total_cmp(&other.primary).then(self.secondary.total_cmp(&other.secondary))
    }
}

fn exceeds(lower: f64, upper: f64) -> bool {
    lower > 0.0 && lower > upper * OVERTAKE
}

fn overtakes(lower: Measure, upper: Measure) -> bool {
    exceeds(lower.primary, upper.primary)
        || (lower.primary == upper.primary && exceeds(lower.secondary, upper.secondary))
}

/// The order of a fold's rows across samples.
#[derive(Debug, Default)]
pub struct Ranking {
    order: Vec<String>,
    /// How many samples running the first key has been ahead of the second, which sits above it.
    streaks: HashMap<(String, String), u8>,
}

impl Ranking {
    /// The new order. Held, the rows keep their places, rows the sample dropped stay where they
    /// were and new ones join at the bottom. Otherwise a row gone from the sample leaves unless
    /// `keep` names it, a new row enters where its figure puts it, and a row overtakes the one
    /// above only after leading it by more than a tenth for two samples in a row.
    pub fn update(
        &mut self,
        sample: &[(String, Measure)],
        holding: bool,
        keep: &HashSet<String>,
    ) -> Vec<String> {
        let measures: HashMap<&str, Measure> =
            sample.iter().map(|(key, measure)| (key.as_str(), *measure)).collect();
        let measure = |key: &str| measures.get(key).copied().unwrap_or_default();

        let mut order: Vec<String> = self
            .order
            .iter()
            .filter(|key| holding || measures.contains_key(key.as_str()) || keep.contains(*key))
            .cloned()
            .collect();
        let placed: HashSet<String> = order.iter().cloned().collect();
        let mut newcomers: Vec<&(String, Measure)> =
            sample.iter().filter(|(key, _)| !placed.contains(key)).collect();
        newcomers.sort_by(|left, right| right.1.ahead_of(left.1).then(left.0.cmp(&right.0)));
        for (key, value) in newcomers {
            let at = if holding {
                order.len()
            } else {
                order
                    .iter()
                    .position(|above| value.ahead_of(measure(above)).is_gt())
                    .unwrap_or(order.len())
            };
            order.insert(at, key.clone());
        }

        if holding {
            // A lead built up while the order could not change would land all at once on the
            // first sample after the pointer leaves, which is the jump holding exists to stop.
            self.streaks.clear();
        } else {
            let mut streaks = HashMap::new();
            for (above, upper) in order.iter().enumerate() {
                for lower in &order[above + 1..] {
                    if overtakes(measure(lower), measure(upper)) {
                        let pair = (lower.clone(), upper.clone());
                        let run = self.streaks.get(&pair).copied().unwrap_or(0) + 1;
                        streaks.insert(pair, run);
                    }
                }
            }
            for at in 1..order.len() {
                let mut climbing = at;
                while climbing > 0 {
                    let pair = (order[climbing].clone(), order[climbing - 1].clone());
                    if streaks.get(&pair).is_none_or(|run| *run < CONFIRMATIONS) {
                        break;
                    }
                    streaks.remove(&pair);
                    order.swap(climbing, climbing - 1);
                    climbing -= 1;
                }
            }
            self.streaks = streaks;
        }
        self.order = order.clone();
        order
    }
}

/// A key QML can hold and hand back: the application key, spelled out.
pub fn key_text(key: &ApplicationKey) -> String {
    match key {
        ApplicationKey::Entry(id) => format!("entry:{id}"),
        ApplicationKey::Unit(unit) => format!("unit:{unit}"),
        ApplicationKey::Named(name) => format!("named:{name}"),
    }
}

pub fn key_from(text: &str) -> Option<ApplicationKey> {
    let (kind, rest) = text.split_once(':')?;
    match kind {
        "entry" => Some(ApplicationKey::Entry(rest.to_owned())),
        "unit" => Some(ApplicationKey::Unit(rest.to_owned())),
        "named" => Some(ApplicationKey::Named(rest.to_owned())),
        _ => None,
    }
}

pub fn percent(share: f32) -> String {
    format!("{:.1}%", share * 100.0)
}

pub fn memory_text(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB { format!("{:.1} GiB", bytes / GIB) } else { format!("{:.0} MiB", bytes / MIB) }
}

/// Rates in decimal units, the way a download speed is usually read.
pub fn rate_text(per_second: u64) -> String {
    let rate = per_second as f64;
    if rate >= 1e9 {
        format!("{:.1} GB/s", rate / 1e9)
    } else if rate >= 1e6 {
        format!("{:.1} MB/s", rate / 1e6)
    } else if rate >= 1e3 {
        format!("{:.0} KB/s", rate / 1e3)
    } else {
        format!("{per_second} B/s")
    }
}

pub fn disk_text(rate: &DiskRate) -> String {
    format!("↓ {} ↑ {}", rate_text(rate.read_per_second), rate_text(rate.written_per_second))
}

/// The member whose parent is outside the application, lowest pid first.
fn leader<V>(application: &ApplicationUse<V>) -> u32 {
    let pids: HashSet<u32> = application.members.iter().map(|member| member.key.pid).collect();
    application
        .members
        .iter()
        .filter(|member| !pids.contains(&member.parent))
        .map(|member| member.key.pid)
        .min()
        .or_else(|| pids.iter().copied().min())
        .unwrap_or(0)
}

/// One sampled application, worded and measured.
#[derive(Debug, Clone, PartialEq)]
pub struct Sampled {
    pub row: Row,
    pub measure: Measure,
    pub pids: Vec<u32>,
}

fn sampled<V>(
    application: &ApplicationUse<V>,
    value: String,
    secondary: String,
    measure: Measure,
) -> Sampled {
    Sampled {
        row: Row {
            key: key_text(&application.key),
            name: application.name.clone(),
            icon: application.icon.clone().unwrap_or_default(),
            value,
            secondary,
            pid: leader(application),
            processes: application.members.len(),
            present: true,
            feedback: None,
        },
        measure,
        pids: application.members.iter().map(|member| member.key.pid).collect(),
    }
}

pub fn cpu_row(application: &ApplicationUse<CpuShare>) -> Sampled {
    let share = application.value.of_machine;
    sampled(
        application,
        percent(share),
        String::new(),
        Measure { primary: f64::from(share), secondary: 0.0 },
    )
}

pub fn memory_row(application: &ApplicationUse<MemoryUse>) -> Sampled {
    let bytes = application.value.bytes;
    sampled(
        application,
        memory_text(bytes),
        String::new(),
        Measure { primary: bytes as f64, secondary: 0.0 },
    )
}

pub fn card_row(application: &ApplicationUse<CardUse>) -> Sampled {
    let used = application.value;
    sampled(
        application,
        used.busy.map_or_else(|| "—".to_owned(), |busy| format!("{:.0}%", busy * 100.0)),
        used.vram.map(memory_text).unwrap_or_default(),
        Measure {
            primary: f64::from(used.busy.unwrap_or(0.0)),
            secondary: used.vram.unwrap_or(0) as f64,
        },
    )
}

pub fn disk_row(application: &ApplicationUse<DiskRate>) -> Sampled {
    let rate = application.value;
    sampled(
        application,
        disk_text(&rate),
        String::new(),
        Measure {
            primary: (rate.read_per_second + rate.written_per_second) as f64,
            secondary: 0.0,
        },
    )
}

/// What one reading of a resource came to, before any ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    pub phase: Phase,
    pub summary_text: String,
    pub rows: Vec<Sampled>,
    /// Whatever the bus reports beside the rows: the machine's load, the card's name.
    pub card: Option<String>,
    pub machine_load: Option<f32>,
}

impl Measured {
    fn without_rows(phase: Phase) -> Self {
        Self {
            phase,
            summary_text: String::new(),
            rows: Vec::new(),
            card: None,
            machine_load: None,
        }
    }
}

/// The card a graphics fold reads when none was named: the discrete one, which is what the
/// monitor's own readout defaults to.
pub fn preferred_card(sampler: &ProcessSampler) -> Option<String> {
    let cards = sampler.cards();
    cards
        .iter()
        .find(|card| card.role == GpuRole::Discrete)
        .or_else(|| cards.first())
        .map(|card| card.name.clone())
}

/// Reads one resource once. `cards` is opened the first time the graphics fold is read and
/// kept: opening it is opening NVML, which the processor, memory and disk folds never need.
pub fn measure(
    resource: Resource,
    card: &str,
    plain: &mut ProcessSampler,
    cards: &mut Option<ProcessSampler>,
) -> Measured {
    let ready = |rows: Vec<Sampled>| Measured {
        phase: if rows.is_empty() { Phase::Idle } else { Phase::Ready },
        summary_text: String::new(),
        rows,
        card: None,
        machine_load: None,
    };
    match resource {
        Resource::Cpu => match plain.cpu(ASKED) {
            Reading::Priming => Measured::without_rows(Phase::Measuring),
            Reading::Ready(fold) => {
                let mut measured = ready(fold.applications.iter().map(cpu_row).collect());
                if fold.elsewhere >= 0.001 {
                    measured.summary_text = format!("Everything else {}", percent(fold.elsewhere));
                }
                measured.machine_load = Some(fold.machine_load);
                measured
            }
        },
        Resource::Memory => {
            ready(plain.memory(ASKED).applications.iter().map(memory_row).collect())
        }
        Resource::Disk => match plain.disk(ASKED) {
            Reading::Priming => Measured::without_rows(Phase::Measuring),
            Reading::Ready(fold) => ready(fold.applications.iter().map(disk_row).collect()),
        },
        Resource::Graphics => {
            let sampler = cards.get_or_insert_with(ProcessSampler::open);
            let name =
                if card.is_empty() { preferred_card(sampler) } else { Some(card.to_owned()) };
            let Some(fold) = name.as_deref().and_then(|name| sampler.graphics(name, ASKED)) else {
                let mut measured = Measured::without_rows(Phase::Unavailable);
                measured.card = name;
                return measured;
            };
            let mut measured = ready(fold.applications.iter().map(card_row).collect());
            if fold.busy_priming && measured.rows.is_empty() {
                measured.phase = Phase::Measuring;
            }
            measured.card = Some(fold.card.name);
            measured
        }
    }
}

pub fn status_text(resource: Resource, phase: Phase) -> String {
    match phase {
        Phase::Closed | Phase::Ready => String::new(),
        Phase::Measuring => "Measuring…".to_owned(),
        Phase::Idle => resource.idle_text().to_owned(),
        Phase::Unavailable => "This card does not say which apps use it".to_owned(),
    }
}

/// Why nothing was sent, in words a person can act on.
pub fn refusal_words(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Gone | Refusal::Replaced => "Already ended".to_owned(),
        Refusal::NotYours => "Belongs to another user".to_owned(),
        Refusal::Init => "Part of the system".to_owned(),
        Refusal::Itself => "This is Kavverna itself".to_owned(),
        Refusal::Ancestor => "Kavverna runs inside it".to_owned(),
        Refusal::Session(_) => "Part of the desktop session".to_owned(),
        Refusal::Member { reason, .. } => refusal_words(reason),
    }
}

/// What an action came to, for the row it was asked from.
pub fn feedback_for(report: &ActionReport) -> Feedback {
    let gone = |outcome: &Outcome| {
        matches!(outcome, Outcome::Ended | Outcome::Refused(Refusal::Gone | Refusal::Replaced))
    };
    let outcomes = || report.outcomes.iter().map(|(_, outcome)| outcome);
    if let Some(Outcome::Refused(refusal)) =
        outcomes().find(|outcome| matches!(outcome, Outcome::Refused(_)) && !gone(outcome))
    {
        return Feedback { kind: FeedbackKind::Refused, text: refusal_words(refusal) };
    }
    if let Some(Outcome::Failed(error)) =
        outcomes().find(|outcome| matches!(outcome, Outcome::Failed(_)))
    {
        return Feedback {
            kind: FeedbackKind::Failed,
            text: format!("Could not be ended: {error}"),
        };
    }
    if outcomes().any(|outcome| matches!(outcome, Outcome::StillRunning)) {
        return Feedback { kind: FeedbackKind::StillRunning, text: "Still running".to_owned() };
    }
    // Every member gone, whether by this signal or before it, is the result that was asked for.
    Feedback { kind: FeedbackKind::Ended, text: "Ended".to_owned() }
}

/// What the worker does next, given whether a fold is open and whether the panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing to measure: wait for a fold to open.
    Sleep,
    /// The panel hid with a fold open. The fold closes with it, so the next opening starts
    /// from a closed page rather than from a reading minutes old.
    Close,
    Sample,
}

pub fn next_step(fold_open: bool, panel_open: bool) -> Step {
    match (fold_open, panel_open) {
        (false, _) => Step::Sleep,
        (true, false) => Step::Close,
        (true, true) => Step::Sample,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Wanted {
    resource: Option<Resource>,
    /// Empty for the preferred card.
    card: String,
    holding: bool,
}

enum Message {
    Look,
    End { key: String, action: Action },
}

static WANTED: Mutex<Wanted> =
    Mutex::new(Wanted { resource: None, card: String::new(), holding: false });
static SHOWN_NOW: Mutex<Option<Shown>> = Mutex::new(None);
static FEEDBACK: Mutex<Option<HashMap<String, (Feedback, Instant)>>> = Mutex::new(None);
static MESSAGES: Mutex<Option<Sender<Message>>> = Mutex::new(None);

fn held<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn send(message: Message) -> bool {
    match held(&MESSAGES).as_ref() {
        Some(sender) => sender.send(message).is_ok(),
        None => false,
    }
}

/// Opens one fold, or closes them all with `None`. `card` names the card for the graphics
/// fold, empty for the discrete one.
pub fn want(resource: Option<Resource>, card: &str) {
    {
        let mut wanted = held(&WANTED);
        if wanted.resource != resource {
            // A pointer that left with the old fold never said so to this one.
            wanted.holding = false;
            if let Some(said) = held(&FEEDBACK).as_mut() {
                said.retain(|_, (feedback, _)| feedback.kind == FeedbackKind::Working);
            }
        }
        wanted.resource = resource;
        wanted.card = card.to_owned();
    }
    send(Message::Look);
}

pub fn hold(holding: bool) {
    held(&WANTED).holding = holding;
}

pub fn wanted_resource() -> Option<Resource> {
    held(&WANTED).resource
}

fn set_feedback(key: &str, feedback: Feedback) {
    held(&FEEDBACK)
        .get_or_insert_with(HashMap::new)
        .insert(key.to_owned(), (feedback, Instant::now()));
}

/// What is shown now, with every action's result still fresh enough to read on its row.
pub fn get() -> Shown {
    let mut shown = held(&SHOWN_NOW).clone().unwrap_or_default();
    let mut feedback = held(&FEEDBACK);
    let feedback = feedback.get_or_insert_with(HashMap::new);
    // Still running stays until another action replaces it, since it is what offers Force end.
    feedback.retain(|_, (said, at)| {
        matches!(said.kind, FeedbackKind::Working | FeedbackKind::StillRunning)
            || at.elapsed() < FEEDBACK_KEPT
    });
    for row in &mut shown.rows {
        row.feedback = feedback.get(&row.key).map(|(said, _)| said.clone());
    }
    shown
}

/// Ends an application, every process of it as it stands when the worker gets to it. Answers
/// false when nothing is running to do it.
pub fn end(key: &str, force: bool) -> bool {
    let action = if force { Action::ForceStop } else { Action::Terminate };
    if !send(Message::End { key: key.to_owned(), action }) {
        return false;
    }
    let text = if force { "Forcing it to end…" } else { "Ending…" };
    set_feedback(key, Feedback { kind: FeedbackKind::Working, text: text.to_owned() });
    true
}

fn publish(shown: Shown, on_change: fn()) {
    *held(&SHOWN_NOW) = Some(shown);
    on_change();
}

struct Worker {
    plain: ProcessSampler,
    cards: Option<ProcessSampler>,
    ranking: Ranking,
    measured_for: Option<(Resource, String)>,
    last: HashMap<String, Row>,
    due: Option<Instant>,
}

impl Worker {
    fn handle(&mut self, message: Message, on_change: fn()) {
        let Message::End { key, action } = message else {
            return;
        };
        let members = key_from(&key).map(|key| self.plain.members_of(&key)).unwrap_or_default();
        std::thread::spawn(move || {
            let report = ProcessControl::for_this_session().act_on_application(&members, action);
            tracing::info!(%key, ?action, outcomes = ?report.outcomes, "acted on an application");
            set_feedback(&key, feedback_for(&report));
            send(Message::Look);
            on_change();
        });
    }

    fn sample(&mut self, wanted: &Wanted, tick: Duration, on_change: fn()) {
        let Some(resource) = wanted.resource else {
            return;
        };
        let target = (resource, wanted.card.clone());
        if self.measured_for.as_ref() != Some(&target) {
            self.ranking = Ranking::default();
            self.last.clear();
            self.measured_for = Some(target);
        }
        let measured = measure(resource, &wanted.card, &mut self.plain, &mut self.cards);
        let fresh: HashSet<String> = held(&FEEDBACK)
            .as_ref()
            .map(|said| {
                said.iter()
                    .filter(|(_, (_, at))| at.elapsed() < FEEDBACK_KEPT)
                    .map(|(key, _)| key.clone())
                    .collect()
            })
            .unwrap_or_default();
        let sample: Vec<(String, Measure)> =
            measured.rows.iter().map(|row| (row.row.key.clone(), row.measure)).collect();
        let order = self.ranking.update(&sample, wanted.holding, &fresh);
        let mut rows_now: HashMap<String, Row> =
            measured.rows.into_iter().map(|row| (row.row.key.clone(), row.row)).collect();
        let rows: Vec<Row> = order
            .iter()
            .take(SHOWN_EXPANDED)
            .filter_map(|key| {
                rows_now.remove(key).or_else(|| {
                    self.last.get(key).map(|before| Row {
                        value: "—".to_owned(),
                        secondary: String::new(),
                        present: false,
                        ..before.clone()
                    })
                })
            })
            .collect();
        self.last = rows.iter().map(|row| (row.key.clone(), row.clone())).collect();
        let phase = match measured.phase {
            Phase::Idle if !rows.is_empty() => Phase::Ready,
            phase => phase,
        };
        publish(
            Shown {
                resource: Some(resource),
                card: wanted.card.clone(),
                phase,
                status_text: status_text(resource, phase),
                summary_text: measured.summary_text,
                rows,
            },
            on_change,
        );
        self.due = Some(Instant::now() + tick);
    }
}

/// Samples the open fold every `tick` while the panel shows, and nothing at all otherwise.
pub fn run(tick: Duration, on_change: fn()) {
    let (sender, messages): (Sender<Message>, Receiver<Message>) = std::sync::mpsc::channel();
    *held(&MESSAGES) = Some(sender);
    let mut worker = Worker {
        plain: ProcessSampler::without_graphics(),
        cards: None,
        ranking: Ranking::default(),
        measured_for: None,
        last: HashMap::new(),
        due: None,
    };
    tracing::info!("the application folds are ready");

    loop {
        let wanted = held(&WANTED).clone();
        match next_step(wanted.resource.is_some(), panel::is_open()) {
            Step::Sleep => {
                if worker.measured_for.take().is_some() {
                    worker.due = None;
                    publish(Shown::default(), on_change);
                }
                match messages.recv() {
                    Ok(message) => worker.handle(message, on_change),
                    Err(_) => return,
                }
            }
            Step::Close => {
                {
                    let mut wanted = held(&WANTED);
                    wanted.resource = None;
                    wanted.holding = false;
                }
                worker.measured_for = None;
                worker.due = None;
                publish(Shown::default(), on_change);
            }
            Step::Sample => {
                let switched = worker.measured_for.as_ref()
                    != Some(&(wanted.resource.expect("a fold is open"), wanted.card.clone()));
                let now = Instant::now();
                match worker.due {
                    Some(due) if !switched && now < due => match messages.recv_timeout(due - now) {
                        Ok(message) => worker.handle(message, on_change),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => return,
                    },
                    _ => worker.sample(&wanted, tick, on_change),
                }
            }
        }
    }
}

/// The top applications for a resource, measured now on a sampler of its own so a script
/// asking does not reset the panel's baselines. Rates are measured over one second. The order
/// is the plain one, without the panel's patience, which is what checking a figure needs.
pub fn top_applications_json(resource: &str, limit: usize) -> Result<String, String> {
    let (name, card) = match resource.split_once(':') {
        Some((name, card)) => (name, card),
        None => (resource, ""),
    };
    let resource = Resource::from_name(name).ok_or_else(|| {
        format!("unknown resource {name:?}: use cpu, memory, disk, gpu or gpu:<card>")
    })?;
    let mut plain = ProcessSampler::without_graphics();
    let mut graphics: Option<ProcessSampler> = None;
    let mut cards_seen = Vec::new();
    let mut measured = None;
    for _ in 0..2 {
        let reading = measure(resource, card, &mut plain, &mut graphics);
        if reading.phase != Phase::Measuring {
            measured = Some(reading);
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    if let Some(sampler) = &graphics {
        cards_seen = sampler.cards().into_iter().map(|card| card.name).collect();
    }
    let Some(measured) = measured else {
        return Ok(serde_json::json!({
            "resource": resource.name(),
            "phase": Phase::Measuring.name(),
            "applications": [],
        })
        .to_string());
    };
    let applications: Vec<serde_json::Value> = measured
        .rows
        .iter()
        .take(limit)
        .map(|sampled| {
            serde_json::json!({
                "key": sampled.row.key,
                "name": sampled.row.name,
                "icon": sampled.row.icon,
                "value": sampled.row.value,
                "secondary": sampled.row.secondary,
                "measure": sampled.measure.primary,
                "pid": sampled.row.pid,
                "pids": sampled.pids,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "resource": resource.name(),
        "phase": measured.phase.name(),
        "status": status_text(resource, measured.phase),
        "card": measured.card,
        "cards": cards_seen,
        "machine_load": measured.machine_load,
        "summary": measured.summary_text,
        "applications": applications,
    })
    .to_string())
}

/// One port as the ports list shows it. A server bound on both IPv4 and IPv6 is one row: the
/// same program answering on the same port, listed twice, reads as two servers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PortRow {
    pub key: String,
    pub port: u16,
    pub transport: &'static str,
    pub address: String,
    /// `everywhere`, `local` or `interface`.
    pub exposure: &'static str,
    pub exposure_text: String,
    pub name: String,
    pub icon: String,
    /// The lowest pid holding the socket, 0 when none of the user's processes does.
    pub pid: u32,
    pub holders: Vec<ProcessKey>,
    /// What Open in browser opens, TCP only, with a wildcard address read as this machine.
    pub url: String,
    /// False for a socket handed in by a socket-activating manager, which no process of the
    /// user's holds and so nothing here can end.
    pub endable: bool,
    pub confirm_text: String,
    /// False once an action closed it, until the next refresh drops the row.
    pub open: bool,
    pub feedback: Option<Feedback>,
}

fn exposure_rank(exposure: Exposure) -> u8 {
    match exposure {
        Exposure::Everywhere => 2,
        Exposure::Interface => 1,
        Exposure::Loopback => 0,
    }
}

fn unmapped(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        v4 => v4,
    }
}

pub fn browser_url(address: IpAddr, port: u16) -> String {
    match unmapped(address) {
        address if address.is_unspecified() => format!("http://localhost:{port}"),
        IpAddr::V6(v6) => format!("http://[{v6}]:{port}"),
        IpAddr::V4(v4) => format!("http://{v4}:{port}"),
    }
}

fn url_for(socket: &ListeningSocket) -> String {
    match socket.transport {
        Transport::Tcp => browser_url(socket.address, socket.port),
        Transport::Udp => String::new(),
    }
}

pub fn port_rows(sockets: &[ListeningSocket]) -> Vec<PortRow> {
    let mut rows: Vec<(PortRow, Exposure)> = Vec::new();
    for socket in sockets {
        let mut holders: Vec<ProcessKey> = socket.holders.iter().map(|holder| holder.key).collect();
        holders.sort();
        let transport = match socket.transport {
            Transport::Tcp => "TCP",
            Transport::Udp => "UDP",
        };
        let pids: Vec<String> = holders.iter().map(|key| key.pid.to_string()).collect();
        let key = format!("{}:{}:{}", transport.to_ascii_lowercase(), socket.port, pids.join(","));
        if let Some((row, widest)) = rows.iter_mut().find(|(row, _)| row.key == key) {
            if exposure_rank(socket.exposure) > exposure_rank(*widest) {
                *widest = socket.exposure;
                row.address = unmapped(socket.address).to_string();
                row.url = url_for(socket);
            }
            continue;
        }
        let first = socket.holders.iter().min_by_key(|holder| holder.key.pid);
        let name = first.map_or_else(|| "Started by the system".to_owned(), |h| h.name.clone());
        rows.push((
            PortRow {
                key,
                port: socket.port,
                transport,
                address: unmapped(socket.address).to_string(),
                exposure: "",
                exposure_text: String::new(),
                icon: first.and_then(|holder| holder.icon.clone()).unwrap_or_default(),
                pid: first.map_or(0, |holder| holder.key.pid),
                url: url_for(socket),
                endable: !holders.is_empty(),
                confirm_text: if holders.is_empty() {
                    String::new()
                } else {
                    format!("This closes port {} by ending {name}", socket.port)
                },
                name,
                holders,
                open: true,
                feedback: None,
            },
            socket.exposure,
        ));
    }
    rows.into_iter()
        .map(|(mut row, exposure)| {
            (row.exposure, row.exposure_text) = match exposure {
                Exposure::Everywhere => ("everywhere", "Everywhere".to_owned()),
                Exposure::Loopback => ("local", "This computer only".to_owned()),
                Exposure::Interface => ("interface", format!("Only on {}", row.address)),
            };
            row
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct PortsShown {
    pub refreshing: bool,
    /// False until the first read has finished, which is what tells "nothing listens" from
    /// "not looked yet".
    pub read: bool,
    pub rows: Vec<PortRow>,
}

impl PortsShown {
    pub fn status_text(&self) -> &'static str {
        if self.refreshing && self.rows.is_empty() {
            "Looking…"
        } else if self.read && self.rows.is_empty() {
            "Nothing of yours is listening"
        } else {
            ""
        }
    }
}

static PORTS: Mutex<Option<PortsShown>> = Mutex::new(None);

pub fn ports() -> PortsShown {
    held(&PORTS).clone().unwrap_or_default()
}

fn ports_mut<R>(change: impl FnOnce(&mut PortsShown) -> R) -> R {
    change(held(&PORTS).get_or_insert_with(PortsShown::default))
}

/// Reads the sockets again, off the Qt thread: the descriptor walk is what costs.
pub fn refresh_ports(on_change: fn()) {
    if ports_mut(|shown| std::mem::replace(&mut shown.refreshing, true)) {
        return;
    }
    on_change();
    std::thread::spawn(move || {
        let rows = port_rows(&system_monitor::listening_sockets());
        ports_mut(|shown| {
            shown.rows = rows;
            shown.refreshing = false;
            shown.read = true;
        });
        on_change();
    });
}

/// The words for a port after its holders were asked to end, read against the sockets as they
/// are afterwards rather than assumed from the signal.
pub fn port_feedback(report: &ActionReport, port: u16, still_listening: bool) -> Feedback {
    let said = feedback_for(report);
    match said.kind {
        FeedbackKind::Ended if still_listening => Feedback {
            kind: FeedbackKind::Ended,
            text: format!("Ended, but port {port} is still open"),
        },
        _ => said,
    }
}

/// Ends what holds a port: the holding processes only, not the application around them, since
/// a server started from a terminal belongs to the terminal's surface.
pub fn end_port(key: &str, force: bool, on_change: fn()) {
    let Some(row) = ports_mut(|shown| shown.rows.iter().find(|row| row.key == key).cloned()) else {
        return;
    };
    if !row.endable {
        return;
    }
    let action = if force { Action::ForceStop } else { Action::Terminate };
    let text = if force { "Forcing it to end…" } else { "Ending…" };
    ports_mut(|shown| {
        if let Some(listed) = shown.rows.iter_mut().find(|listed| listed.key == key) {
            listed.feedback = Some(Feedback { kind: FeedbackKind::Working, text: text.to_owned() });
        }
    });
    on_change();
    std::thread::spawn(move || {
        let report = ProcessControl::for_this_session().act_on_application(&row.holders, action);
        tracing::info!(port = row.port, ?action, outcomes = ?report.outcomes, "acted on a port");
        let mut rows = port_rows(&system_monitor::listening_sockets());
        let still = rows.iter().any(|now| now.port == row.port && now.transport == row.transport);
        let said = port_feedback(&report, row.port, still);
        let at = ports_mut(|shown| shown.rows.iter().position(|listed| listed.key == row.key));
        match rows.iter_mut().find(|now| now.key == row.key) {
            Some(now) => now.feedback = Some(said),
            None => {
                let at = at.unwrap_or(rows.len()).min(rows.len());
                rows.insert(at, PortRow { open: still, feedback: Some(said), ..row });
            }
        }
        ports_mut(|shown| {
            shown.rows = rows;
            shown.read = true;
        });
        on_change();
    });
}

pub fn listening_ports_json() -> String {
    let ports: Vec<serde_json::Value> = port_rows(&system_monitor::listening_sockets())
        .iter()
        .map(|row| {
            serde_json::json!({
                "port": row.port,
                "transport": row.transport,
                "address": row.address,
                "exposure": row.exposure,
                "exposure_text": row.exposure_text,
                "name": row.name,
                "icon": row.icon,
                "pid": row.pid,
                "pids": row.holders.iter().map(|key| key.pid).collect::<Vec<_>>(),
                "url": row.url,
            })
        })
        .collect();
    serde_json::Value::Array(ports).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use system_monitor::{Member, SocketHolder};

    fn sample(figures: &[(&str, f64)]) -> Vec<(String, Measure)> {
        figures
            .iter()
            .map(|(key, value)| ((*key).to_owned(), Measure { primary: *value, secondary: 0.0 }))
            .collect()
    }

    fn none() -> HashSet<String> {
        HashSet::new()
    }

    #[test]
    fn the_first_sample_is_ordered_by_its_figures() {
        let mut ranking = Ranking::default();
        let order = ranking.update(&sample(&[("a", 1.0), ("b", 5.0), ("c", 3.0)]), false, &none());
        assert_eq!(order, ["b", "c", "a"]);
    }

    #[test]
    fn a_close_rival_never_takes_the_place_above() {
        let mut ranking = Ranking::default();
        ranking.update(&sample(&[("a", 3.2), ("b", 3.1)]), false, &none());
        for _ in 0..5 {
            let order = ranking.update(&sample(&[("a", 3.1), ("b", 3.3)]), false, &none());
            assert_eq!(order, ["a", "b"], "a lead under a tenth is noise");
        }
    }

    #[test]
    fn a_clear_lead_takes_the_place_only_on_the_second_sample_running() {
        let mut ranking = Ranking::default();
        ranking.update(&sample(&[("a", 5.0), ("b", 1.0)]), false, &none());

        let once = ranking.update(&sample(&[("a", 5.0), ("b", 9.0)]), false, &none());
        assert_eq!(once, ["a", "b"], "one sample is a spike");

        let twice = ranking.update(&sample(&[("a", 5.0), ("b", 9.0)]), false, &none());
        assert_eq!(twice, ["b", "a"]);
    }

    #[test]
    fn a_lead_that_breaks_off_starts_counting_again() {
        let mut ranking = Ranking::default();
        ranking.update(&sample(&[("a", 5.0), ("b", 1.0)]), false, &none());
        ranking.update(&sample(&[("a", 5.0), ("b", 9.0)]), false, &none());
        ranking.update(&sample(&[("a", 5.0), ("b", 1.0)]), false, &none());
        let order = ranking.update(&sample(&[("a", 5.0), ("b", 9.0)]), false, &none());
        assert_eq!(order, ["a", "b"]);
    }

    #[test]
    fn a_row_far_ahead_climbs_past_several_at_once() {
        let mut ranking = Ranking::default();
        ranking.update(&sample(&[("a", 4.0), ("b", 3.0), ("c", 2.0), ("d", 1.0)]), false, &none());
        let surge = sample(&[("a", 4.0), ("b", 3.0), ("c", 2.0), ("d", 40.0)]);
        ranking.update(&surge, false, &none());
        assert_eq!(ranking.update(&surge, false, &none()), ["d", "a", "b", "c"]);
    }

    #[test]
    fn a_newcomer_enters_where_its_figure_puts_it() {
        let mut ranking = Ranking::default();
        ranking.update(&sample(&[("a", 5.0), ("b", 1.0)]), false, &none());
        let order = ranking.update(&sample(&[("a", 5.0), ("b", 1.0), ("n", 3.0)]), false, &none());
        assert_eq!(order, ["a", "n", "b"]);
    }

    #[test]
    fn video_memory_orders_applications_level_on_the_card() {
        let mut ranking = Ranking::default();
        let idle =
            |key: &str, vram: f64| (key.to_owned(), Measure { primary: 0.0, secondary: vram });
        let order = ranking.update(&[idle("small", 10.0), idle("big", 900.0)], false, &none());
        assert_eq!(order, ["big", "small"]);
    }

    #[test]
    fn under_the_pointer_the_order_is_frozen_and_only_figures_change() {
        let mut ranking = Ranking::default();
        ranking.update(&sample(&[("a", 5.0), ("b", 3.0), ("c", 1.0)]), false, &none());

        for _ in 0..4 {
            let held = sample(&[("a", 1.0), ("c", 9.0), ("n", 50.0)]);
            let order = ranking.update(&held, true, &none());
            assert_eq!(order, ["a", "b", "c", "n"], "a dropped row stays, a new one joins below");
        }

        let released = ranking.update(&sample(&[("a", 1.0), ("c", 9.0)]), false, &none());
        assert_eq!(released, ["a", "c"], "no lead counted while held lands at once");
        assert_eq!(ranking.update(&sample(&[("a", 1.0), ("c", 9.0)]), false, &none()), ["c", "a"]);
    }

    #[test]
    fn a_row_carrying_what_an_action_did_stays_after_its_application_ended() {
        let mut ranking = Ranking::default();
        ranking.update(&sample(&[("a", 5.0), ("b", 3.0)]), false, &none());
        let keep: HashSet<String> = ["a".to_owned()].into();
        assert_eq!(ranking.update(&sample(&[("b", 3.0)]), false, &keep), ["a", "b"]);
        assert_eq!(ranking.update(&sample(&[("b", 3.0)]), false, &none()), ["b"]);
    }

    #[test]
    fn the_worker_samples_only_an_open_fold_on_a_visible_panel() {
        assert_eq!(next_step(false, true), Step::Sleep);
        assert_eq!(next_step(false, false), Step::Sleep);
        assert_eq!(next_step(true, false), Step::Close, "hiding the panel closes the fold");
        assert_eq!(next_step(true, true), Step::Sample);
    }

    #[test]
    fn every_resource_name_round_trips_and_has_its_own_idle_words() {
        for resource in [Resource::Cpu, Resource::Memory, Resource::Graphics, Resource::Disk] {
            assert_eq!(Resource::from_name(resource.name()), Some(resource));
            assert!(!status_text(resource, Phase::Idle).is_empty());
        }
        assert_eq!(status_text(Resource::Graphics, Phase::Idle), "No app is using the GPU now");
        assert_eq!(status_text(Resource::Cpu, Phase::Measuring), "Measuring…");
        assert_eq!(Resource::from_name("network"), None);
    }

    #[test]
    fn application_keys_survive_the_trip_through_qml() {
        for key in [
            ApplicationKey::Entry("org.kde.konsole".into()),
            ApplicationKey::Unit("app-ghostty-surface-transient-5.scope".into()),
            ApplicationKey::Named("PipeWire".into()),
        ] {
            assert_eq!(key_from(&key_text(&key)), Some(key));
        }
        assert_eq!(key_from("pid:4"), None);
    }

    #[test]
    fn figures_read_in_their_own_units() {
        assert_eq!(percent(0.123), "12.3%");
        assert_eq!(memory_text(1_288_490_188), "1.2 GiB");
        assert_eq!(memory_text(512 * 1024 * 1024), "512 MiB");
        let rate = DiskRate { read_per_second: 4_100_000, written_per_second: 220_000 };
        assert_eq!(disk_text(&rate), "↓ 4.1 MB/s ↑ 220 KB/s");
        assert_eq!(rate_text(12), "12 B/s");
    }

    fn report(outcomes: Vec<Outcome>) -> ActionReport {
        ActionReport {
            action: Action::Terminate,
            outcomes: outcomes
                .into_iter()
                .enumerate()
                .map(|(at, outcome)| (ProcessKey { pid: at as u32 + 10, started: 1 }, outcome))
                .collect(),
        }
    }

    #[test]
    fn refusals_are_said_in_plain_words() {
        let session = Refusal::Session("plasmashell".into());
        assert_eq!(refusal_words(&session), "Part of the desktop session");
        assert_eq!(refusal_words(&Refusal::NotYours), "Belongs to another user");
        let member = Refusal::Member { pid: 4, reason: Box::new(session) };
        assert_eq!(
            refusal_words(&member),
            "Part of the desktop session",
            "the reason, not the pid"
        );

        let refused = feedback_for(&report(vec![
            Outcome::Refused(member),
            Outcome::Refused(Refusal::Session("plasmashell".into())),
        ]));
        assert_eq!(refused.kind, FeedbackKind::Refused);
        assert_eq!(refused.text, "Part of the desktop session");
    }

    #[test]
    fn an_action_reads_as_ended_still_running_or_failed() {
        let ended = feedback_for(&report(vec![Outcome::Ended, Outcome::Refused(Refusal::Gone)]));
        assert_eq!(ended, Feedback { kind: FeedbackKind::Ended, text: "Ended".into() });
        assert_eq!(feedback_for(&report(vec![])).kind, FeedbackKind::Ended, "nothing left to end");

        let stubborn = feedback_for(&report(vec![Outcome::Ended, Outcome::StillRunning]));
        let still = Feedback { kind: FeedbackKind::StillRunning, text: "Still running".into() };
        assert_eq!(stubborn, still);

        let failed = feedback_for(&report(vec![Outcome::Failed("Operation not permitted".into())]));
        assert_eq!(failed.kind, FeedbackKind::Failed);

        let open = port_feedback(&report(vec![Outcome::Ended]), 3000, true);
        assert_eq!(open.text, "Ended, but port 3000 is still open");
        assert_eq!(port_feedback(&report(vec![Outcome::Ended]), 3000, false).text, "Ended");
    }

    #[test]
    fn copy_pid_copies_the_process_the_others_descend_from() {
        let member = |pid: u32, parent: u32| Member {
            key: ProcessKey { pid, started: 1 },
            parent,
            command: "x".into(),
            executable: None,
            value: CpuShare::default(),
        };
        let application = ApplicationUse {
            key: ApplicationKey::Named("x".into()),
            name: "x".into(),
            icon: None,
            value: CpuShare::default(),
            members: vec![member(30, 20), member(20, 1), member(40, 20)],
        };
        assert_eq!(leader(&application), 20);
    }

    fn holder(pid: u32, name: &str) -> SocketHolder {
        SocketHolder {
            key: ProcessKey { pid, started: 7 },
            command: name.to_ascii_lowercase(),
            name: name.into(),
            icon: Some("icon".into()),
        }
    }

    fn socket(
        transport: Transport,
        address: &str,
        port: u16,
        exposure: Exposure,
        holders: Vec<SocketHolder>,
    ) -> ListeningSocket {
        ListeningSocket { transport, address: address.parse().unwrap(), port, exposure, holders }
    }

    #[test]
    fn a_dual_stack_server_is_one_row_reachable_as_widely_as_either_socket() {
        let node = || vec![holder(5, "Node")];
        let rows = port_rows(&[
            socket(Transport::Tcp, "127.0.0.1", 3000, Exposure::Loopback, node()),
            socket(Transport::Tcp, "::", 3000, Exposure::Everywhere, node()),
            socket(Transport::Udp, "0.0.0.0", 3000, Exposure::Everywhere, node()),
        ]);
        assert_eq!(rows.len(), 2, "UDP on the same port is another service");
        let tcp = &rows[0];
        assert_eq!((tcp.transport, tcp.exposure), ("TCP", "everywhere"));
        assert_eq!(tcp.url, "http://localhost:3000", "a wildcard opens on this machine");
        assert_eq!(tcp.confirm_text, "This closes port 3000 by ending Node");
        assert_eq!(rows[1].url, "", "a datagram socket has nothing to open in a browser");
    }

    #[test]
    fn a_port_nobody_of_yours_holds_cannot_be_ended() {
        let rows = port_rows(&[socket(Transport::Tcp, "::1", 631, Exposure::Loopback, Vec::new())]);
        assert!(!rows[0].endable);
        assert_eq!(rows[0].confirm_text, "");
        assert_eq!(rows[0].exposure_text, "This computer only");
        assert_eq!(rows[0].url, "http://[::1]:631");
    }
}
