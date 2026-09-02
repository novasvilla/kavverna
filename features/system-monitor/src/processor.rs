use std::path::{Path, PathBuf};

/// What the machine calls its processor, and how much of it there is. Read once: a chip does
/// not change while the program runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Processor {
    /// The marketing name, tidied of the padding vendors put in it.
    pub name: String,
    pub cores: usize,
    pub threads: usize,
    clocks: Vec<PathBuf>,
}

impl Processor {
    pub fn discover() -> Self {
        Self::discover_in(Path::new("/proc"), Path::new("/sys/devices/system/cpu"))
    }

    pub fn discover_in(proc_root: &Path, cpu_root: &Path) -> Self {
        let described = std::fs::read_to_string(proc_root.join("cpuinfo")).unwrap_or_default();
        let (name, cores, threads) = describe(&described);
        Self { name, cores, threads, clocks: clock_files(cpu_root) }
    }

    /// The fastest core right now, which is the number a boost clock is about. `None` when the
    /// kernel exposes no frequency for this processor, as it does not inside a container.
    pub fn speed_ghz(&self) -> Option<f32> {
        self.clocks
            .iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .filter_map(|value| value.trim().parse::<f64>().ok())
            .max_by(f64::total_cmp)
            .map(|kilohertz| (kilohertz / 1_000_000.0) as f32)
    }
}

/// `scaling_cur_freq` is what the governor asked for; `cpuinfo_cur_freq` is what the hardware
/// reports and needs privileges on some drivers, so the governor's figure is the one every
/// machine can read.
fn clock_files(cpu_root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cpu_root) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path().join("cpufreq/scaling_cur_freq"))
        .filter(|path| path.exists())
        .collect();
    found.sort();
    found
}

/// Vendors pad the name with their own words: "AMD Ryzen 7 9700X 8-Core Processor" says the
/// core count twice over once the count is shown beside it, and Intel writes "(R)" and "CPU".
fn describe(cpuinfo: &str) -> (String, usize, usize) {
    let field = |name: &str| {
        cpuinfo.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim().eq(name).then(|| value.trim().to_owned())
        })
    };
    let threads = cpuinfo.lines().filter(|line| line.starts_with("processor")).count();
    let cores = field("cpu cores").and_then(|value| value.parse().ok()).unwrap_or(threads);
    let name = field("model name").map(|name| tidy(&name)).unwrap_or_default();
    (name, cores, threads)
}

fn tidy(name: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    for word in name.split_whitespace() {
        let plain = word.trim_end_matches("(R)").trim_end_matches("(TM)").trim_end_matches("(tm)");
        let padding = plain.eq_ignore_ascii_case("cpu")
            || plain.eq_ignore_ascii_case("processor")
            || plain.ends_with("-Core")
            || plain.eq_ignore_ascii_case("with");
        if padding {
            if plain.eq_ignore_ascii_case("with") {
                break;
            }
            continue;
        }
        if !plain.is_empty() {
            kept.push(plain);
        }
    }
    let name = kept.join(" ");
    match name.split_once(" @ ") {
        Some((before, _)) => before.trim().to_owned(),
        None => name,
    }
}

/// Cumulative jiffies since boot. Usage is only meaningful as the difference between two
/// readings, so a single sample says nothing on its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuTicks {
    busy: u64,
    idle: u64,
}

impl CpuTicks {
    /// Fields are user, nice, system, idle, iowait, irq, softirq, steal, guest, guest_nice.
    /// Waiting on I/O counts as idle: the core is available, and calling it busy makes a
    /// disk-bound machine look compute-bound.
    fn from_fields(fields: &[u64]) -> Self {
        let idle = fields.get(3).copied().unwrap_or(0) + fields.get(4).copied().unwrap_or(0);
        let busy: u64 = fields
            .iter()
            .take(8)
            .enumerate()
            .filter(|(index, _)| *index != 3 && *index != 4)
            .map(|(_, value)| *value)
            .sum();

        Self { busy, idle }
    }

