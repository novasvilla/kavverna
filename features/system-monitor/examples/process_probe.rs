//! Prints the applications spending each resource, and how long each request took, so the
//! figures can be compared against `ps`, `nvidia-smi pmon`, `iotop` and `ss -tulpn`.

use std::time::{Duration, Instant};
use system_monitor::{ProcessSampler, Reading, listening_sockets};

const LIMIT: usize = 8;

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn timed<T>(label: &str, run: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let value = run();
    println!("  [{label} took {:.2} ms]", started.elapsed().as_secs_f64() * 1000.0);
    value
}

fn main() {
    let opened = Instant::now();
    let mut sampler = ProcessSampler::open();
    println!("opened in {:.1} ms", opened.elapsed().as_secs_f64() * 1000.0);

    println!("\nPRIMING");
    let priming = timed("cpu, first", || sampler.cpu(LIMIT));
    assert!(matches!(priming, Reading::Priming));
    timed("disk, first", || sampler.disk(LIMIT));
    let cards = sampler.cards();
    for card in &cards {
        timed(&format!("{}, first", card.name), || sampler.graphics(&card.name, LIMIT));
    }
    std::thread::sleep(Duration::from_secs(2));

    println!("\nPROCESSOR");
    if let Reading::Ready(fold) = timed("cpu", || sampler.cpu(LIMIT)) {
        println!("  machine {:.1}%  ({} threads)", fold.machine_load * 100.0, fold.threads);
        for app in &fold.applications {
            println!(
                "  {:>6.1}%  {:>6.1}% of a core  {}  ({} processes)",
                app.value.of_machine * 100.0,
                app.value.of_one_core * 100.0,
                app.name,
                app.members.len()
            );
        }
        println!("  {:>6.1}%  elsewhere", fold.elsewhere * 100.0);
    }

    println!("\nMEMORY");
    let fold = timed("memory, cold", || sampler.memory(LIMIT));
    for app in &fold.applications {
        println!(
            "  {:>8.1} MiB  {:?}  {}  ({} processes)",
            mib(app.value.bytes),
            app.value.basis,
            app.name,
            app.members.len()
        );
    }
    timed("memory, warm", || sampler.memory(LIMIT));

    println!("\nDISK");
    if let Reading::Ready(fold) = timed("disk", || sampler.disk(LIMIT)) {
        for app in &fold.applications {
            println!(
                "  read {:>8.1} KiB/s  write {:>8.1} KiB/s  {}",
                app.value.read_per_second as f64 / 1024.0,
                app.value.written_per_second as f64 / 1024.0,
                app.name
            );
        }
        if fold.applications.is_empty() {
            println!("  nothing moving");
        }
    }

    for card in &cards {
        println!("\nGRAPHICS {}  {:?}  {:?}", card.name, card.role, card.pci_slot);
        let Some(fold) = timed(&card.name, || sampler.graphics(&card.name, LIMIT)) else {
            continue;
        };
        if fold.busy_priming {
            println!("  busy figures still priming");
        }
        for app in &fold.applications {
            let pids: Vec<u32> = app.members.iter().map(|member| member.key.pid).collect();
            println!(
                "  busy {:>6}  vram {:>8}  {}  {:?}",
                app.value.busy.map_or("-".into(), |busy| format!("{:.1}%", busy * 100.0)),
                app.value.vram.map_or("-".into(), |vram| format!("{:.1} MiB", mib(vram))),
                app.name,
                pids
            );
        }
    }

    println!("\nLISTENING");
    for socket in timed("ports", listening_sockets) {
        let holders: Vec<String> = socket
            .holders
            .iter()
            .map(|holder| format!("{} ({})", holder.name, holder.key.pid))
            .collect();
        println!(
            "  {:?} {}:{}  {:?}  {}",
            socket.transport,
            socket.address,
            socket.port,
            socket.exposure,
            holders.join(", ")
        );
    }
}
