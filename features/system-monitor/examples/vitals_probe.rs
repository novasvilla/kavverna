//! Prints a reading so it can be compared against nvidia-smi, sensors and free.

use system_monitor::Vitalsigns;
use std::time::Duration;

fn gib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

fn main() {
    let mut vitals = Vitalsigns::open();
    vitals.sample();
    std::thread::sleep(Duration::from_millis(700));
    let reading = vitals.sample();

    println!("PROCESSOR");
    match reading.cpu_load {
        Some(load) => println!("  load        {:.1}%", load * 100.0),
        None => println!("  load        (needs two readings)"),
    }
    println!("  cores       {}", reading.core_loads.len());
    match reading.cpu_temperature {
        Some(celsius) => println!("  temperature {celsius:.1} C"),
        None => println!("  temperature unavailable"),
    }

    println!("\nMEMORY");
    println!(
        "  in use      {:.2} GiB of {:.2} GiB  ({:.1}%)",
        gib(reading.memory.in_use),
        gib(reading.memory.total),
        reading.memory.used_fraction() * 100.0
    );
    println!(
        "  apps hold   {:.2} GiB  ({:.1}%)",
        gib(reading.memory.held_by_apps),
        reading.memory.apps_fraction() * 100.0
    );
    println!("  cached      {:.2} GiB", gib(reading.memory.cached));
    println!(
        "  swap        {:.2} GiB of {:.2} GiB",
        gib(reading.memory.swap_used),
        gib(reading.memory.swap_total)
    );
    println!(
        "  pressure    full avg10 {:.2}  ({:?})",
        reading.pressure.full_ten_seconds,
        reading.pressure.level()
    );

    println!("\nGRAPHICS");
    for card in &reading.graphics.cards {
        let name = &card.reading.name;
        let preferred = reading.graphics.preferred().is_some_and(|p| p.reading.name == *name);
        println!("  {} {name}  [{:?}]", if preferred { "*" } else { " " }, card.role);

        let show = |label: &str, value: Option<String>| {
            println!("      {label:<12}{}", value.unwrap_or_else(|| "unavailable".into()))
        };
        show("usage", card.reading.utilisation.map(|v| format!("{:.0}%", v * 100.0)));
        show("temperature", card.reading.temperature.map(|v| format!("{v:.0} C")));
        show("power", card.reading.power_watts.map(|v| format!("{v:.1} W")));
        show(
            "VRAM",
            card.reading.memory_used.zip(card.reading.memory_total).map(|(used, total)| {
                format!(
                    "{:.0} MiB of {:.0} MiB  ({:.1}%)",
                    used as f64 / 1048576.0,
                    total as f64 / 1048576.0,
                    card.reading.memory_fraction().unwrap_or(0.0) * 100.0
                )
            }),
        );
    }
}
