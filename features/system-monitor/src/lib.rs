//! Reading what the machine is and what it is doing: the processor it names itself after, its
//! load and speed, memory, graphics and temperatures.

mod disk;
mod graphics;
mod memory;
mod network;
mod nvidia;
mod processor;
mod thermal;

pub use disk::{DiskReading, DiskSampler, Volume};
pub use graphics::{Gpu, GpuReading, GpuRole, GraphicsReading, SysfsCard, discover_sysfs_cards};
pub use memory::{
    CompressedSwap, MemoryPressure, MemoryReading, PressureLevel, discover_compressed_swap,
    parse_meminfo, parse_mm_stat, parse_pressure,
};
pub use network::{NetworkReading, NetworkSampler};
pub use nvidia::NvidiaCards;
pub use processor::{CpuTicks, Processor, ProcessorTicks, parse_stat};
pub use thermal::{Sensor, Thermometer, parse_label};

use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct Vitals {
    pub cpu_load: Option<f32>,
    pub core_loads: Vec<f32>,
    pub cpu_temperature: Option<f32>,
    /// The fastest core at this moment, in GHz.
    pub cpu_speed: Option<f32>,
    pub memory: MemoryReading,
    pub pressure: MemoryPressure,
    pub compressed_swap: Vec<CompressedSwap>,
    pub graphics: GraphicsReading,
    pub network: Option<Vec<NetworkReading>>,
    pub disks: Option<Vec<DiskReading>>,
    pub taken_at: Option<Instant>,
}

/// Holds the previous processor reading, because load is a difference rather than a value.
pub struct Vitalsigns {
    system_enabled: bool,
    network_enabled: bool,
    previous: Option<ProcessorTicks>,
    processor: Processor,
    thermometer: Thermometer,
    nvidia: Option<NvidiaCards>,
    amd: Vec<SysfsCard>,
    network: NetworkSampler,
    disk: DiskSampler,
}

impl Vitalsigns {
    pub fn open() -> Self {
        Self::for_features(true, true)
    }

    pub fn for_features(system_enabled: bool, network_enabled: bool) -> Self {
        Self {
            system_enabled,
            network_enabled,
            previous: None,
            processor: if system_enabled { Processor::discover() } else { Processor::default() },
            thermometer: if system_enabled {
                Thermometer::discover()
            } else {
                Thermometer::default()
            },
            nvidia: if system_enabled { NvidiaCards::open() } else { None },
            amd: if system_enabled { discover_sysfs_cards() } else { Vec::new() },
            network: NetworkSampler::default(),
            disk: DiskSampler::default(),
        }
    }

    /// What the machine calls its processor, read once when the sampler opened.
    pub fn processor(&self) -> &Processor {
        &self.processor
    }

    /// The first call cannot report processor load: there is nothing to compare against.
    pub fn sample(&mut self) -> Vitals {
        let now = Instant::now();
        if !self.system_enabled {
            return Vitals {
                network: self.read_network(now),
                taken_at: Some(now),
                ..Default::default()
            };
        }
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
        cards.extend(
            self.amd.iter().map(|card| Gpu { role: GpuRole::Integrated, reading: card.read() }),
        );
        cards.sort_by_key(|card| card.role);

        Vitals {
            cpu_load,
            core_loads,
            cpu_temperature: self.thermometer.processor_celsius(),
            cpu_speed: self.processor.speed_ghz(),
            memory: std::fs::read_to_string("/proc/meminfo")
                .map(|contents| parse_meminfo(&contents))
                .unwrap_or_default(),
            pressure: std::fs::read_to_string("/proc/pressure/memory")
                .map(|contents| parse_pressure(&contents))
                .unwrap_or_default(),
            compressed_swap: discover_compressed_swap(),
            graphics: GraphicsReading { cards },
            network: self.read_network(now),
            disks: self.disk.sample(),
            taken_at: Some(now),
        }
    }

    fn read_network(&mut self, now: Instant) -> Option<Vec<NetworkReading>> {
        if !self.network_enabled {
            return None;
        }
        std::fs::read_to_string("/proc/net/dev")
            .ok()
            .map(|contents| self.network.sample(&contents, now))
    }
}
