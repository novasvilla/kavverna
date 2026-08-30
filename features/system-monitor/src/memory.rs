const KIB: u64 = 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryReading {
    pub total: u64,
    /// Everything the kernel is not willing to hand back on demand.
    pub in_use: u64,
    /// What applications themselves hold, which excludes the page cache. The smaller of the
    /// two figures, and the one that answers "is something leaking".
    pub held_by_apps: u64,
    pub cached: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

impl MemoryReading {
    pub fn used_fraction(self) -> f32 {
        fraction(self.in_use, self.total)
    }

    pub fn apps_fraction(self) -> f32 {
        fraction(self.held_by_apps, self.total)
    }

    pub fn swap_fraction(self) -> f32 {
        fraction(self.swap_used, self.swap_total)
    }
}

fn fraction(part: u64, whole: u64) -> f32 {
    if whole == 0 { 0.0 } else { (part as f32 / whole as f32).clamp(0.0, 1.0) }
}

/// Parses `/proc/meminfo`, whose values are in kibibytes.
pub fn parse_meminfo(contents: &str) -> MemoryReading {
    let field = |name: &str| -> u64 {
        contents
            .lines()
            .find(|line| line.starts_with(name) && line.as_bytes().get(name.len()) == Some(&b':'))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
            .map(|kib| kib * KIB)
            .unwrap_or(0)
    };

    let total = field("MemTotal");
    let available = field("MemAvailable");
    let swap_total = field("SwapTotal");

    MemoryReading {
        total,
        // Not `total - free`: the kernel keeps a large page cache it will surrender on
        // demand, and counting that as used makes every Linux machine look full.
        in_use: total.saturating_sub(available),
        held_by_apps: field("AnonPages") + field("Shmem"),
        cached: field("Cached") + field("SReclaimable"),
        swap_total,
        swap_used: swap_total.saturating_sub(field("SwapFree")),
    }
}

/// Swap that lives in compressed RAM rather than on a disk. Reporting only what
/// `/proc/meminfo` says would claim gigabytes of swap on a machine whose disk is untouched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompressedSwap {
    pub device: String,
    /// What applications believe they swapped out.
    pub stored: u64,
    /// What it actually costs in RAM once compressed.
    pub ram_cost: u64,
}

impl CompressedSwap {
    pub fn ratio(&self) -> f32 {
        if self.ram_cost == 0 { 0.0 } else { self.stored as f32 / self.ram_cost as f32 }
    }
}

/// Parses a zram `mm_stat` line: orig_data_size, compr_data_size, mem_used_total, then
/// counters this does not need.
pub fn parse_mm_stat(device: &str, contents: &str) -> Option<CompressedSwap> {
    let fields: Vec<u64> =
        contents.split_whitespace().filter_map(|value| value.parse().ok()).collect();

    Some(CompressedSwap {
        device: device.to_owned(),
        stored: *fields.first()?,
        ram_cost: *fields.get(2)?,
    })
}

const BLOCK_DEVICES: &str = "/sys/block";

pub fn discover_compressed_swap() -> Vec<CompressedSwap> {
    discover_compressed_swap_in(std::path::Path::new(BLOCK_DEVICES))
}

/// Takes the root so a test can point it at a tree it wrote itself. A machine with no zram would
/// otherwise let the test pass by finding nothing at all.
pub fn discover_compressed_swap_in(root: &std::path::Path) -> Vec<CompressedSwap> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("zram"))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let contents = std::fs::read_to_string(entry.path().join("mm_stat")).ok()?;
            parse_mm_stat(&name, &contents)
        })
        .collect()
}

/// Stall time as a share of the last window. Linux reports this directly, where macOS only
/// exposes a three-state hint.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MemoryPressure {
    /// Share of the last ten seconds in which every task was stalled on memory.
    pub full_ten_seconds: f32,
    pub some_ten_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureLevel {
    Easy,
    Noticeable,
    Severe,
}

impl MemoryPressure {
    pub fn level(self) -> PressureLevel {
        match self.full_ten_seconds {
            value if value >= 10.0 => PressureLevel::Severe,
            value if value >= 1.0 => PressureLevel::Noticeable,
            _ => PressureLevel::Easy,
        }
    }
}

