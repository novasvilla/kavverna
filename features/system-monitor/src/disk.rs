use nix::sys::stat::{major, minor};
use nix::sys::statvfs::statvfs;
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct DiskReading {
    pub name: String,
    pub mount: PathBuf,
    pub total: Option<u64>,
    pub available: Option<u64>,
    pub read_per_second: Option<f64>,
    pub written_per_second: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Volume {
    pub name: String,
    pub mount: PathBuf,
    pub device: PathBuf,
}

#[derive(Default)]
pub struct DiskSampler {
    previous: BTreeMap<(u64, u64), (u64, u64)>,
    taken_at: Option<Instant>,
}

impl DiskSampler {
    pub fn sample(&mut self) -> Option<Vec<DiskReading>> {
        let mounts = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
        let stats = std::fs::read_to_string("/proc/diskstats").ok();
        let counters = stats.as_deref().map(parse_diskstats).unwrap_or_default();
        let now = Instant::now();
        let elapsed = self.taken_at.map(|before| now.saturating_duration_since(before));
        let mut current = BTreeMap::new();
        let mut readings = Vec::new();

        for volume in volumes(&mounts) {
            let size = statvfs(&volume.mount).ok().map(|stat| {
                let block = stat.fragment_size();
                (stat.blocks().saturating_mul(block), stat.blocks_available().saturating_mul(block))
            });
            let device = std::fs::metadata(&volume.device).ok().and_then(|metadata| {
                metadata.file_type().is_block_device().then(|| {
                    let id = metadata.rdev();
                    (major(id), minor(id))
                })
            });
            let mut rates = (None, None);
            if let Some(device) = device {
                if let Some(&(read, written)) = counters.get(&device) {
                    rates =
                        rates_since(self.previous.get(&device).copied(), (read, written), elapsed);
                    current.insert(device, (read, written));
                }
            }
            readings.push(DiskReading {
                name: volume.name,
                mount: volume.mount,
                total: size.map(|(total, _)| total),
                available: size.map(|(_, available)| available),
                read_per_second: rates.0,
                written_per_second: rates.1,
            });
        }

        self.previous = current;
        self.taken_at = Some(now);
        Some(readings)
    }
}

fn valid_gap(elapsed: Duration) -> bool {
    elapsed >= Duration::from_millis(100) && elapsed <= Duration::from_secs(10)
}

fn rates_since(
    before: Option<(u64, u64)>,
    now: (u64, u64),
    elapsed: Option<Duration>,
) -> (Option<f64>, Option<f64>) {
    let (Some((before_read, before_written)), Some(elapsed)) = (before, elapsed) else {
        return (None, None);
    };
    if !valid_gap(elapsed) {
        return (None, None);
    }
    let seconds = elapsed.as_secs_f64();
    (
        now.0.checked_sub(before_read).map(|bytes| bytes as f64 / seconds),
        now.1.checked_sub(before_written).map(|bytes| bytes as f64 / seconds),
    )
}

pub fn parse_diskstats(contents: &str) -> BTreeMap<(u64, u64), (u64, u64)> {
    let mut counters = BTreeMap::new();
    for line in contents.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        let parsed = (
            fields[0].parse(),
            fields[1].parse(),
            fields[5].parse::<u64>(),
            fields[9].parse::<u64>(),
        );
        if let (Ok(major), Ok(minor), Ok(read), Ok(written)) = parsed {
            counters
                .insert((major, minor), (read.saturating_mul(512), written.saturating_mul(512)));
        }
    }
    counters
}

pub fn volumes(mountinfo: &str) -> Vec<Volume> {
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    for line in mountinfo.lines() {
        let Some((before, after)) = line.split_once(" - ") else { continue };
        let fields: Vec<_> = before.split_whitespace().collect();
        let source: Vec<_> = after.split_whitespace().collect();
        if fields.len() < 5 || source.len() < 2 {
            continue;
        }
        if !matches!(
            source[0],
            "btrfs" | "ext4" | "xfs" | "f2fs" | "vfat" | "exfat" | "ntfs" | "ntfs3"
        ) {
            continue;
        }
        let mount = PathBuf::from(std::ffi::OsString::from_vec(unescape(fields[4])));
        if mount != Path::new("/")
            && mount != Path::new("/home")
            && !mount.starts_with("/run/media")
            && !mount.starts_with("/media")
            && !mount.starts_with("/mnt")
        {
            continue;
        }
        let device = PathBuf::from(source[1]);
        if !device.starts_with("/dev") || !seen.insert(device.clone()) {
            continue;
        }
        let name = if mount == Path::new("/") {
            "System".into()
        } else if mount == Path::new("/home") {
            "Home".into()
        } else {
            mount
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| mount.display().to_string())
        };
        found.push(Volume { name, mount, device });
    }
    found
}

fn unescape(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && index + 3 < bytes.len()
            && bytes[index + 1..index + 4].iter().all(|digit| (b'0'..=b'7').contains(digit))
        {
            let encoded = u16::from(bytes[index + 1] - b'0') * 64
                + u16::from(bytes[index + 2] - b'0') * 8
                + u16::from(bytes[index + 3] - b'0');
            if let Ok(encoded) = u8::try_from(encoded) {
                result.push(encoded);
                index += 4;
                continue;
            }
        } else {
            result.push(bytes[index]);
            index += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_and_distinct_home_are_selected_without_virtual_filesystems() {
        let mounts = "1 0 0:33 /@ / rw - btrfs /dev/nvme0n1p2 rw\n2 1 0:33 /@home /home rw - btrfs /dev/nvme0n1p2 rw\n3 1 259:1 / /boot rw - vfat /dev/nvme0n1p1 rw\n4 1 8:1 / /run/media/A\\040B rw - ext4 /dev/sda1 rw\n5 1 0:1 / /proc rw - proc proc rw";
        let found = volumes(mounts);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "System");
        assert_eq!(found[1].name, "A B");
        assert_eq!(found[1].mount, PathBuf::from("/run/media/A B"));
    }

    #[test]
    fn diskstats_use_the_device_number_and_512_byte_sectors() {
        let stats = parse_diskstats("259 2 nvme0n1p2 10 0 20 1 30 0 40 1\n");
        assert_eq!(stats.get(&(259, 2)), Some(&(10_240, 20_480)));
    }

    #[test]
    fn disk_rates_need_a_baseline_and_ignore_resets_or_long_gaps() {
        assert_eq!(rates_since(None, (200, 300), Some(Duration::from_secs(2))), (None, None));
        assert_eq!(
            rates_since(Some((200, 300)), (400, 500), Some(Duration::from_secs(2))),
            (Some(100.0), Some(100.0))
        );
        assert_eq!(
            rates_since(Some((400, 500)), (10, 600), Some(Duration::from_secs(2))),
            (None, Some(50.0))
        );
        assert_eq!(
            rates_since(Some((10, 600)), (100, 700), Some(Duration::from_secs(30))),
            (None, None)
        );
    }
}
