//! Interim macOS implementation of [`allot_core::sampler::Sampler`], from sysinfo alone: CPU,
//! memory, and processes, with no GPU or drive data.

use std::{
    collections::HashMap,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use allot_core::sampler::{RawProcess, RawSample, Sampler};
use allot_shared::{
    Result,
    types::{MemStats, ResourceUsage},
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::cpu::{CpuTimes, ProcessTime};

const BYTES_PER_MB: u64 = 1024 * 1024;

/// Reads the local machine.
#[derive(Debug)]
pub struct SystemSampler {
    system: System,
    cpu_times: CpuTimes,
    /// Logical cores per PID since the previous refresh.
    process_cores: HashMap<u32, f32>,
}

impl SystemSampler {
    /// Takes a first reading, so the next [`Sampler::sample`] has a baseline for its rates.
    ///
    /// # Errors
    ///
    /// Never fails; the `Result` matches the Windows sampler, whose counters can fail to open.
    pub fn new() -> Result<Self> {
        let mut sampler = Self {
            system: System::new(),
            cpu_times: CpuTimes::default(),
            process_cores: HashMap::new(),
        };
        sampler.refresh();
        Ok(sampler)
    }

    fn refresh(&mut self) {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_exe(UpdateKind::OnlyIfNotSet),
        );
        let times: Vec<ProcessTime> = self
            .system
            .processes()
            .values()
            .map(|process| ProcessTime {
                pid: process.pid().as_u32(),
                start_time: process.start_time(),
                cpu_ms: process.accumulated_cpu_time(),
            })
            .collect();
        let logical_cores = self.system.cpus().len().max(1) as f32;
        self.process_cores = self.cpu_times.update(Instant::now(), &times, logical_cores);
    }
}

impl Sampler for SystemSampler {
    fn sample(&mut self) -> Result<RawSample> {
        self.refresh();
        let processes = self
            .system
            .processes()
            .values()
            .map(|process| {
                let pid = process.pid().as_u32();
                RawProcess {
                    pid,
                    parent_pid: process.parent().map(Pid::as_u32),
                    name: process.name().to_string_lossy().into_owned(),
                    exe: process.exe().map(|path| path.display().to_string()),
                    start_time: process.start_time(),
                    usage: ResourceUsage {
                        cores: self.process_cores.get(&pid).copied().unwrap_or_default(),
                        ram_mb: process.memory() / BYTES_PER_MB,
                        ..ResourceUsage::default()
                    },
                }
            })
            .collect();
        let taken_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        let total_mb = self.system.total_memory() / BYTES_PER_MB;
        let used_mb = self.system.used_memory() / BYTES_PER_MB;
        // macOS has no commit charge; memory in use plus swap in use is the nearest equivalent.
        Ok(RawSample {
            taken_at_ms,
            cpu_total_pct: self.system.global_cpu_usage(),
            per_core_pct: self
                .system
                .cpus()
                .iter()
                .map(|cpu| cpu.cpu_usage())
                .collect(),
            memory: MemStats {
                total_mb,
                used_mb,
                available_mb: self.system.available_memory() / BYTES_PER_MB,
                commit_used_mb: used_mb + self.system.used_swap() / BYTES_PER_MB,
                commit_limit_mb: total_mb + self.system.total_swap() / BYTES_PER_MB,
            },
            processes,
            gpus: Vec::new(),
            drives: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{process, thread};

    use super::*;

    #[test]
    fn test_system_sampler_sample_reads_own_process_and_memory() {
        let mut sampler = SystemSampler::new().unwrap();
        thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let raw = sampler.sample().unwrap();

        let own = raw.processes.iter().find(|p| p.pid == process::id());
        let own = own.expect("own process is sampled");
        assert!(own.usage.ram_mb > 0);
        assert!(own.exe.is_some(), "own exe path is read");
        assert!(!raw.per_core_pct.is_empty());
        assert!(raw.memory.total_mb > 0 && raw.memory.available_mb <= raw.memory.total_mb);
        assert!(raw.memory.commit_used_mb <= raw.memory.commit_limit_mb);
        assert!(raw.gpus.is_empty() && raw.drives.is_empty());
    }
}
