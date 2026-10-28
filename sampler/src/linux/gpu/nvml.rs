//! NVIDIA GPUs through NVML, which is loaded at run time: without the driver there is no
//! library and no reader.
//!
//! The reader works on [`NvmlLibrary`], so tests run it on a fake on every OS.

use std::{
    collections::HashMap,
    fmt,
    path::Path,
    time::{Duration, Instant},
};

use allot_core::sampler::{EngineLoad, RawGpu};
use allot_shared::types::{Accuracy, GpuSource};

use crate::linux::gpu::{GpuReader, GpuReading, ProcessGpu, bytes_to_mb, pci};

/// How long a process keeps its last utilization sample. NVML samples each busy process
/// about once a second, so a sample can be missing from a one-second window.
const SAMPLE_HOLD: Duration = Duration::from_secs(2);

/// What the reader needs from NVML.
pub(crate) trait NvmlLibrary: fmt::Debug {
    /// The number of devices.
    fn count(&self) -> u32;

    /// Device `index`, with utilization samples newer than `last_seen_us`, or `None` when it
    /// cannot be read.
    fn read_device(&self, index: u32, last_seen_us: Option<u64>) -> Option<DeviceReading>;
}

/// One reading of a device.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DeviceReading {
    pub(crate) name: String,
    /// PCI address as NVML spells it, for example `00000000:01:00.0`.
    pub(crate) bus_id: String,
    /// Share of the last sample period with a kernel running (3D or compute).
    pub(crate) gpu_pct: u32,
    pub(crate) encoder_pct: u32,
    pub(crate) decoder_pct: u32,
    pub(crate) memory_used_bytes: u64,
    pub(crate) memory_total_bytes: u64,
    /// Processes with a compute or graphics context on the device; one may be listed twice.
    pub(crate) processes: Vec<DeviceProcess>,
    /// Per-process utilization since the requested timestamp.
    pub(crate) samples: Vec<UtilizationSample>,
}

/// A process with a context on a device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DeviceProcess {
    pub(crate) pid: u32,
    /// Video memory the process uses; WSL2's driver does not report it.
    pub(crate) memory_bytes: Option<u64>,
}

/// One per-process utilization sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UtilizationSample {
    pub(crate) pid: u32,
    /// CPU timestamp of the sample, in microseconds.
    pub(crate) timestamp_us: u64,
    pub(crate) sm_pct: u32,
    pub(crate) encoder_pct: u32,
    pub(crate) decoder_pct: u32,
}

impl UtilizationSample {
    /// Like Task Manager: the busiest engine.
    fn busiest(&self) -> f32 {
        self.sm_pct
            .max(self.encoder_pct)
            .max(self.decoder_pct)
            .min(100) as f32
    }
}

/// Per-device state between readings.
#[derive(Debug, Default)]
struct DeviceState {
    last_seen_us: Option<u64>,
    /// Each process's latest load and when it was read.
    recent: HashMap<u32, (f32, Instant)>,
}

/// Reads every NVIDIA device.
#[derive(Debug)]
pub(crate) struct Nvml<L> {
    library: L,
    /// Under WSL2 NVML sees the Windows GPU: machine numbers are right, but samples name
    /// Windows PIDs and process memory is not reported, so nothing is read per process.
    wsl: bool,
    devices: Vec<DeviceState>,
}

impl<L: NvmlLibrary> Nvml<L> {
    /// The reader over `library`, or `None` when it lists no device.
    pub(crate) fn with_library(library: L, wsl: bool) -> Option<Self> {
        let count = library.count();
        (count > 0).then(|| Self {
            library,
            wsl,
            devices: (0..count).map(|_| DeviceState::default()).collect(),
        })
    }

    fn per_process_accuracy(&self) -> Accuracy {
        if self.wsl {
            Accuracy::Unavailable
        } else {
            Accuracy::Exact
        }
    }
}