    /// `None` until a second reading exists, or when the counters have not moved.
    pub fn load_since(self, earlier: Self) -> Option<f32> {
        let busy = self.busy.checked_sub(earlier.busy)?;
        let idle = self.idle.checked_sub(earlier.idle)?;
        let total = busy + idle;

        (total > 0).then(|| (busy as f32 / total as f32).clamp(0.0, 1.0))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProcessorTicks {
    pub total: CpuTicks,
    pub cores: Vec<CpuTicks>,
}

/// Parses `/proc/stat`. Lines after the per-core ones are not about processor time.
pub fn parse_stat(contents: &str) -> ProcessorTicks {
    let mut total = CpuTicks::default();
    let mut cores = Vec::new();

    for line in contents.lines() {
        let Some(rest) = line.strip_prefix("cpu") else {
            break;
        };
        let (label, numbers) = match rest.split_once(char::is_whitespace) {
            Some(parts) => parts,
            None => continue,
        };

        let fields: Vec<u64> =
            numbers.split_whitespace().filter_map(|value| value.parse().ok()).collect();
        let ticks = CpuTicks::from_fields(&fields);

        if label.is_empty() {
            total = ticks;
        } else {
            cores.push(ticks);
        }
    }

    ProcessorTicks { total, cores }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The opening lines of a real `/proc/stat` on the target machine.
    const SAMPLE: &str = "cpu  2146306 46507 182693 9022048 2251420 56910 14963 0 0 0
cpu0 201271 4629 18397 10712 615391 2230 1719 0 0 0
cpu1 243667 7485 19929 442497 138743 1904 1538 0 0 0
cpu2 194868 4487 16913 469221 168194 1890 1184 0 0 0
intr 123456 0 0
ctxt 987654
";

    #[test]
    fn the_total_and_every_core_are_read() {
        let ticks = parse_stat(SAMPLE);

        assert_eq!(ticks.cores.len(), 3);
        assert_eq!(ticks.total.busy, 2146306 + 46507 + 182693 + 56910 + 14963);
        assert_eq!(ticks.total.idle, 9022048 + 2251420);
    }

    #[test]
    fn lines_after_the_cores_are_left_alone() {
        assert_eq!(parse_stat(SAMPLE).cores.len(), 3);
        assert_eq!(parse_stat("").cores.len(), 0);
    }

    #[test]
    fn a_single_reading_yields_no_load() {
        let ticks = parse_stat(SAMPLE);

        assert_eq!(ticks.total.load_since(ticks.total), None);
    }

    #[test]
    fn load_is_the_busy_share_of_the_interval() {
        let earlier = CpuTicks { busy: 100, idle: 900 };
        let later = CpuTicks { busy: 400, idle: 1600 };

        assert_eq!(later.load_since(earlier), Some(0.3));
    }

    /// Counters reset when a core is hotplugged, and a wrapped subtraction would read as a
    /// gigantic spike.
    #[test]
    fn counters_going_backwards_report_nothing_rather_than_a_spike() {
        let earlier = CpuTicks { busy: 500, idle: 500 };
        let later = CpuTicks { busy: 100, idle: 100 };

        assert_eq!(later.load_since(earlier), None);
    }

    #[test]
    fn waiting_on_disk_is_not_counted_as_busy() {
        let waiting = CpuTicks::from_fields(&[0, 0, 0, 0, 1000, 0, 0, 0]);

        assert_eq!(waiting.busy, 0);
        assert_eq!(waiting.idle, 1000);
    }
    /// The line every AMD chip carries, and the shape Intel writes instead.
    #[test]
    fn a_processor_is_named_the_way_a_person_would_name_it() {
        let amd = "processor\t: 0\nmodel name\t: AMD Ryzen 7 9700X 8-Core Processor\ncpu cores\t: 8\nprocessor\t: 1\n";
        assert_eq!(describe(amd), ("AMD Ryzen 7 9700X".into(), 8, 2));

        let intel = "processor\t: 0\nmodel name\t: Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz\ncpu cores\t: 6\n";
        assert_eq!(describe(intel), ("Intel Core i7-9750H".into(), 6, 1));

        let graphics = "processor\t: 0\nmodel name\t: AMD Ryzen 5 5600G with Radeon Graphics\ncpu cores\t: 6\n";
        assert_eq!(describe(graphics).0, "AMD Ryzen 5 5600G");
    }

    #[test]
    fn a_machine_that_says_nothing_about_its_processor_is_left_blank() {
        assert_eq!(describe(""), (String::new(), 0, 0));
        assert_eq!(describe("processor\t: 0\nprocessor\t: 1\n"), (String::new(), 2, 2));
    }

    /// The kernel exposes one clock file per core, and the fastest is the boost figure.
    #[test]
    fn the_speed_is_the_fastest_core_the_kernel_reports() {
        let room = tempfile::tempdir().unwrap();
        let proc_root = room.path().join("proc");
        let cpu_root = room.path().join("cpu");
        std::fs::create_dir_all(&proc_root).unwrap();
        std::fs::write(
            proc_root.join("cpuinfo"),
            "processor\t: 0\nmodel name\t: AMD Ryzen 7 9700X 8-Core Processor\ncpu cores\t: 8\n",
        )
        .unwrap();
        for (core, kilohertz) in [("cpu0", "4200000"), ("cpu1", "5558825")] {
            let dir = cpu_root.join(core).join("cpufreq");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("scaling_cur_freq"), kilohertz).unwrap();
        }

        let processor = Processor::discover_in(&proc_root, &cpu_root);
        assert_eq!(processor.name, "AMD Ryzen 7 9700X");
        assert_eq!((processor.cores, processor.threads), (8, 1));
        assert!(processor.speed_ghz().is_some_and(|ghz| (ghz - 5.558825).abs() < 0.001));

        let bare = Processor::discover_in(&proc_root, &room.path().join("nothing"));
        assert_eq!(bare.speed_ghz(), None);
    }
}
