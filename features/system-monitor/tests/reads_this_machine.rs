//! What the readers report when pointed at the real `/proc` and `/sys` rather than at a fixture.
//!
//! Ignored by default: the answers depend on the hardware underneath, so a build machine with no
//! processor sensor would fail these for being a build machine. Run them on a desktop with
//! `cargo test -p system-monitor -- --include-ignored`.

use std::thread::sleep;
use std::time::Duration;
use system_monitor::{Thermometer, Vitalsigns};

#[test]
#[ignore = "reads the machine's own sensors"]
fn the_processor_temperature_is_a_believable_one() {
    let thermometer = Thermometer::discover();
    let chips = thermometer.chips();

    assert!(!chips.is_empty(), "no hwmon on this machine at all");
    let celsius = thermometer
        .processor_celsius()
        .unwrap_or_else(|| panic!("no processor chip among {chips:?}"));

    assert!((10.0..120.0).contains(&celsius), "processor read {celsius} degrees");
}

/// Load is a difference between two readings, so the first sample cannot report one. Getting a
/// figure out of the first call would mean it was inventing a baseline.
#[test]
#[ignore = "reads the machine's own /proc"]
fn load_arrives_on_the_second_sample_and_not_the_first() {
    let mut vitals = Vitalsigns::open();

    assert!(vitals.sample().cpu_load.is_none());

    sleep(Duration::from_millis(120));
    let second = vitals.sample();

    let load = second.cpu_load.expect("a second sample reports load");
    assert!((0.0..=1.0).contains(&load), "load was {load}");
    assert!(!second.core_loads.is_empty(), "no per core figures");
}

#[test]
#[ignore = "reads the machine's own /proc/meminfo"]
fn memory_adds_up_the_way_the_kernel_reports_it() {
    let reading = Vitalsigns::open().sample().memory;

    assert!(reading.total > 0, "no total memory");
    assert!(reading.in_use <= reading.total, "in use exceeds total");
    assert!(reading.held_by_apps <= reading.in_use, "applications hold more than is in use");
}