#[cfg(target_os = "linux")]
impl Nvml<nvml_wrapper::Nvml> {
    /// Loads NVML, or `None` when the driver is missing or lists no device.
    pub(crate) fn new(root: &Path) -> Option<Self> {
        let library = nvml_wrapper::Nvml::init().ok()?;
        Self::with_library(library, is_wsl(root))
    }
}

/// Whether `root` is a WSL2 machine, by its GPU library folder or its kernel's name.
pub(crate) fn is_wsl(root: &Path) -> bool {
    root.join("usr/lib/wsl/lib").is_dir()
        || std::fs::read_to_string(root.join("proc/sys/kernel/osrelease"))
            .is_ok_and(|release| release.to_ascii_lowercase().contains("microsoft"))
}

impl<L: NvmlLibrary> GpuReader for Nvml<L> {
    fn read(&mut self, now: Instant) -> GpuReading {
        let mut reading = GpuReading::default();
        let accuracy = self.per_process_accuracy();
        for (index, state) in (0_u32..).zip(self.devices.iter_mut()) {
            let Some(device) = self.library.read_device(index, state.last_seen_us) else {
                continue;
            };
            if !self.wsl {
                record_samples(state, &device.samples, now);
                for (&pid, &(pct, _)) in &state.recent {
                    let process = reading.processes.entry(pid).or_default();
                    process.gpu_pct = process.gpu_pct.max(pct);
                }
                let mut memory: HashMap<u32, u64> = HashMap::new();
                for process in &device.processes {
                    let bytes = memory.entry(process.pid).or_default();
                    // A process with compute and graphics contexts is listed twice.
                    *bytes = (*bytes).max(process.memory_bytes.unwrap_or(0));
                }
                for (pid, bytes) in memory {
                    reading.processes.entry(pid).or_default().vram_mb += bytes_to_mb(bytes);
                }
            }
            reading.adapters.push(RawGpu {
                luid: pci::normalize_address(&device.bus_id).unwrap_or(device.bus_id),
                name: device.name,
                engines: EngineLoad {
                    three_d_pct: device.gpu_pct.min(100) as f32,
                    video_pct: device.encoder_pct.max(device.decoder_pct).min(100) as f32,
                    ..EngineLoad::default()
                },
                dedicated_used_mb: bytes_to_mb(device.memory_used_bytes),
                dedicated_total_mb: (device.memory_total_bytes > 0)
                    .then(|| bytes_to_mb(device.memory_total_bytes)),
                shared_used_mb: 0,
                unified_memory: false,
                source: GpuSource::Nvml,
                process_load: accuracy,
                process_memory: accuracy,
            });
        }
        reading
            .processes
            .retain(|_, usage| *usage != ProcessGpu::default());
        reading
    }
}

/// Folds new samples into `state`: each process's load is the mean of its new samples, and
/// a process without one for [`SAMPLE_HOLD`] reads idle.
fn record_samples(state: &mut DeviceState, samples: &[UtilizationSample], now: Instant) {
    let mut fresh: HashMap<u32, (f32, u32)> = HashMap::new();
    for sample in samples {
        let (sum, count) = fresh.entry(sample.pid).or_default();
        *sum += sample.busiest();
        *count += 1;
        state.last_seen_us = state.last_seen_us.max(Some(sample.timestamp_us));
    }
    for (pid, (sum, count)) in fresh {
        state.recent.insert(pid, (sum / count as f32, now));
    }
    state
        .recent
        .retain(|_, (_, read_at)| now.saturating_duration_since(*read_at) <= SAMPLE_HOLD);
}

#[cfg(target_os = "linux")]
impl NvmlLibrary for nvml_wrapper::Nvml {
    fn count(&self) -> u32 {
        self.device_count().unwrap_or(0)
    }

