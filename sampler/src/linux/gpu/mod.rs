//! GPU readers for Linux: NVML for NVIDIA, and the DRM drivers' fdinfo with amdgpu's sysfs
//! files for AMD, Intel and Arm GPUs. Each reader covers its own adapters. A machine where
//! none works lists no adapters, as Windows does without GPU counters.

mod drm;
mod fdinfo;
mod nvml;
mod pci;

use std::{collections::HashMap, fmt, time::Instant};

use allot_core::sampler::RawGpu;

const BYTES_PER_MB: u64 = 1024 * 1024;

fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / BYTES_PER_MB
}

/// Reads one kind of GPU source.
pub(crate) trait GpuReader: fmt::Debug {
    /// Reads the source's adapters and the GPU use of each process on them. Rates cover the
    /// time since the previous call, so the first call reads no load.
    fn read(&mut self, now: Instant) -> GpuReading;
}

/// GPU adapters and per-process GPU use.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct GpuReading {
    pub(crate) adapters: Vec<RawGpu>,
    /// Keyed by PID.
    pub(crate) processes: HashMap<u32, ProcessGpu>,
}

/// The GPU use of one process.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ProcessGpu {
    /// Load of the process's busiest engine, as a percentage of one adapter.
    pub(crate) gpu_pct: f32,
    /// Video memory, over every adapter.
    pub(crate) vram_mb: u64,
}

impl GpuReading {
    /// Adds the adapters and processes of `other`. A process on several adapters shows its
    /// busiest one, as on Windows, and the video memory of all of them.
    fn merge(&mut self, other: Self) {
        self.adapters.extend(other.adapters);
        for (pid, usage) in other.processes {
            let process = self.processes.entry(pid).or_default();
            process.gpu_pct = process.gpu_pct.max(usage.gpu_pct);
            process.vram_mb += usage.vram_mb;
        }
    }
}

/// Every GPU reader that works on the machine.
#[derive(Debug)]
pub(crate) struct Gpus {
    readers: Vec<Box<dyn GpuReader>>,
}

impl Gpus {
    /// The readers for the machine at `root`: NVML when the NVIDIA driver is installed, and
    /// DRM when a GPU's driver publishes fdinfo statistics.
    #[cfg(target_os = "linux")]
    pub(crate) fn new(root: &std::path::Path) -> Self {
        let mut readers: Vec<Box<dyn GpuReader>> = Vec::new();
        if let Some(nvml) = nvml::Nvml::new(root) {
            readers.push(Box::new(nvml));
        }
        if let Some(drm) = drm::Drm::new(root) {
            readers.push(Box::new(drm));
        }
        Self { readers }
    }

    /// Reads every source; empty when there is none.
    pub(crate) fn read(&mut self, now: Instant) -> GpuReading {
        let mut reading = GpuReading::default();
        for reader in &mut self.readers {
            reading.merge(reader.read(now));
        }
        reading
    }
}

#[cfg(test)]
mod tests {
    use allot_core::sampler::EngineLoad;
    use allot_shared::types::{Accuracy, GpuSource};

    use super::*;

    #[derive(Debug)]
    struct Fixed(&'static str, Vec<(u32, ProcessGpu)>);

    impl GpuReader for Fixed {
        fn read(&mut self, _now: Instant) -> GpuReading {
            GpuReading {
                adapters: vec![RawGpu {
                    name: self.0.into(),
                    luid: self.0.into(),
                    engines: EngineLoad::default(),
                    dedicated_used_mb: 0,
                    dedicated_total_mb: None,
                    shared_used_mb: 0,
                    unified_memory: false,
                    source: GpuSource::Nvml,
                    process_load: Accuracy::Exact,
                    process_memory: Accuracy::Exact,
                }],
                processes: self.1.iter().copied().collect(),
            }
        }
    }

    fn usage(gpu_pct: f32, vram_mb: u64) -> ProcessGpu {
        ProcessGpu { gpu_pct, vram_mb }
    }

    #[test]
    fn test_gpus_without_readers_read_nothing() {
        let mut gpus = Gpus {
            readers: Vec::new(),
        };
        assert_eq!(gpus.read(Instant::now()), GpuReading::default());
    }

    #[test]
    fn test_gpus_read_merges_adapters_and_processes() {
        let mut gpus = Gpus {
            readers: vec![
                Box::new(Fixed(
                    "nvidia",
                    vec![(1, usage(40.0, 300)), (2, usage(5.0, 0))],
                )),
                Box::new(Fixed(
                    "intel",
                    vec![(1, usage(70.0, 20)), (3, usage(1.0, 1))],
                )),
            ],
        };
        let reading = gpus.read(Instant::now());
        let names: Vec<&str> = reading
            .adapters
            .iter()
            .map(|gpu| gpu.luid.as_str())
            .collect();
        assert_eq!(names, ["nvidia", "intel"]);
        assert_eq!(reading.processes[&1], usage(70.0, 320));
        assert_eq!(reading.processes[&2], usage(5.0, 0));
        assert_eq!(reading.processes[&3], usage(1.0, 1));
    }
}
