//! Apple Silicon GPU use from IOKit: the accelerator's `PerformanceStatistics` for the
//! machine, and the `AppUsage` of each user client (one per process that opened the device)
//! for processes.
//!
//! GPU time is credited to a client when a command buffer completes, and long buffers are
//! credited only in part, with the rest charged to whoever else submits meanwhile. Process
//! load is therefore an estimate, and admission relies on the machine's load and the leases'
//! reservations instead. Clients report no memory, so process GPU memory is not available;
//! the memory a process allocates for the GPU is part of its footprint.

use std::{collections::HashMap, time::Instant};

use allot_core::sampler::{EngineLoad, RawGpu};
use allot_shared::types::{Accuracy, GpuSource};

use crate::macos::value::{Entry, Value};

const BYTES_PER_MB: u64 = 1024 * 1024;

/// The accelerator properties the reader uses.
pub(crate) const ACCELERATOR_KEYS: [&str; 2] = ["model", "PerformanceStatistics"];
/// The user client properties the reader uses.
pub(crate) const CLIENT_KEYS: [&str; 2] = ["IOUserClientCreator", "AppUsage"];

/// GPU adapters and the GPU load of each process, as a percentage of one adapter, keyed by
/// PID.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct GpuReading {
    pub(crate) adapters: Vec<RawGpu>,
    pub(crate) processes: HashMap<u32, f32>,
}

/// Remembers each client's GPU time at the previous read to turn it into load.
#[derive(Debug, Default)]
pub(crate) struct GpuTimes {
    /// GPU nanoseconds by client registry ID.
    previous: HashMap<u64, u64>,
    previous_at: Option<Instant>,
}

impl GpuTimes {
    /// The adapters among `accelerators` (read with their clients as children), and each
    /// process's load since the previous call. A client that appeared since then counts
    /// from zero; the first call only records a baseline.
    pub(crate) fn read(&mut self, accelerators: &[Entry], now: Instant) -> GpuReading {
        let adapters = accelerators.iter().filter_map(adapter).collect();
        let clients: Vec<Client> = accelerators
            .iter()
            .flat_map(|accelerator| &accelerator.children)
            .filter_map(client)
            .collect();
        let interval_ns = self
            .previous_at
            .map(|at| now.saturating_duration_since(at).as_nanos() as f64)
            .filter(|&ns| ns > 0.0);
        let processes = interval_ns
            .map(|interval_ns| self.loads(&clients, interval_ns))
            .unwrap_or_default();
        self.previous = clients
            .iter()
            .map(|client| (client.id, client.gpu_ns))
            .collect();
        self.previous_at = Some(now);
        GpuReading {
            adapters,
            processes,
        }
    }

    /// Each busy process's GPU time since the previous read, summed over its clients, as a
    /// share of `interval_ns`. Queues run in parallel, so the sum can exceed the interval;
    /// it is capped at one adapter.
    fn loads(&self, clients: &[Client], interval_ns: f64) -> HashMap<u32, f32> {
        let mut busy_ns: HashMap<u32, u64> = HashMap::new();
        for client in clients {
            let before = self.previous.get(&client.id).copied().unwrap_or_default();
            *busy_ns.entry(client.pid).or_default() += client.gpu_ns.saturating_sub(before);
        }
        busy_ns
            .into_iter()
            .filter(|&(_, ns)| ns > 0)
            .map(|(pid, ns)| (pid, (ns as f64 / interval_ns * 100.0).min(100.0) as f32))
            .collect()
    }
}

/// An accelerator as an adapter. `None` when it publishes no utilization, as paravirtual
/// GPUs in virtual machines may not.
fn adapter(accelerator: &Entry) -> Option<RawGpu> {
    let statistics = accelerator.properties.get("PerformanceStatistics")?;
    let device_pct = statistics.u64_at("Device Utilization %")?;
    let name = accelerator.properties.get("model").and_then(Value::as_str);
    Some(RawGpu {
        name: name.unwrap_or("Apple GPU").to_owned(),
        luid: format!("0x{:x}", accelerator.id),
        engines: EngineLoad {
            // Device utilization covers render, tiler, and compute work alike.
            three_d_pct: device_pct.min(100) as f32,
            ..EngineLoad::default()
        },
        // Memory allocated for the GPU, out of RAM; "In use system memory" does not grow
        // with allocations.
        dedicated_used_mb: statistics.u64_at("Alloc system memory").unwrap_or_default()
            / BYTES_PER_MB,
        dedicated_total_mb: None,
        shared_used_mb: 0,
        unified_memory: true,
        source: GpuSource::Iokit,
        process_load: Accuracy::Estimated,
        process_memory: Accuracy::Unavailable,
    })
}

/// One process's open GPU device, with the GPU time of all its command queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Client {
    id: u64,
    pid: u32,
    gpu_ns: u64,
}