/// Both lines, though `full` is the one the card leads with: `some` counts one task stalling
/// while others run, which is life, and `full` is every task stuck at once.
pub fn parse_pressure(contents: &str) -> MemoryPressure {
    let read = |prefix: &str| -> f32 {
        contents
            .lines()
            .find(|line| line.starts_with(prefix))
            .and_then(|line| {
                line.split_whitespace()
                    .find_map(|field| field.strip_prefix("avg10="))
                    .and_then(|value| value.parse().ok())
            })
            .unwrap_or(0.0)
    };

    MemoryPressure { full_ten_seconds: read("full"), some_ten_seconds: read("some") }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read from `/proc/meminfo` on the target machine.
    const SAMPLE: &str = "MemTotal:       31987280 kB
MemFree:        11083788 kB
MemAvailable:   22321208 kB
Buffers:              32 kB
Cached:         11527476 kB
SwapCached:        12345 kB
SwapTotal:      31986684 kB
SwapFree:       27016012 kB
AnonPages:       5403524 kB
Shmem:            283776 kB
SReclaimable:     581160 kB
";

    #[test]
    fn the_headline_figures_match_the_machine() {
        let memory = parse_meminfo(SAMPLE);

        assert_eq!(memory.total, 31987280 * KIB);
        assert_eq!(memory.in_use, (31987280 - 22321208) * KIB);
        assert_eq!(memory.held_by_apps, (5403524 + 283776) * KIB);
        assert_eq!(memory.swap_used, (31986684 - 27016012) * KIB);
    }

    /// Free memory is not the same as available memory, and the difference is most of the
    /// page cache.
    #[test]
    fn the_page_cache_is_not_counted_as_used() {
        let memory = parse_meminfo(SAMPLE);
        let naive = (31987280u64 - 11083788) * KIB;

        assert!(memory.in_use < naive);
        assert!(memory.used_fraction() < 0.35);
    }

    #[test]
    fn applications_hold_less_than_the_total_in_use() {
        let memory = parse_meminfo(SAMPLE);

        assert!(memory.held_by_apps < memory.in_use);
    }

    /// A field that is missing on some kernels must not poison the rest of the reading.
    #[test]
    fn a_missing_field_reads_as_zero() {
        let memory = parse_meminfo("MemTotal: 1024 kB\n");

        assert_eq!(memory.total, 1024 * KIB);
        assert_eq!(memory.held_by_apps, 0);
        assert_eq!(memory.swap_fraction(), 0.0);
    }

    /// `SwapCached` starts with `Swap` too, so a prefix match alone would read the wrong line.
    #[test]
    fn a_field_name_that_prefixes_another_is_not_confused_for_it() {
        assert_eq!(parse_meminfo(SAMPLE).swap_total, 31986684 * KIB);
    }

    /// The real `mm_stat` of this machine's zram device.
    #[test]
    fn compressed_swap_reports_what_it_really_costs() {
        let sample = "4299317248 1121926831 1167548416 0 2478813184 55690 88269 44859 201557";
        let swap = parse_mm_stat("zram0", sample).expect("parsed");

        assert_eq!(swap.stored, 4_299_317_248);
        assert_eq!(swap.ram_cost, 1_167_548_416);
        assert!((swap.ratio() - 3.68).abs() < 0.01, "ratio was {}", swap.ratio());
    }

    /// What meminfo calls swap is nearly four times what it costs, so showing the meminfo
    /// figure alone reads as disk thrashing on a machine with no disk swap at all.
    #[test]
    fn the_cost_is_far_below_what_meminfo_reports() {
        let swap =
            parse_mm_stat("zram0", "4299317248 1121926831 1167548416 0 0 0 0 0").expect("parsed");

        assert!(swap.ram_cost < swap.stored / 3);
    }

    #[test]
    fn a_truncated_mm_stat_is_ignored_rather_than_half_read() {
        assert_eq!(parse_mm_stat("zram0", "123 456"), None);
        assert_eq!(parse_mm_stat("zram0", ""), None);
    }

    #[test]
    fn only_the_zram_devices_are_read_out_of_the_block_directory() {
        let room = tempfile::tempdir().unwrap();
        let device = |name: &str, stat: Option<&str>| {
            let path = room.path().join(name);
            std::fs::create_dir_all(&path).unwrap();
            if let Some(stat) = stat {
                std::fs::write(path.join("mm_stat"), stat).unwrap();
            }
        };
        device("zram0", Some("4299317248 1121926831 1167548416 0 0 0 0 0"));
        device("nvme0n1", None);
        device("sda", None);

        let found = discover_compressed_swap_in(room.path());

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].device, "zram0");
    }

    #[test]
    fn pressure_is_read_from_the_full_line() {
        let sample = "some avg10=4.25 avg60=1.00 avg300=0.20 total=16073\n\
                      full avg10=12.50 avg60=0.50 avg300=0.10 total=15041\n";
        let pressure = parse_pressure(sample);

        assert_eq!(pressure.some_ten_seconds, 4.25);
        assert_eq!(pressure.full_ten_seconds, 12.5);
        assert_eq!(pressure.level(), PressureLevel::Severe);
    }

    #[test]
    fn an_idle_machine_reports_easy_pressure() {
        let sample = "some avg10=0.00 avg60=0.00 avg300=0.00 total=0\n\
                      full avg10=0.00 avg60=0.00 avg300=0.00 total=0\n";

        assert_eq!(parse_pressure(sample).level(), PressureLevel::Easy);
    }

    #[test]
    fn pressure_thresholds_sit_where_they_are_documented() {
        let at = |value| MemoryPressure { full_ten_seconds: value, some_ten_seconds: 0.0 }.level();

        assert_eq!(at(0.99), PressureLevel::Easy);
        assert_eq!(at(1.0), PressureLevel::Noticeable);
        assert_eq!(at(9.99), PressureLevel::Noticeable);
        assert_eq!(at(10.0), PressureLevel::Severe);
    }
}
