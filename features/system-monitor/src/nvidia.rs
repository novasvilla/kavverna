use crate::graphics::{Gpu, GpuReading, GpuRole};
use nvml_wrapper::Nvml;

/// Absent when no driver is installed, which is the normal case on a machine with only an
/// integrated card.
pub struct NvidiaCards {
    nvml: Nvml,
}

impl NvidiaCards {
    pub fn open() -> Option<Self> {
        match Nvml::init() {
            Ok(nvml) => Some(Self { nvml }),
            Err(err) => {
                tracing::info!(%err, "no NVIDIA driver, skipping those cards");
                None
            }
        }
    }

    pub fn read(&self) -> Vec<Gpu> {
        let count = self.nvml.device_count().unwrap_or(0);

        (0..count)
            .filter_map(|index| self.nvml.device_by_index(index).ok())
            .map(|device| {
                let memory = device.memory_info().ok();

                Gpu {
                    role: GpuRole::Discrete,
                    reading: GpuReading {
                        name: device.name().unwrap_or_else(|_| "NVIDIA".into()),
                        utilisation: device
                            .utilization_rates()
                            .ok()
                            .map(|rates| rates.gpu as f32 / 100.0),
                        temperature: device
                            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                            .ok()
                            .map(|celsius| celsius as f32),
                        // Reported in milliwatts.
                        power_watts: device
                            .power_usage()
                            .ok()
                            .map(|milliwatts| milliwatts as f32 / 1000.0),
                        memory_used: memory.as_ref().map(|memory| memory.used),
                        memory_total: memory.as_ref().map(|memory| memory.total),
                    },
                }
            })
            .collect()
    }
}
