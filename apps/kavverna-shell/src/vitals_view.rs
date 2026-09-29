use crate::vitals_state;
use cxx_qt::Threading;
use cxx_qt_lib::{QList, QString, QStringList};
use std::sync::Mutex;
use system_monitor::Vitals;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qlist.h");
        type QList_f32 = cxx_qt_lib::QList<f32>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(f32, cpu_load)]
        #[qproperty(QString, cpu_load_text)]
        #[qproperty(QString, cpu_temperature_text)]
        #[qproperty(QString, processor_name)]
        #[qproperty(QString, processor_detail)]
        #[qproperty(QList_f32, core_loads)]
        #[qproperty(QList_f32, cpu_history)]
        #[qproperty(f32, memory_used)]
        #[qproperty(QString, memory_text)]
        #[qproperty(QList_f32, memory_history)]
        #[qproperty(f32, memory_apps)]
        #[qproperty(QString, memory_apps_text)]
        #[qproperty(QString, pressure_text)]
        #[qproperty(QString, swap_text)]
        #[qproperty(QStringList, card_names)]
        #[qproperty(i32, chosen_card)]
        #[qproperty(QString, gpu_usage_text)]
        #[qproperty(f32, gpu_usage)]
        #[qproperty(QList_f32, gpu_history)]
        #[qproperty(QString, gpu_temperature_text)]
        #[qproperty(QString, gpu_power_text)]
        #[qproperty(f32, vram_used)]
        #[qproperty(QString, vram_text)]
        #[qproperty(QStringList, network_names)]
        #[qproperty(QStringList, network_received)]
        #[qproperty(QStringList, network_sent)]
        #[qproperty(QStringList, network_totals)]
        #[qproperty(QString, network_status)]
        #[qproperty(QStringList, disk_names)]
        #[qproperty(QStringList, disk_spaces)]
        #[qproperty(QStringList, disk_rates)]
        #[qproperty(QString, disk_status)]
        type VitalsView = super::VitalsViewRust;
    }

    impl cxx_qt::Threading for VitalsView {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn attach(self: Pin<&mut VitalsView>);
        #[qinvokable]
        fn choose_card(self: Pin<&mut VitalsView>, index: i32);
    }
}

use core::pin::Pin;

static VIEW: Mutex<Option<cxx_qt::CxxQtThread<qobject::VitalsView>>> = Mutex::new(None);

#[derive(Default)]
pub struct VitalsViewRust {
    cpu_load: f32,
    cpu_load_text: QString,
    cpu_temperature_text: QString,
    processor_name: QString,
    processor_detail: QString,
    core_loads: QList<f32>,
    cpu_history: QList<f32>,
    memory_used: f32,
    memory_text: QString,
    memory_history: QList<f32>,
    memory_apps: f32,
    memory_apps_text: QString,
    pressure_text: QString,
    swap_text: QString,
    card_names: QStringList,
    chosen_card: i32,
    gpu_usage_text: QString,
    gpu_usage: f32,
    gpu_history: QList<f32>,
    gpu_temperature_text: QString,
    gpu_power_text: QString,
    vram_used: f32,
    vram_text: QString,
    network_names: QStringList,
    network_received: QStringList,
    network_sent: QStringList,
    network_totals: QStringList,
    network_status: QString,
    disk_names: QStringList,
    disk_spaces: QStringList,
    disk_rates: QStringList,
    disk_status: QString,
}

fn as_list(series: &std::collections::VecDeque<f32>) -> QList<f32> {
    let mut list = QList::<f32>::default();
    for reading in series {
        list.append(*reading);
    }
    list
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn bytes_label(bytes: f64) -> String {
    if bytes >= 1024.0_f64.powi(4) {
        format!("{:.1} TiB", bytes / 1024.0_f64.powi(4))
    } else if bytes >= 1024.0_f64.powi(3) {
        format!("{:.1} GiB", bytes / 1024.0_f64.powi(3))
    } else if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MiB", bytes / (1024.0 * 1024.0))
    } else if bytes >= 1024.0 {
        format!("{:.1} KiB", bytes / 1024.0)
    } else {
        format!("{bytes:.0} B")
    }
}

