//! What each application of the current user is spending: processor, memory, a graphics card,
//! or the disk, one resource per request.
//!
//! Each resource is asked for on its own so the shell pays only for the fold a person has
//! open. The answers are applications rather than processes: one row for a browser and its
//! forty helpers, with the processes kept underneath for anyone who wants them and for the
//! actions, which bind to each process's id and start time.

mod amdgpu;
mod control;
#[cfg(test)]
pub(crate) mod fixture;
mod grouping;
pub(crate) mod procfs;

pub use control::{
    Action, ActionReport, CpuWeight, Outcome, PriorityError, ProcessControl, Refusal,
};
pub use grouping::ApplicationKey;

use crate::graphics::{GpuRole, discover_sysfs_cards_in};
use crate::processor::{CpuTicks, parse_stat};
use amdgpu::ClientLedger;
use app_identity::DesktopEntry;
use nvml_wrapper::Nvml;
use nvml_wrapper::enums::device::UsedGpuMemory;
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::struct_wrappers::device::ProcessUtilizationSample;
use procfs::{parse_io, parse_pss, parse_stat_line, parse_statm, read_into};
use std::collections::{HashMap, HashSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// One process for its whole life. A pid alone can name a different program a second later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcessKey {
    pub pid: u32,
    /// Clock ticks after boot, field 22 of `/proc/<pid>/stat`.
    pub started: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member<V> {
    pub key: ProcessKey,
    pub parent: u32,
    /// What the kernel calls it, at most fifteen bytes.
    pub command: String,
    pub executable: Option<PathBuf>,
    pub value: V,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationUse<V> {
    pub key: ApplicationKey,
    pub name: String,
    pub icon: Option<String>,
    /// The sum over `members`.
    pub value: V,
    pub members: Vec<Member<V>>,
}

impl<V> ApplicationUse<V> {
    pub fn processes(&self) -> Vec<ProcessKey> {
        self.members.iter().map(|member| member.key).collect()
    }
}

/// Rates are differences between two readings. The first request after opening, or after the
/// fold sat closed for longer than [`BASELINE_LIMIT`], only takes the baseline; a figure
/// averaged over the minutes nobody looked would be a different question answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading<T> {
    Priming,
    Ready(T),
}

pub const BASELINE_LIMIT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CpuShare {
    /// Share of the whole machine, the scale the monitor's own load figure is on.
    pub of_machine: f32,
    /// The same time counted against one core, so a process saturating two cores reads 2.0.
    pub of_one_core: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuFold {
    /// Busy share of the machine over the same interval, read from the same `/proc/stat`.
    pub machine_load: f32,
    pub threads: usize,
    pub applications: Vec<ApplicationUse<CpuShare>>,
    /// What the listed rows leave of `machine_load`: the kernel, other users, processes that
    /// ended during the interval, and the applications under the floor or past the limit.
    pub elsewhere: f32,
}

/// Where a memory figure came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBasis {
    /// Proportional set size from `smaps_rollup`: shared pages divided among their users.
    Proportional,
    /// Resident pages from `statm`, which counts every shared library once per process.
    Resident,
    /// Some members read one way and some the other.
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryUse {
    pub bytes: u64,
    pub basis: MemoryBasis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryFold {
    pub applications: Vec<ApplicationUse<MemoryUse>>,
    /// Applications below the floor or past the limit.
    pub quiet: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DiskRate {
    pub read_per_second: u64,
    pub written_per_second: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskFold {
    pub applications: Vec<ApplicationUse<DiskRate>>,
    pub quiet: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessCard {
    /// The name the monitor gives the same card, so a fold can be matched to its readout.
    pub name: String,
    pub pci_slot: Option<String>,
    pub role: GpuRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CardUse {
    /// Busy share of this card alone. `None` when the driver cannot say or a baseline is
    /// still being taken.
    pub busy: Option<f32>,
    pub vram: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsFold {
    pub card: ProcessCard,
    /// True while busy figures wait for a second reading. VRAM needs none and is present.
    pub busy_priming: bool,
    pub applications: Vec<ApplicationUse<CardUse>>,
    pub quiet: usize,
}

/// Below these an application is left out of a fold rather than listed at zero.
const CPU_FLOOR: f32 = 0.001;
const MEMORY_FLOOR: u64 = 1024 * 1024;
const DISK_FLOOR: u64 = 1024;
const BUSY_FLOOR: f32 = 0.005;
const VRAM_FLOOR: u64 = 1024 * 1024;

/// `smaps_rollup` walks every mapping of the process, about half a millisecond for a large
/// one, so only the applications that can make the list are read, each at most this often
/// and never more than this many per request. Measured: all 130 processes of a desktop
/// session take 55 ms, more than every other read in a sample put together. A process never
/// read comes before one read a while ago, so a group of hundreds settles within a minute.
const PSS_REFRESH: Duration = Duration::from_secs(30);
const PSS_KEEP: Duration = Duration::from_secs(120);
const PSS_BUDGET: usize = 32;

/// What a process is called and where it runs, which does not change for a pid and start
/// time, so it is read once.
struct Known {
    command: String,
    executable: Option<PathBuf>,
    unit: Option<String>,
    name: String,
    icon: Option<String>,
    session: bool,
}

struct Walked {
    key: ProcessKey,
    parent: u32,
    cpu_ticks: u64,
}

struct Group {
    key: ApplicationKey,
    name: String,
    icon: Option<String>,
    /// The program that started a unit naming no entry, a terminal for its surfaces. The
    /// group is then named after its busiest member, which depends on the resource asked.
    host: Option<DesktopEntry>,
    members: Vec<usize>,
}

enum Backend {
    Nvidia { index: u32 },
    Amd,
}

struct CardSource {
    card: ProcessCard,
    backend: Backend,
}

struct CpuBaseline {
    at: Instant,
    total: CpuTicks,
    ticks: HashMap<ProcessKey, u64>,
}

struct DiskBaseline {
    at: Instant,
    bytes: HashMap<ProcessKey, (u64, u64)>,
}

#[derive(Default)]
struct GraphicsBaseline {
    at: Option<Instant>,
    engines: HashMap<(usize, u64), Vec<(String, u64)>>,
    nvidia_seen: Option<u64>,
}

pub struct ProcessSampler {
    proc_root: PathBuf,
    uid: u32,
    page_size: u64,
    nvml: Option<Nvml>,
    cards: Vec<CardSource>,
    known: HashMap<ProcessKey, Known>,
    cpu: Option<CpuBaseline>,
    disk: Option<DiskBaseline>,
    graphics: HashMap<usize, GraphicsBaseline>,
    pss: HashMap<ProcessKey, (u64, Instant)>,
    resolve: fn(&str) -> Option<DesktopEntry>,
    resolve_host: fn(&str) -> Option<DesktopEntry>,
    buffer: String,
}

fn desktop_entry(id: &str) -> Option<DesktopEntry> {
    app_identity::named_after_identity(id).cloned()
}

/// A terminal is known by its binary as often as by its identity: Ghostty's entry is
/// `com.mitchellh.ghostty` and runs `ghostty`, which is what its surfaces are named after.
fn host_entry(id: &str) -> Option<DesktopEntry> {
    app_identity::named_after_identity(id).or_else(|| app_identity::named_after_binary(id)).cloned()
}

impl ProcessSampler {
    pub fn open() -> Self {
        let nvml = Nvml::init().ok();
        Self::build(Path::new("/proc"), Some(Path::new("/sys/class/drm")), current_uid(), nvml)
    }

    /// Knows no card and never asks a driver anything, for the folds that need none: opening
    /// NVML is enough to wake a discrete card that was asleep.
    pub fn without_graphics() -> Self {
        Self::build(Path::new("/proc"), None, current_uid(), None)
    }

    /// Reads processes under `proc_root` and amdgpu cards under `drm_root`, with no NVIDIA
    /// driver, so a test can point both at trees it wrote.
    pub fn open_in(proc_root: &Path, drm_root: &Path) -> Self {
        Self::build(proc_root, Some(drm_root), current_uid(), None)
    }

    fn build(proc_root: &Path, drm_root: Option<&Path>, uid: u32, nvml: Option<Nvml>) -> Self {
        let mut cards = Vec::new();
        if let Some(nvml) = &nvml {
            for index in 0..nvml.device_count().unwrap_or(0) {
                let Ok(device) = nvml.device_by_index(index) else {
                    continue;
                };
                cards.push(CardSource {
                    card: ProcessCard {
                        name: device.name().unwrap_or_else(|_| "NVIDIA".into()),
                        pci_slot: device.pci_info().ok().map(|info| short_slot(&info.bus_id)),
                        role: GpuRole::Discrete,
                    },
                    backend: Backend::Nvidia { index },
                });
            }
        }
        for card in drm_root.map(discover_sysfs_cards_in).unwrap_or_default() {
            let pci_slot =
                std::fs::read_to_string(card.device.join("uevent")).ok().and_then(|text| {
                    text.lines()
                        .find_map(|line| line.strip_prefix("PCI_SLOT_NAME=").map(str::to_owned))
                });
            cards.push(CardSource {
                card: ProcessCard { name: card.name, pci_slot, role: GpuRole::Integrated },
                backend: Backend::Amd,
            });
        }
        Self {
            proc_root: proc_root.to_path_buf(),
            uid,
            page_size: page_size(),
            nvml,
            cards,
            known: HashMap::new(),
            cpu: None,
            disk: None,
            graphics: HashMap::new(),
            pss: HashMap::new(),
            resolve: desktop_entry,
            resolve_host: host_entry,
            buffer: String::with_capacity(4096),
        }
    }

    pub fn cards(&self) -> Vec<ProcessCard> {
        self.cards.iter().map(|source| source.card.clone()).collect()
    }

    pub fn cpu(&mut self, limit: usize) -> Reading<CpuFold> {
        let now = Instant::now();
        let machine = if read_into(&self.proc_root.join("stat"), &mut self.buffer) {
            parse_stat(&self.buffer)
        } else {
            Default::default()
        };
        let walked = self.walk();
        let baseline = self.cpu.take().filter(|baseline| now - baseline.at <= BASELINE_LIMIT);
        self.cpu = Some(CpuBaseline {
            at: now,
            total: machine.total,
            ticks: walked.iter().map(|process| (process.key, process.cpu_ticks)).collect(),
        });
        let Some(baseline) = baseline else {
            return Reading::Priming;
        };
        let (Some(elapsed), Some(machine_load)) = (
            machine.total.elapsed_since(baseline.total).filter(|ticks| *ticks > 0),
            machine.total.load_since(baseline.total),
        ) else {
            return Reading::Priming;
        };

        let threads = machine.cores.len().max(1);
        let mut shares: Vec<Option<f32>> = walked
            .iter()
            .map(|process| {
                // A process the baseline did not hold started inside the interval, so all of
                // its time was spent there.
                let before = baseline.ticks.get(&process.key).copied().unwrap_or(0);
                Some(process.cpu_ticks.saturating_sub(before) as f32 / elapsed as f32)
            })
            .collect();
        // The two counters are read a moment apart, so the processes can add up to slightly
        // more than the machine says it did. The machine's figure is the one shown above the
        // fold, and the rows are scaled to fit under it.
        let total: f32 = shares.iter().flatten().sum();
        if total > machine_load && total > 0.0 {
            let scale = machine_load / total;
            for share in shares.iter_mut().flatten() {
                *share *= scale;
            }
        }
        let values: Vec<Option<CpuShare>> = shares
            .into_iter()
            .map(|share| {
                share.map(|of_machine| {
                    let of_machine = of_machine.min(1.0);
                    CpuShare { of_machine, of_one_core: of_machine * threads as f32 }
                })
            })
            .collect();

        let groups = self.group(&walked);
        let busiest = |share: &CpuShare| f64::from(share.of_machine);
        let mut applications = self.fold(&groups, &walked, &values, busiest, |members| {
            let of_machine: f32 = members.iter().map(|share| share.of_machine).sum();
            CpuShare { of_machine: of_machine.min(1.0), of_one_core: of_machine * threads as f32 }
        });
        applications.retain(|application| application.value.of_machine >= CPU_FLOOR);
        applications.sort_by(|left, right| {
            right
                .value
                .of_machine
                .total_cmp(&left.value.of_machine)
                .then_with(|| left.key.cmp(&right.key))
        });
        applications.truncate(limit);
        let listed: f32 = applications.iter().map(|application| application.value.of_machine).sum();
        Reading::Ready(CpuFold {
            machine_load,
            threads,
            applications,
            elsewhere: (machine_load - listed).max(0.0),
        })
    }

    pub fn memory(&mut self, limit: usize) -> MemoryFold {
        let now = Instant::now();
        let walked = self.walk();
        let resident: Vec<Option<u64>> = walked
            .iter()
            .map(|process| {
                let path = self.proc_root.join(process.key.pid.to_string()).join("statm");
                read_into(&path, &mut self.buffer)
                    .then(|| parse_statm(&self.buffer))
                    .flatten()
                    .map(|(pages, _)| pages.saturating_mul(self.page_size))
            })
            .collect();
        let groups = self.group(&walked);

        // Ranked by resident size first, since that is free, then the likely top of the list
        // is read properly. A little past the limit, because the proportional figure can
        // reorder neighbours.
        let mut ranked: Vec<(u64, &Group)> = groups
            .iter()
            .map(|group| (group.members.iter().filter_map(|&at| resident[at]).sum(), group))
            .collect();
        ranked.sort_by_key(|(bytes, _)| std::cmp::Reverse(*bytes));
        let mut candidates: Vec<usize> = ranked
            .iter()
            .take(limit + limit / 2 + 2)
            .flat_map(|(_, group)| group.members.iter().copied())
            .collect();
        candidates.sort_by_key(|&at| {
            (self.pss.contains_key(&walked[at].key), std::cmp::Reverse(resident[at].unwrap_or(0)))
        });
        let mut budget = PSS_BUDGET;
        for at in candidates {
            if budget == 0 {
                break;
            }
            let key = walked[at].key;
            if self.pss.get(&key).is_some_and(|(_, read)| now - *read < PSS_REFRESH) {
                continue;
            }
            budget -= 1;
            let path = self.proc_root.join(key.pid.to_string()).join("smaps_rollup");
            if let Some(bytes) =
                read_into(&path, &mut self.buffer).then(|| parse_pss(&self.buffer)).flatten()
            {
                self.pss.insert(key, (bytes, now));
            }
        }
        let present: HashSet<ProcessKey> = walked.iter().map(|process| process.key).collect();
        self.pss.retain(|key, (_, read)| present.contains(key) && now - *read < PSS_KEEP);

        let values: Vec<Option<MemoryUse>> = walked
            .iter()
            .zip(&resident)
            .map(|(process, resident)| match self.pss.get(&process.key) {
                Some((bytes, _)) => {
                    Some(MemoryUse { bytes: *bytes, basis: MemoryBasis::Proportional })
                }
                None => resident.map(|bytes| MemoryUse { bytes, basis: MemoryBasis::Resident }),
            })
            .collect();
        let busiest = |use_: &MemoryUse| use_.bytes as f64;
        let mut applications = self.fold(&groups, &walked, &values, busiest, |members| MemoryUse {
            bytes: members.iter().map(|member| member.bytes).sum(),
            basis: if members.iter().all(|member| member.basis == MemoryBasis::Proportional) {
                MemoryBasis::Proportional
            } else if members.iter().all(|member| member.basis == MemoryBasis::Resident) {
                MemoryBasis::Resident
            } else {
                MemoryBasis::Mixed
            },
        });
        let all = applications.len();
        applications.retain(|application| application.value.bytes >= MEMORY_FLOOR);
        applications.sort_by(|left, right| {
            right.value.bytes.cmp(&left.value.bytes).then_with(|| left.key.cmp(&right.key))
        });
        applications.truncate(limit);
        MemoryFold { quiet: all - applications.len(), applications }
    }

    pub fn disk(&mut self, limit: usize) -> Reading<DiskFold> {
        let now = Instant::now();
        let walked = self.walk();
        let bytes: Vec<Option<(u64, u64)>> = walked
            .iter()
            .map(|process| {
                let path = self.proc_root.join(process.key.pid.to_string()).join("io");
                read_into(&path, &mut self.buffer).then(|| parse_io(&self.buffer)).flatten()
            })
            .collect();
        let baseline = self.disk.take().filter(|baseline| now - baseline.at <= BASELINE_LIMIT);
        self.disk = Some(DiskBaseline {
            at: now,
            bytes: walked
                .iter()
                .zip(&bytes)
                .filter_map(|(process, bytes)| Some((process.key, (*bytes)?)))
                .collect(),
        });
        let Some(baseline) = baseline else {
            return Reading::Priming;
        };
        let seconds = (now - baseline.at).as_secs_f64();
        if seconds <= 0.0 {
            return Reading::Priming;
        }
        let rate = |delta: u64| (delta as f64 / seconds).round() as u64;
        let values: Vec<Option<DiskRate>> = walked
            .iter()
            .zip(&bytes)
            .map(|(process, bytes)| {
                let (read, written) = (*bytes)?;
                let (read_before, written_before) =
                    baseline.bytes.get(&process.key).copied().unwrap_or((0, 0));
                Some(DiskRate {
                    read_per_second: rate(read.saturating_sub(read_before)),
                    written_per_second: rate(written.saturating_sub(written_before)),
                })
            })
            .collect();
        let groups = self.group(&walked);
        let busiest = |rate: &DiskRate| (rate.read_per_second + rate.written_per_second) as f64;
        let mut applications = self.fold(&groups, &walked, &values, busiest, |members| DiskRate {
            read_per_second: members.iter().map(|rate| rate.read_per_second).sum(),
            written_per_second: members.iter().map(|rate| rate.written_per_second).sum(),
        });
        let all = applications.len();
        let moved = |rate: &DiskRate| rate.read_per_second + rate.written_per_second;
        applications.retain(|application| moved(&application.value) >= DISK_FLOOR);
        applications.sort_by(|left, right| {
            moved(&right.value).cmp(&moved(&left.value)).then_with(|| left.key.cmp(&right.key))
        });
        applications.truncate(limit);
        Reading::Ready(DiskFold { quiet: all - applications.len(), applications })
    }

    /// `None` for a card this sampler does not know by that name.
    pub fn graphics(&mut self, card_name: &str, limit: usize) -> Option<GraphicsFold> {
        let index = self.cards.iter().position(|source| source.card.name == card_name)?;
        let now = Instant::now();
        let walked = self.walk();
        let mut baseline = self.graphics.remove(&index).unwrap_or_default();
        let fresh = baseline.at.filter(|at| now - *at <= BASELINE_LIMIT);
        let (per_pid, busy_priming) = match self.cards[index].backend {
            Backend::Nvidia { index: device } => {
                let seen = if fresh.is_some() { baseline.nvidia_seen } else { None };
                let (rows, newest) = self.nvidia_use(device, seen);
                if newest.is_some() {
                    baseline.nvidia_seen = newest;
                }
                (rows, false)
            }
            Backend::Amd => {
                let rows = self.amd_use(index, &walked, &mut baseline, fresh.map(|at| now - at));
                (rows, fresh.is_none())
            }
        };
        baseline.at = Some(now);
        self.graphics.insert(index, baseline);

        let values: Vec<Option<CardUse>> =
            walked.iter().map(|process| per_pid.get(&process.key.pid).copied()).collect();
        let groups = self.group(&walked);
        // Busy share decides; video memory, scaled far below any busy figure, only separates
        // members equally busy, which on an idle card is all of them.
        let busiest = |use_: &CardUse| {
            f64::from(use_.busy.unwrap_or(0.0)) + use_.vram.unwrap_or(0) as f64 / 1e15
        };
        let mut applications = self.fold(&groups, &walked, &values, busiest, |members| {
            let busy: Option<f32> =
                members.iter().filter_map(|use_| use_.busy).reduce(|a, b| a + b);
            let vram: Option<u64> =
                members.iter().filter_map(|use_| use_.vram).reduce(|a, b| a + b);
            CardUse { busy: busy.map(|busy| busy.min(1.0)), vram }
        });
        let all = applications.len();
        applications.retain(|application| {
            application.value.busy.is_some_and(|busy| busy >= BUSY_FLOOR)
                || application.value.vram.is_some_and(|vram| vram >= VRAM_FLOOR)
        });
        applications.sort_by(|left, right| {
            let busy = |use_: &CardUse| use_.busy.unwrap_or(0.0);
            busy(&right.value)
                .total_cmp(&busy(&left.value))
                .then_with(|| right.value.vram.cmp(&left.value.vram))
                .then_with(|| left.key.cmp(&right.key))
        });
        applications.truncate(limit);
        Some(GraphicsFold {
            card: self.cards[index].card.clone(),
            busy_priming,
            quiet: all - applications.len(),
            applications,
        })
    }

    /// Every process of an application as it stands now, not only those a fold listed: the
    /// graphics fold lists the processes on a card, and ending a browser means its window
    /// too, not just its GPU helper.
    pub fn members_of(&mut self, key: &ApplicationKey) -> Vec<ProcessKey> {
        let walked = self.walk();
        self.group(&walked)
            .into_iter()
            .find(|group| group.key == *key)
            .map(|group| group.members.iter().map(|&at| walked[at].key).collect())
            .unwrap_or_default()
    }

    fn nvidia_use(&self, device: u32, seen: Option<u64>) -> (HashMap<u32, CardUse>, Option<u64>) {
        let Some(device) = self.nvml.as_ref().and_then(|nvml| nvml.device_by_index(device).ok())
        else {
            return (HashMap::new(), None);
        };
        let memory: Vec<(u32, Option<u64>)> = device
            .running_graphics_processes()
            .into_iter()
            .flatten()
            .chain(device.running_compute_processes().into_iter().flatten())
            .map(|process| {
                let bytes = match process.used_gpu_memory {
                    UsedGpuMemory::Used(bytes) => Some(bytes),
                    UsedGpuMemory::Unavailable => None,
                };
                (process.pid, bytes)
            })
            .collect();
        // The driver keeps a ring of samples stamped in microseconds of wall time and returns
        // those newer than the stamp it is given. With no earlier call to continue from, the
        // last two seconds stand in for an interval, which spares a priming round.
        let since = seen.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |now| now.as_micros() as u64)
                .saturating_sub(2_000_000)
        });
        // No process busy in the interval is reported as NotFound, not as an empty list.
        let samples = match device.process_utilization_stats(since) {
            Ok(samples) => Some(samples),
            Err(NvmlError::NotFound) => Some(Vec::new()),
            Err(_) => None,
        };
        nvidia_rows(&memory, samples.as_deref())
    }

    fn amd_use(
        &mut self,
        card: usize,
        walked: &[Walked],
        baseline: &mut GraphicsBaseline,
        elapsed: Option<Duration>,
    ) -> HashMap<u32, CardUse> {
        let slot = self.cards[card].card.pci_slot.clone();
        let amd_cards =
            self.cards.iter().filter(|source| matches!(source.backend, Backend::Amd)).count();
        let mut ledger = ClientLedger::default();
        for process in walked {
            let dir = self.proc_root.join(process.key.pid.to_string());
            let Ok(descriptors) = std::fs::read_dir(dir.join("fd")) else {
                continue;
            };
            for descriptor in descriptors.flatten() {
                // Only descriptors open on a DRM node are worth an fdinfo read; a browser holds
                // hundreds of others.
                if !std::fs::read_link(descriptor.path())
                    .is_ok_and(|target| target.starts_with("/dev/dri"))
                {
                    continue;
                }
                let info = dir.join("fdinfo").join(descriptor.file_name());
                let Some(reading) = read_into(&info, &mut self.buffer)
                    .then(|| amdgpu::parse_fdinfo(&self.buffer))
                    .flatten()
                else {
                    continue;
                };
                if reading.driver.as_deref() != Some("amdgpu") {
                    continue;
                }
                // Without a PCI address the counters cannot be tied to one of two amdgpu
                // cards, and adding them to either would be a guess.
                let ours = match (&reading.pci_slot, &slot) {
                    (Some(theirs), Some(ours)) => theirs == ours,
                    _ => amd_cards == 1,
                };
                if ours {
                    ledger.record(card, process.key.pid, reading);
                }
            }
        }
        let elapsed_ns = elapsed.map(|elapsed| elapsed.as_nanos() as u64);
        ledger
            .settle(&mut baseline.engines, elapsed_ns)
            .into_iter()
            .map(|((pid, _), (busy, vram))| (pid, CardUse { busy, vram }))
            .collect()
    }

    /// The current user's processes, kernel threads aside. Ownership is the directory's
    /// owner, a stat the directory listing already paid for, so nothing of another user's
    /// process is opened.
    fn walk(&mut self) -> Vec<Walked> {
        let mut walked = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.proc_root) else {
            return walked;
        };
        let mut path = self.proc_root.clone();
        for entry in entries.flatten() {
            let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };
            if !entry.metadata().is_ok_and(|metadata| metadata.uid() == self.uid) {
                continue;
            }
            path.push(entry.file_name());
            path.push("stat");
            let line =
                read_into(&path, &mut self.buffer).then(|| parse_stat_line(&self.buffer)).flatten();
            path.pop();
            path.pop();
            let Some(line) = line.filter(|line| !line.kernel_thread) else {
                continue;
            };
            let key = ProcessKey { pid, started: line.started };
            if !self.known.contains_key(&key) {
                let known = self.identify(pid, line.command);
                self.known.insert(key, known);
            }
            walked.push(Walked { key, parent: line.parent, cpu_ticks: line.cpu_ticks });
        }
        let present: HashSet<ProcessKey> = walked.iter().map(|process| process.key).collect();
        self.known.retain(|key, _| present.contains(key));
        walked
    }

    fn identify(&mut self, pid: u32, command: String) -> Known {
        let dir = self.proc_root.join(pid.to_string());
        let executable = std::fs::read_link(dir.join("exe")).ok();
        let unit = read_into(&dir.join("cgroup"), &mut self.buffer)
            .then(|| {
                procfs::control_group(&self.buffer)
                    .and_then(|group| procfs::user_unit(group, self.uid))
            })
            .flatten();
        let session = session_of(&command, executable.as_deref()).is_some();
        let (name, icon) = process_name(&self.proc_root, pid, &command, executable.clone());
        Known { command, executable, unit, name, icon, session }
    }

    fn group(&self, walked: &[Walked]) -> Vec<Group> {
        let known: Vec<&Known> = walked.iter().map(|process| &self.known[&process.key]).collect();
        let placements: Vec<grouping::Placement<'_>> = walked
            .iter()
            .zip(&known)
            .map(|(process, known)| grouping::Placement {
                pid: process.key.pid,
                parent: process.parent,
                unit: known.unit.as_deref(),
                name: &known.name,
                session: known.session,
            })
            .collect();
        let assigned = grouping::assign(&placements, &self.resolve);

        let mut groups: Vec<Group> = Vec::new();
        let mut by_key: HashMap<ApplicationKey, usize> = HashMap::new();
        let mut entries = Vec::new();
        for (at, found) in assigned.into_iter().enumerate() {
            let slot = *by_key.entry(found.key.clone()).or_insert_with(|| {
                let host = match (&found.key, &found.entry) {
                    (ApplicationKey::Unit(unit), None) => {
                        grouping::host_of(unit, &self.resolve_host)
                    }
                    _ => None,
                };
                groups.push(Group {
                    key: found.key,
                    name: String::new(),
                    icon: None,
                    host,
                    members: Vec::new(),
                });
                entries.push(found.entry);
                groups.len() - 1
            });
            groups[slot].members.push(at);
        }
        let pids: HashMap<u32, usize> =
            walked.iter().enumerate().map(|(at, process)| (process.key.pid, at)).collect();
        for (group, entry) in groups.iter_mut().zip(entries) {
            // The process the rest descend from names a group no entry names: its first
            // member whose parent is outside it.
            let inside: HashSet<usize> = group.members.iter().copied().collect();
            let leader = group
                .members
                .iter()
                .copied()
                .filter(|&at| {
                    pids.get(&walked[at].parent).is_none_or(|parent| !inside.contains(parent))
                })
                .min_by_key(|&at| walked[at].key.pid)
                .unwrap_or(group.members[0]);
            match entry {
                Some(entry) => {
                    group.name = entry.name;
                    group.icon = entry.icon.or_else(|| known[leader].icon.clone());
                }
                None => {
                    group.name = known[leader].name.clone();
                    group.icon = match &group.host {
                        Some(host) => host.icon.clone().or_else(|| known[leader].icon.clone()),
                        None => known[leader].icon.clone(),
                    };
                }
            }
        }
        groups
    }

    fn fold<V: Copy>(
        &self,
        groups: &[Group],
        walked: &[Walked],
        values: &[Option<V>],
        weight: impl Fn(&V) -> f64,
        sum: impl Fn(&[V]) -> V,
    ) -> Vec<ApplicationUse<V>> {
        groups
            .iter()
            .filter_map(|group| {
                let members: Vec<Member<V>> = group
                    .members
                    .iter()
                    .filter_map(|&at| {
                        let value = values[at]?;
                        let process = &walked[at];
                        let known = &self.known[&process.key];
                        Some(Member {
                            key: process.key,
                            parent: process.parent,
                            command: known.command.clone(),
                            executable: known.executable.clone(),
                            value,
                        })
                    })
                    .collect();
                if members.is_empty() {
                    return None;
                }
                let each: Vec<V> = members.iter().map(|member| member.value).collect();
                // Ties go to the lowest pid, the shell a surface opened with, so an idle
                // terminal reads as its shell rather than as whichever process listed last.
                let name = match &group.host {
                    Some(host) => {
                        let busiest = members
                            .iter()
                            .min_by(|left, right| {
                                weight(&right.value)
                                    .total_cmp(&weight(&left.value))
                                    .then_with(|| left.key.pid.cmp(&right.key.pid))
                            })
                            .map(|member| self.known[&member.key].name.as_str())
                            .unwrap_or(&group.name);
                        format!("{busiest} · {}", host.name)
                    }
                    None => group.name.clone(),
                };
                Some(ApplicationUse {
                    key: group.key.clone(),
                    name,
                    icon: group.icon.clone(),
                    value: sum(&each),
                    members,
                })
            })
            .collect()
    }
}

/// NVML spells a PCI address with an eight digit domain, DRM with four.
fn short_slot(bus_id: &str) -> String {
    let lower = bus_id.to_ascii_lowercase();
    match lower.len() {
        len if len > 12 => lower[len - 12..].to_owned(),
        _ => lower,
    }
}

fn current_uid() -> u32 {
    nix::unistd::geteuid().as_raw()
}

fn page_size() -> u64 {
    nix::unistd::sysconf(nix::unistd::SysconfVar::PAGE_SIZE)
        .ok()
        .flatten()
        .and_then(|size| u64::try_from(size).ok())
        .unwrap_or(4096)
}

/// What a person would call one process, and its icon: the desktop's name for the program,
/// else the kernel's command. A binary named after its release, `2.1.252`, tells nobody what
/// is running, and the command is that same word, so the directory it was installed under
/// stands in.
pub(crate) fn process_name(
    proc_root: &Path,
    pid: u32,
    command: &str,
    executable: Option<PathBuf>,
) -> (String, Option<String>) {
    if let Some((name, icon)) = session_of(command, executable.as_deref()) {
        return (name.to_owned(), Some(icon.to_owned()));
    }
    let identity = app_identity::process_in(proc_root, pid);
    let name = identity
        .name
        .filter(|name| !looks_like_version(name))
        .or_else(|| (!looks_like_version(command)).then(|| command.to_owned()))
        .or_else(|| executable.and_then(installed_under))
        .unwrap_or_else(|| command.to_owned());
    (name, identity.icon)
}

fn session_of(command: &str, executable: Option<&Path>) -> Option<(&'static str, &'static str)> {
    let binary =
        executable.and_then(Path::file_name).and_then(|name| name.to_str()).unwrap_or(command);
    grouping::session_service(binary)
}

fn looks_like_version(name: &str) -> bool {
    name.chars().any(|c| c.is_ascii_digit()) && name.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn installed_under(executable: PathBuf) -> Option<String> {
    executable
        .ancestors()
        .skip(1)
        .filter_map(|directory| directory.file_name())
        .map(|directory| directory.to_string_lossy().into_owned())
        .find(|directory| directory != "versions" && !looks_like_version(directory))
}

/// Every listed process is on the card; one absent from the fresh samples was idle there.
/// Only the newest sample describes the interval: a maximum would keep a spike alive after
/// the work stopped. Returns the newest stamp seen, for the next call.
fn nvidia_rows(
    memory: &[(u32, Option<u64>)],
    samples: Option<&[ProcessUtilizationSample]>,
) -> (HashMap<u32, CardUse>, Option<u64>) {
    let mut rows: HashMap<u32, CardUse> = HashMap::new();
    for (pid, bytes) in memory {
        let row = rows.entry(*pid).or_insert(CardUse { busy: samples.map(|_| 0.0), vram: None });
        // A process on the graphics and the compute list is reported twice with one figure.
        row.vram = match (row.vram, *bytes) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (left, right) => left.or(right),
        };
    }
    let mut newest_per_pid: HashMap<u32, &ProcessUtilizationSample> = HashMap::new();
    let mut newest: Option<u64> = None;
    for sample in samples.into_iter().flatten() {
        newest = Some(newest.map_or(sample.timestamp, |seen| seen.max(sample.timestamp)));
        let latest = newest_per_pid.entry(sample.pid).or_insert(sample);
        if sample.timestamp > latest.timestamp {
            *latest = sample;
        }
    }
    for (pid, sample) in newest_per_pid {
        rows.entry(pid).or_default().busy = Some(sample.sm_util as f32 / 100.0);
    }
    (rows, newest)
}

#[cfg(test)]
mod tests;
