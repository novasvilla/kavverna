//! Reading what the machine is doing: processor, memory, graphics and temperatures.

mod graphics;
mod memory;
mod nvidia;
mod processor;
mod thermal;

pub use graphics::{Gpu, GpuReading, GpuRole, GraphicsReading, SysfsCard, discover_sysfs_cards};
pub use memory::{
    CompressedSwap, MemoryPressure, MemoryReading, PressureLevel, discover_compressed_swap,
    parse_meminfo, parse_mm_stat, parse_pressure,
};
pub use nvidia::NvidiaCards;
pub use processor::{CpuTicks, ProcessorTicks, parse_stat};
pub use thermal::{Sensor, Thermometer, parse_label};

use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct Vitals {
    pub cpu_load: Option<f32>,
    pub core_loads: Vec<f32>,
    pub cpu_temperature: Option<f32>,
    pub memory: MemoryReading,
    pub pressure: MemoryPressure,
    pub compressed_swap: Vec<CompressedSwap>,
    pub graphics: GraphicsReading,
    pub taken_at: Option<Instant>,
}

/// Holds the previous processor reading, because load is a difference rather than a value.
pub struct Vitalsigns {
    previous: Option<ProcessorTicks>,
    thermometer: Thermometer,
    nvidia: Option<NvidiaCards>,
    amd: Vec<SysfsCard>,
}

impl Vitalsigns {
    pub fn open() -> Self {
        Self {
            previous: None,
            thermometer: Thermometer::discover(),
            nvidia: NvidiaCards::open(),
            amd: discover_sysfs_cards(),
        }
    }

    /// The first call cannot report processor load: there is nothing to compare against.
    pub fn sample(&mut self) -> Vitals {
        let ticks = std::fs::read_to_string("/proc/stat")
            .map(|contents| parse_stat(&contents))
            .unwrap_or_default();

        let (cpu_load, core_loads) = match self.previous.take() {
            Some(previous) => (
                ticks.total.load_since(previous.total),
                ticks
                    .cores
                    .iter()
                    .zip(previous.cores.iter())
                    .filter_map(|(now, before)| now.load_since(*before))
                    .collect(),
            ),
            None => (None, Vec::new()),
        };
        self.previous = Some(ticks);

        let mut cards = self.nvidia.as_ref().map(NvidiaCards::read).unwrap_or_default();
        cards.extend(self.amd.iter().map(|card| Gpu {
            role: GpuRole::Integrated,
            reading: card.read(),
        }));
        cards.sort_by_key(|card| card.role);

        Vitals {
            cpu_load,
            core_loads,
            cpu_temperature: self.thermometer.processor_celsius(),
            memory: std::fs::read_to_string("/proc/meminfo")
                .map(|contents| parse_meminfo(&contents))
                .unwrap_or_default(),
            pressure: std::fs::read_to_string("/proc/pressure/memory")
                .map(|contents| parse_pressure(&contents))
                .unwrap_or_default(),
            compressed_swap: discover_compressed_swap(),
            graphics: GraphicsReading { cards },
            taken_at: Some(Instant::now()),
        }
    }
}