    fn read_device(&self, index: u32, last_seen_us: Option<u64>) -> Option<DeviceReading> {
        use nvml_wrapper::enums::device::UsedGpuMemory;

        let device = self.device_by_index(index).ok()?;
        let memory = device.memory_info().ok();
        let mut processes = Vec::new();
        let lists = [
            device.running_compute_processes(),
            device.running_graphics_processes(),
        ];
        for process in lists.into_iter().flatten().flatten() {
            processes.push(DeviceProcess {
                pid: process.pid,
                memory_bytes: match process.used_gpu_memory {
                    UsedGpuMemory::Used(bytes) => Some(bytes),
                    UsedGpuMemory::Unavailable => None,
                },
            });
        }
        // NotFound means no sample is newer than `last_seen_us`.
        let samples = device
            .process_utilization_stats(last_seen_us)
            .unwrap_or_default()
            .into_iter()
            .map(|sample| UtilizationSample {
                pid: sample.pid,
                timestamp_us: sample.timestamp,
                sm_pct: sample.sm_util,
                encoder_pct: sample.enc_util,
                decoder_pct: sample.dec_util,
            })
            .collect();
        Some(DeviceReading {
            name: device.name().ok()?,
            bus_id: device.pci_info().ok()?.bus_id,
            gpu_pct: device.utilization_rates().map_or(0, |rates| rates.gpu),
            encoder_pct: device
                .encoder_utilization()
                .map_or(0, |info| info.utilization),
            decoder_pct: device
                .decoder_utilization()
                .map_or(0, |info| info.utilization),
            memory_used_bytes: memory.as_ref().map_or(0, |memory| memory.used),
            memory_total_bytes: memory.as_ref().map_or(0, |memory| memory.total),
            processes,
            samples,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::linux::tests::fixture;

    const MIB: u64 = 1024 * 1024;

    /// Stands in for NVML: returns the queued readings in order and records the timestamps
    /// it was asked for.
    #[derive(Debug, Default)]
    struct FakeLibrary {
        devices: u32,
        readings: RefCell<Vec<DeviceReading>>,
        asked_since: RefCell<Vec<Option<u64>>>,
    }

    impl NvmlLibrary for FakeLibrary {
        fn count(&self) -> u32 {
            self.devices
        }

        fn read_device(&self, _index: u32, last_seen_us: Option<u64>) -> Option<DeviceReading> {
            self.asked_since.borrow_mut().push(last_seen_us);
            let mut readings = self.readings.borrow_mut();
            (!readings.is_empty()).then(|| readings.remove(0))
        }
    }

    fn gtx_970(processes: Vec<DeviceProcess>, samples: Vec<UtilizationSample>) -> DeviceReading {
        DeviceReading {
            name: "NVIDIA GeForce GTX 970".into(),
            bus_id: "00000000:01:00.0".into(),
            gpu_pct: 57,
            encoder_pct: 0,
            decoder_pct: 12,
            memory_used_bytes: 2304 * MIB,
            memory_total_bytes: 4096 * MIB,
            processes,
            samples,
        }
    }

    fn process(pid: u32, mib: Option<u64>) -> DeviceProcess {
        DeviceProcess {
            pid,
            memory_bytes: mib.map(|mib| mib * MIB),
        }
    }

    fn sample(pid: u32, timestamp_us: u64, sm_pct: u32) -> UtilizationSample {
        UtilizationSample {
            pid,
            timestamp_us,
            sm_pct,
            encoder_pct: 0,
            decoder_pct: 0,
        }
    }

    fn reader(readings: Vec<DeviceReading>, wsl: bool) -> Nvml<FakeLibrary> {
        let library = FakeLibrary {
            devices: 1,
            readings: RefCell::new(readings),
            ..FakeLibrary::default()
        };
        Nvml::with_library(library, wsl).unwrap()
    }

    #[test]
    fn test_nvml_with_no_device_is_no_reader() {
        assert!(Nvml::with_library(FakeLibrary::default(), false).is_none());
    }

    #[test]
    fn test_nvml_read_reports_the_adapter() {
        let mut nvml = reader(vec![gtx_970(Vec::new(), Vec::new())], false);
        let reading = nvml.read(Instant::now());
        assert_eq!(
            reading.adapters,
            [RawGpu {
                name: "NVIDIA GeForce GTX 970".into(),
                luid: "0000:01:00.0".into(),
                engines: EngineLoad {
                    three_d_pct: 57.0,
                    video_pct: 12.0,
                    ..EngineLoad::default()
                },
                dedicated_used_mb: 2304,
                dedicated_total_mb: Some(4096),
                shared_used_mb: 0,
                unified_memory: false,
                source: GpuSource::Nvml,
                process_load: Accuracy::Exact,
                process_memory: Accuracy::Exact,
            }]
        );
        assert!(reading.processes.is_empty());
    }

    #[test]
    fn test_nvml_read_takes_each_process_once_with_its_mean_load() {
        let mut nvml = reader(
            vec![gtx_970(
                vec![
                    process(10, Some(300)),
                    process(10, Some(300)),
                    process(11, Some(11)),
                ],
                vec![
                    sample(10, 1_000, 40),
                    sample(10, 2_000, 60),
                    sample(12, 1_500, 5),
                ],
            )],
            false,
        );
        let reading = nvml.read(Instant::now());
        assert_eq!(
            reading.processes[&10],
            ProcessGpu {
                gpu_pct: 50.0,
                vram_mb: 300
            }
        );
        assert_eq!(reading.processes[&11].vram_mb, 11);
        assert_eq!(reading.processes[&12].gpu_pct, 5.0);
    }

    #[test]
    fn test_nvml_read_asks_for_samples_since_the_newest_seen() {
        let mut nvml = reader(
            vec![
                gtx_970(
                    Vec::new(),
                    vec![sample(10, 1_000, 40), sample(10, 3_000, 60)],
                ),
                gtx_970(Vec::new(), Vec::new()),
            ],
            false,
        );
        let start = Instant::now();
        nvml.read(start);
        nvml.read(start + Duration::from_secs(1));
        assert_eq!(*nvml.library.asked_since.borrow(), [None, Some(3_000)]);
    }

    #[test]
    fn test_nvml_read_holds_a_sample_for_a_while_then_reads_idle() {
        let mut nvml = reader(
            vec![
                gtx_970(Vec::new(), vec![sample(10, 1_000, 80)]),
                gtx_970(Vec::new(), Vec::new()),
                gtx_970(Vec::new(), Vec::new()),
            ],
            false,
        );
        let start = Instant::now();
        assert_eq!(nvml.read(start).processes[&10].gpu_pct, 80.0);
        let held = nvml.read(start + Duration::from_secs(1));
        assert_eq!(held.processes[&10].gpu_pct, 80.0);
        let later = nvml.read(start + Duration::from_secs(3));
        assert!(!later.processes.contains_key(&10));
    }

    #[test]
    fn test_nvml_read_under_wsl_reports_the_machine_only() {
        let mut nvml = reader(
            vec![gtx_970(vec![process(10, None)], vec![sample(4, 1_000, 94)])],
            true,
        );
        let reading = nvml.read(Instant::now());
        assert!(reading.processes.is_empty());
        let adapter = &reading.adapters[0];
        assert_eq!(adapter.engines.three_d_pct, 57.0);
        assert_eq!(adapter.process_load, Accuracy::Unavailable);
        assert_eq!(adapter.process_memory, Accuracy::Unavailable);
    }

    #[test]
    fn test_nvml_read_skips_a_device_that_cannot_be_read() {
        let mut nvml = reader(Vec::new(), false);
        assert_eq!(nvml.read(Instant::now()), GpuReading::default());
    }

    #[test]
    fn test_is_wsl_by_library_folder_or_kernel_release() {
        assert!(is_wsl(&fixture("wsl")));
        assert!(!is_wsl(&fixture("desktop")));
    }
}