impl qobject::VitalsView {
    fn attach(mut self: Pin<&mut Self>) {
        let thread = self.as_mut().qt_thread();
        if let Ok(mut view) = VIEW.lock() {
            *view = Some(thread);
        }
        self.apply(vitals_state::get());
    }

    /// The discrete card is what the readings default to, but the integrated one stays
    /// selectable rather than hidden.
    fn choose_card(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().set_chosen_card(index);
        self.apply(vitals_state::get());
    }

    fn apply(mut self: Pin<&mut Self>, vitals: Vitals) {
        let load = vitals.cpu_load.unwrap_or(0.0);
        self.as_mut().set_cpu_load(load);
        self.as_mut().set_cpu_load_text(QString::from(&match vitals.cpu_load {
            Some(load) => format!("{:.0}%", load * 100.0),
            None => "--".into(),
        }));
        self.as_mut().set_cpu_temperature_text(QString::from(&match vitals.cpu_temperature {
            Some(celsius) => format!("{celsius:.0} C"),
            None => "--".into(),
        }));

        let processor = vitals_state::processor();
        self.as_mut().set_processor_name(QString::from(&processor.name));
        // Cores and threads say the same thing on a chip without simultaneous threading, and
        // saying it twice reads as a mistake.
        let mut detail = match (processor.cores, processor.threads) {
            (0, 0) => String::new(),
            (cores, threads) if cores == threads => format!("{cores} cores"),
            (cores, threads) => format!("{cores} cores, {threads} threads"),
        };
        if let Some(ghz) = vitals.cpu_speed {
            if !detail.is_empty() {
                detail.push_str("  ·  ");
            }
            detail.push_str(&format!("{ghz:.2} GHz"));
        }
        self.as_mut().set_processor_detail(QString::from(&detail));

        let mut cores = QList::<f32>::default();
        for load in &vitals.core_loads {
            cores.append(*load);
        }
        self.as_mut().set_core_loads(cores);

        let past = vitals_state::history();
        self.as_mut().set_cpu_history(as_list(&past.cpu));
        self.as_mut().set_memory_history(as_list(&past.memory));

        let memory = vitals.memory;
        self.as_mut().set_memory_used(memory.used_fraction());
        self.as_mut().set_memory_text(QString::from(&format!(
            "{:.1} of {:.1} GiB",
            gib(memory.in_use),
            gib(memory.total)
        )));
        self.as_mut().set_memory_apps(memory.apps_fraction());
        self.as_mut()
            .set_memory_apps_text(QString::from(&format!("{:.1} GiB", gib(memory.held_by_apps))));

        self.as_mut().set_pressure_text(QString::from(&format!(
            "{:?}  ·  {:.2}% stalled",
            vitals.pressure.level(),
            vitals.pressure.full_ten_seconds
        )));

        // Swap here is compressed RAM, so the figure that matters is what it costs, not what
        // applications think they swapped.
        self.as_mut().set_swap_text(QString::from(&match vitals.compressed_swap.first() {
            Some(swap) => format!(
                "{:.1} GiB stored, {:.1} GiB of RAM ({:.1}x)",
                gib(swap.stored),
                gib(swap.ram_cost),
                swap.ratio()
            ),
            None => format!("{:.1} GiB", gib(memory.swap_used)),
        }));

        let mut names = QStringList::default();
        for name in vitals.graphics.names() {
            names.append(QString::from(&name));
        }
        self.as_mut().set_card_names(names);

        let index = usize::try_from(*self.chosen_card()).unwrap_or(0);
        let card = vitals.graphics.cards.get(index).or_else(|| vitals.graphics.preferred());

        let reading = card.map(|card| &card.reading);
        // Indexed the same way the cards are, so switching cards shows that card's own past
        // rather than a line spliced from both.
        self.as_mut().set_gpu_history(past.cards.get(index).map(as_list).unwrap_or_default());
        self.as_mut().set_gpu_usage(reading.and_then(|r| r.utilisation).unwrap_or(0.0));
        self.as_mut().set_gpu_usage_text(QString::from(
            &reading
                .and_then(|r| r.utilisation)
                .map_or("--".into(), |value| format!("{:.0}%", value * 100.0)),
        ));
        self.as_mut().set_gpu_temperature_text(QString::from(
            &reading
                .and_then(|r| r.temperature)
                .map_or("--".into(), |value| format!("{value:.0} C")),
        ));
        self.as_mut().set_gpu_power_text(QString::from(
            &reading
                .and_then(|r| r.power_watts)
                .map_or("--".into(), |value| format!("{value:.1} W")),
        ));
        self.as_mut().set_vram_used(reading.and_then(|r| r.memory_fraction()).unwrap_or(0.0));
        self.as_mut().set_vram_text(QString::from(
            &reading
                .and_then(|r| r.memory_used.zip(r.memory_total))
                .map_or("--".into(), |(used, total)| {
                    format!("{:.0} of {:.0} MiB", mib(used), mib(total))
                }),
        ));

        let mut network_names = QStringList::default();
        let mut network_received = QStringList::default();
        let mut network_sent = QStringList::default();
        let mut network_totals = QStringList::default();
        let status = match vitals.network {
            None => "Network counters are unavailable.",
            Some(ref interfaces) if interfaces.is_empty() => "No network interfaces found.",
            Some(ref interfaces) => {
                for interface in interfaces {
                    network_names.append(QString::from(&interface.interface));
                    network_received.append(QString::from(
                        &interface
                            .received_per_second
                            .map_or("—".into(), |rate| format!("↓ {}/s", bytes_label(rate))),
                    ));
                    network_sent.append(QString::from(
                        &interface
                            .sent_per_second
                            .map_or("—".into(), |rate| format!("↑ {}/s", bytes_label(rate))),
                    ));
                    network_totals.append(QString::from(&format!(
                        "This session: ↓ {}  ↑ {}",
                        bytes_label(interface.received_this_session as f64),
                        bytes_label(interface.sent_this_session as f64)
                    )));
                }
                ""
            }
        };
        self.as_mut().set_network_names(network_names);
        self.as_mut().set_network_received(network_received);
        self.as_mut().set_network_sent(network_sent);
        self.as_mut().set_network_totals(network_totals);
        self.as_mut().set_network_status(QString::from(status));

        let mut disk_names = QStringList::default();
        let mut disk_spaces = QStringList::default();
        let mut disk_rates = QStringList::default();
        let disk_status = match vitals.disks {
            None => "Mounted volumes could not be read.",
            Some(ref disks) if disks.is_empty() => "No local volumes found.",
            Some(ref disks) => {
                for disk in disks {
                    disk_names.append(QString::from(&disk.name));
                    disk_spaces.append(QString::from(&match (disk.available, disk.total) {
                        (Some(available), Some(total)) => format!(
                            "{} free of {}",
                            bytes_label(available as f64),
                            bytes_label(total as f64)
                        ),
                        _ => "Capacity unavailable".into(),
                    }));
                    let rate = |value: Option<f64>| {
                        value.map_or("—".into(), |value| format!("{}/s", bytes_label(value)))
                    };
                    disk_rates.append(QString::from(&format!(
                        "Read {}  ·  Write {}",
                        rate(disk.read_per_second),
                        rate(disk.written_per_second)
                    )));
                }
                ""
            }
        };
        self.as_mut().set_disk_names(disk_names);
        self.as_mut().set_disk_spaces(disk_spaces);
        self.as_mut().set_disk_rates(disk_rates);
        self.as_mut().set_disk_status(QString::from(disk_status));
    }
}

pub fn publish() {
    let Ok(view) = VIEW.lock() else {
        return;
    };
    if let Some(thread) = view.as_ref() {
        let _ = thread.queue(|view| view.apply(vitals_state::get()));
    }
}
