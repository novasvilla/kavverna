use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuRole {
    /// The card that does the work, and the one the readouts should default to.
    Discrete,
    /// Built into the processor. Present, selectable, but rarely what you want to watch.
    Integrated,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuReading {
    pub name: String,
    pub utilisation: Option<f32>,
    pub temperature: Option<f32>,
    pub power_watts: Option<f32>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
}

impl GpuReading {
    pub fn memory_fraction(&self) -> Option<f32> {
        let (used, total) = (self.memory_used?, self.memory_total?);
        (total > 0).then(|| (used as f32 / total as f32).clamp(0.0, 1.0))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gpu {
    pub role: GpuRole,
    pub reading: GpuReading,
}

/// Never summed into one figure: two cards doing different work at once would read as a
/// meaningless average.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GraphicsReading {
    pub cards: Vec<Gpu>,
}

impl GraphicsReading {
    /// The discrete card when there is one, since that is what the machine renders with.
    pub fn preferred(&self) -> Option<&Gpu> {
        self.cards.iter().find(|card| card.role == GpuRole::Discrete).or_else(|| self.cards.first())
    }

    pub fn chosen(&self, name: Option<&str>) -> Option<&Gpu> {
        match name {
            Some(name) => self
                .cards
                .iter()
                .find(|card| card.reading.name == name)
                .or_else(|| self.preferred()),
            None => self.preferred(),
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.cards.iter().map(|card| card.reading.name.clone()).collect()
    }
}

/// An amdgpu card under `/sys/class/drm/cardN/device`, which reports everything through
/// small text files.
#[derive(Debug, Clone)]
pub struct SysfsCard {
    pub name: String,
    pub device: PathBuf,
    pub hwmon: Option<PathBuf>,
}

impl SysfsCard {
    pub fn read(&self) -> GpuReading {
        GpuReading {
            name: self.name.clone(),
            utilisation: number(&self.device.join("gpu_busy_percent")).map(|v| v as f32 / 100.0),
            temperature: self
                .hwmon
                .as_ref()
                .and_then(|hwmon| number(&hwmon.join("temp1_input")))
                .map(millis_to_units),
            power_watts: self
                .hwmon
                .as_ref()
                .and_then(|hwmon| number(&hwmon.join("power1_input")))
                .map(micros_to_units),
            memory_used: number(&self.device.join("mem_info_vram_used")),
            memory_total: number(&self.device.join("mem_info_vram_total")),
        }
    }
}

fn number(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// hwmon reports temperature in thousandths of a degree.
fn millis_to_units(value: u64) -> f32 {
    value as f32 / 1000.0
}

/// hwmon reports power in microwatts.
fn micros_to_units(value: u64) -> f32 {
    value as f32 / 1_000_000.0
}

const DRM: &str = "/sys/class/drm";

pub fn discover_sysfs_cards() -> Vec<SysfsCard> {
    discover_sysfs_cards_in(Path::new(DRM))
}

/// Takes the root so a test can point it at a tree it wrote itself, rather than at whichever
/// cards the machine running the test happens to have.
pub fn discover_sysfs_cards_in(root: &Path) -> Vec<SysfsCard> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut cards = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        // Connectors are named cardN-HDMI-A-1 and are not cards.
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }

        let device = entry.path().join("device");
        let driver = std::fs::read_link(device.join("driver"))
            .ok()
            .and_then(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();

        if driver != "amdgpu" {
            continue;
        }

        cards.push(SysfsCard {
            name: format!("AMD {name}"),
            hwmon: first_child(&device.join("hwmon")),
            device,
        });
    }

    cards
}

fn first_child(path: &Path) -> Option<PathBuf> {
    std::fs::read_dir(path).ok()?.flatten().next().map(|entry| entry.path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, role: GpuRole) -> Gpu {
        Gpu { role, reading: GpuReading { name: name.into(), ..Default::default() } }
    }

    #[test]
    fn only_amd_cards_are_taken_and_connectors_are_not_cards() {
        let room = tempfile::tempdir().unwrap();
        let entry = |name: &str, driver: Option<&str>| {
            let device = room.path().join(name).join("device");
            std::fs::create_dir_all(&device).unwrap();
            if let Some(driver) = driver {
                std::os::unix::fs::symlink(
                    format!("../../bus/pci/drivers/{driver}"),
                    device.join("driver"),
                )
                .unwrap();
            }
        };
        entry("card0", Some("amdgpu"));
        entry("card1", Some("nvidia"));
        entry("card0-HDMI-A-1", Some("amdgpu"));

        let cards = discover_sysfs_cards_in(room.path());

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].name, "AMD card0");
    }

    #[test]
    fn the_discrete_card_is_preferred_whatever_the_order() {
        let graphics = GraphicsReading {
            cards: vec![
                card("AMD card0", GpuRole::Integrated),
                card("RTX 5070", GpuRole::Discrete),
            ],
        };

        assert_eq!(graphics.preferred().unwrap().reading.name, "RTX 5070");
    }

    #[test]
    fn a_machine_with_only_an_integrated_card_still_reports_one() {
        let graphics = GraphicsReading { cards: vec![card("AMD card0", GpuRole::Integrated)] };

        assert_eq!(graphics.preferred().unwrap().reading.name, "AMD card0");
        assert!(GraphicsReading::default().preferred().is_none());
    }

    #[test]
    fn the_user_can_pin_a_card_by_name() {
        let graphics = GraphicsReading {
            cards: vec![
                card("RTX 5070", GpuRole::Discrete),
                card("AMD card0", GpuRole::Integrated),
            ],
        };

        assert_eq!(graphics.chosen(Some("AMD card0")).unwrap().reading.name, "AMD card0");
    }

    /// Unplugging a card should fall back rather than show nothing.
    #[test]
    fn a_pinned_card_that_is_gone_falls_back_to_the_preferred_one() {
        let graphics = GraphicsReading { cards: vec![card("RTX 5070", GpuRole::Discrete)] };

        assert_eq!(graphics.chosen(Some("Unplugged")).unwrap().reading.name, "RTX 5070");
    }

    #[test]
    fn memory_is_reported_as_a_share_of_the_card() {
        let reading = GpuReading {
            memory_used: Some(1992 * 1024 * 1024),
            memory_total: Some(12227 * 1024 * 1024),
            ..Default::default()
        };

        let fraction = reading.memory_fraction().expect("both figures present");
        assert!((fraction - 0.1629).abs() < 0.001, "got {fraction}");
    }

    #[test]
    fn a_card_that_reports_no_memory_reports_no_share() {
        assert_eq!(GpuReading::default().memory_fraction(), None);
        assert_eq!(
            GpuReading { memory_used: Some(1), memory_total: Some(0), ..Default::default() }
                .memory_fraction(),
            None
        );
    }

    #[test]
    fn hwmon_units_are_converted() {
        assert_eq!(millis_to_units(41000), 41.0);
        assert_eq!(micros_to_units(21_540_000), 21.54);
    }
}
