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
}
