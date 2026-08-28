use std::path::{Path, PathBuf};

/// hwmon numbering is not stable across boots, so a sensor is always found by the chip's
/// `name` file rather than by its `hwmonN` directory.
#[derive(Debug, Clone)]
pub struct Sensor {
    pub chip: String,
    pub path: PathBuf,
}

#[derive(Debug, Default)]
pub struct Thermometer {
    sensors: Vec<Sensor>,
}

/// The chips that report processor package temperature, in the order they should be tried.
const PROCESSOR_CHIPS: [&str; 3] = ["k10temp", "coretemp", "zenpower"];

/// AMD reports the control temperature under this label, which is the one to show.
const PROCESSOR_LABELS: [&str; 3] = ["Tctl", "Tdie", "Package id 0"];

const HWMON: &str = "/sys/class/hwmon";

impl Thermometer {
    pub fn discover() -> Self {
        Self::discover_in(Path::new(HWMON))
    }

    /// Takes the root so a test can point it at a tree it wrote itself. Reading the machine's own
    /// `/sys` from a unit test makes the result depend on which machine runs it.
    pub fn discover_in(root: &Path) -> Self {
        let Ok(entries) = std::fs::read_dir(root) else {
            return Self::default();
        };

        let sensors = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let chip = std::fs::read_to_string(path.join("name")).ok()?.trim().to_owned();
                Some(Sensor { chip, path })
            })
            .collect();

        Self { sensors }
    }

    pub fn chips(&self) -> Vec<String> {
        self.sensors.iter().map(|sensor| sensor.chip.clone()).collect()
    }

    pub fn chip(&self, name: &str) -> Option<&Sensor> {
        self.sensors.iter().find(|sensor| sensor.chip == name)
    }

    pub fn processor_celsius(&self) -> Option<f32> {
        PROCESSOR_CHIPS
            .iter()
            .find_map(|chip| self.chip(chip))
            .and_then(|sensor| labelled_celsius(&sensor.path, &PROCESSOR_LABELS))
    }
}

/// Reads the input whose label matches, falling back to the first input when the chip does
/// not label its readings.
fn labelled_celsius(path: &Path, wanted: &[&str]) -> Option<f32> {
    for index in 1..=16 {
        let Some(label) = std::fs::read_to_string(path.join(format!("temp{index}_label"))).ok()
        else {
            continue;
        };
        if wanted.contains(&label.trim()) {
            return millicelsius(&path.join(format!("temp{index}_input")));
        }
    }

    millicelsius(&path.join("temp1_input"))
}

fn millicelsius(path: &Path) -> Option<f32> {
    let value: i64 = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
    Some(value as f32 / 1000.0)
}

/// hwmon labels carry a trailing newline, which would defeat a naive comparison.
pub fn parse_label(raw: &str) -> &str {
    raw.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_compared_without_its_newline() {
        assert_eq!(parse_label("Tctl\n"), "Tctl");
        assert_eq!(parse_label("  Package id 0  \n"), "Package id 0");
    }

    #[test]
    fn the_processor_chips_are_tried_in_order() {
        assert_eq!(PROCESSOR_CHIPS[0], "k10temp");
        assert!(PROCESSOR_CHIPS.contains(&"coretemp"));
    }

    /// Naming the AMD control temperature matters: a chip reports several inputs and only
    /// this one is the figure people mean by "CPU temperature".
    #[test]
    fn the_control_temperature_is_the_one_looked_for() {
        assert_eq!(PROCESSOR_LABELS[0], "Tctl");
    }

    fn chip(root: &Path, directory: &str, name: &str, readings: &[(&str, &str)]) {
        let path = root.join(directory);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("name"), format!("{name}\n")).unwrap();
        for (file, value) in readings {
            std::fs::write(path.join(file), format!("{value}\n")).unwrap();
        }
    }

    #[test]
    fn the_control_temperature_is_read_from_the_processor_chip() {
        let room = tempfile::tempdir().unwrap();
        chip(room.path(), "hwmon2", "nvme", &[("temp1_input", "38000")]);
        chip(
            room.path(),
            "hwmon0",
            "k10temp",
            &[
                ("temp1_label", "Tctl"),
                ("temp1_input", "51500"),
                ("temp2_label", "Tccd1"),
                ("temp2_input", "47000"),
            ],
        );

        let thermometer = Thermometer::discover_in(room.path());

        assert_eq!(thermometer.processor_celsius(), Some(51.5));
    }

    /// A machine with sensors but none of the processor chips reports nothing rather than
    /// handing back whatever it did find, which is how a disk temperature would end up on the
    /// processor row.
    #[test]
    fn a_machine_with_no_processor_chip_reports_no_temperature() {
        let room = tempfile::tempdir().unwrap();
        chip(room.path(), "hwmon0", "nvme", &[("temp1_input", "38000")]);

        let thermometer = Thermometer::discover_in(room.path());

        assert_eq!(thermometer.chips(), vec!["nvme"]);
        assert_eq!(thermometer.processor_celsius(), None);
    }

    #[test]
    fn a_chip_that_labels_nothing_falls_back_to_its_first_reading() {
        let room = tempfile::tempdir().unwrap();
        chip(room.path(), "hwmon0", "coretemp", &[("temp1_input", "44000")]);

        assert_eq!(Thermometer::discover_in(room.path()).processor_celsius(), Some(44.0));
    }

    #[test]
    fn nothing_at_the_root_is_not_an_error() {
        let room = tempfile::tempdir().unwrap();

        assert!(Thermometer::discover_in(&room.path().join("absent")).chips().is_empty());
    }
}
