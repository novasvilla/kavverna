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

impl Thermometer {
    pub fn discover() -> Self {
        let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") else {
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

    #[test]
    fn discovery_finds_the_chips_on_this_machine() {
        let thermometer = Thermometer::discover();
        let chips = thermometer.chips();

        if chips.is_empty() {
            eprintln!("skipped: no hwmon on this machine");
            return;
        }

        assert!(
            thermometer.processor_celsius().is_some_and(|c| (10.0..120.0).contains(&c)),
            "processor temperature out of range, chips: {chips:?}"
        );
    }
}