/// A user client of the accelerator. A client that has not submitted work yet has no
/// `AppUsage` and counts with zero time.
fn client(entry: &Entry) -> Option<Client> {
    let creator = entry.properties.get("IOUserClientCreator")?.as_str()?;
    let gpu_ns = entry
        .properties
        .get("AppUsage")
        .map(|usage| {
            usage
                .items()
                .iter()
                .filter_map(|queue| queue.u64_at("accumulatedGPUTime"))
                .sum()
        })
        .unwrap_or_default();
    Some(Client {
        id: entry.id,
        pid: creator_pid(creator)?,
        gpu_ns,
    })
}

/// The PID in an `IOUserClientCreator` value such as `pid 812, WindowServer`.
fn creator_pid(creator: &str) -> Option<u32> {
    let rest = creator.strip_prefix("pid ")?;
    let pid = rest.split(',').next()?;
    pid.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::macos::tests::ioreg_fixture;

    #[test]
    fn test_creator_pid_reads_the_pid_and_rejects_other_text() {
        assert_eq!(creator_pid("pid 812, WindowServer"), Some(812));
        assert_eq!(creator_pid("pid 43994, gpuburn"), Some(43994));
        assert_eq!(creator_pid("pid 7"), Some(7));
        assert_eq!(creator_pid("kernel"), None);
        assert_eq!(creator_pid("pid x, y"), None);
    }

    #[test]
    fn test_read_lists_the_accelerator_as_a_unified_estimated_adapter() {
        let mut times = GpuTimes::default();
        let reading = times.read(&ioreg_fixture("m3max", "gpu-t0.json"), Instant::now());
        assert_eq!(reading.adapters.len(), 1);
        let gpu = &reading.adapters[0];
        assert_eq!(gpu.name, "Apple M3 Max");
        assert!(gpu.luid.starts_with("0x"), "{}", gpu.luid);
        assert!(gpu.unified_memory);
        assert_eq!(gpu.source, GpuSource::Iokit);
        assert_eq!(gpu.process_load, Accuracy::Estimated);
        assert_eq!(gpu.process_memory, Accuracy::Unavailable);
        assert_eq!(gpu.dedicated_total_mb, None);
        assert!(gpu.dedicated_used_mb > 0);
        assert!((0.0..=100.0).contains(&gpu.engines.three_d_pct));
        assert!(reading.processes.is_empty(), "the first read is a baseline");
    }

    #[test]
    fn test_read_turns_gpu_time_into_process_load_between_reads() {
        let start = Instant::now();
        let mut times = GpuTimes::default();
        times.read(&ioreg_fixture("m3max", "gpu-t0.json"), start);
        let after = ioreg_fixture("m3max", "gpu-t1.json");
        let elapsed = fixture_interval();
        let reading = times.read(&after, start + elapsed);

        let burner = burner_pid(&after);
        let load = reading.processes.get(&burner).copied().unwrap_or_default();
        assert!(
            load > 10.0,
            "the burner read {load}%: {:?}",
            reading.processes
        );
        assert!(
            reading
                .processes
                .values()
                .all(|&pct| (0.0..=100.0).contains(&pct))
        );
        assert!(reading.processes.values().all(|&pct| pct > 0.0));
    }

    #[test]
    fn test_read_counts_a_new_client_from_zero_and_caps_at_one_adapter() {
        let start = Instant::now();
        let mut times = GpuTimes::default();
        let mut before = ioreg_fixture("m3max", "gpu-t0.json");
        let burner = burner_pid(&before);
        before[0]
            .children
            .retain(|client| client_pid(client) != Some(burner));
        times.read(&before, start);
        let after = ioreg_fixture("m3max", "gpu-t1.json");
        let reading = times.read(&after, start + Duration::from_millis(1));
        // All of the burner's GPU time is new, far more than 1 ms of wall time.
        assert_eq!(reading.processes.get(&burner), Some(&100.0));
    }

    #[test]
    fn test_read_skips_an_accelerator_without_statistics() {
        let mut accelerators = ioreg_fixture("m3max", "gpu-t0.json");
        accelerators[0].properties = Value::Dict(Vec::new());
        let reading = GpuTimes::default().read(&accelerators, Instant::now());
        assert!(reading.adapters.is_empty());
    }

    fn client_pid(entry: &Entry) -> Option<u32> {
        client(entry).map(|client| client.pid)
    }

    /// The PID of the fixture's `gpuburn` client, the Metal load running during the capture.
    fn burner_pid(accelerators: &[Entry]) -> u32 {
        accelerators[0]
            .children
            .iter()
            .find(|entry| {
                let creator = entry.properties.get("IOUserClientCreator");
                creator
                    .and_then(Value::as_str)
                    .is_some_and(|c| c.ends_with("gpuburn"))
            })
            .and_then(client_pid)
            .expect("the fixture has a gpuburn client")
    }

    /// Wall time between `gpu-t0.json` and `gpu-t1.json`, recorded by the capture.
    fn fixture_interval() -> Duration {
        let text = std::fs::read_to_string(
            crate::macos::tests::fixture("m3max").join("gpu-interval-ms.txt"),
        )
        .unwrap();
        Duration::from_millis(text.trim().parse().unwrap())
    }
}
