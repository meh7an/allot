//! The live macOS sampler.

use std::{
    collections::HashMap,
    mem::MaybeUninit,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use allot_core::sampler::{RawProcess, RawSample, Sampler};
use allot_shared::{Error, Result, types::ResourceUsage};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::{
    cpu::{CpuTimes, ProcessTime},
    macos::{
        drives::Drives,
        gpu::{self, GpuReading, GpuTimes},
        iokit,
        memory::{self, PressureLevel},
        value::Entry,
    },
};

const BYTES_PER_MB: u64 = 1024 * 1024;

/// Reads the local machine.
///
/// A process's RAM is its physical footprint, as Activity Monitor shows it: dirty and
/// compressed memory, including what it allocated for the GPU, but not clean file pages.
/// macOS gives the footprint only for the user's own processes; others show their resident
/// size.
#[derive(Debug)]
pub struct SystemSampler {
    system: System,
    cpu_times: CpuTimes,
    /// Logical cores per PID since the previous refresh.
    process_cores: HashMap<u32, f32>,
    drives: Drives,
    gpu_times: GpuTimes,
    /// GPU use as of the last refresh.
    gpu: GpuReading,
}

impl SystemSampler {
    /// Takes a first reading, so the next [`Sampler::sample`] has a baseline for its rates.
    ///
    /// Drives and GPUs are best effort: a drive whose disk driver is not found reads as
    /// idle, and a Mac whose GPU publishes no statistics (an Intel Mac, or a virtual
    /// machine) lists no adapters.
    ///
    /// # Errors
    ///
    /// Never fails; the `Result` matches the Windows sampler, whose counters can fail to open.
    pub fn new() -> Result<Self> {
        let mut sampler = Self {
            system: System::new(),
            cpu_times: CpuTimes::default(),
            process_cores: HashMap::new(),
            drives: Drives::new(),
            gpu_times: GpuTimes::default(),
            gpu: GpuReading::default(),
        };
        sampler.refresh();
        Ok(sampler)
    }

    /// The kernel's memory pressure level right now; `None` when it cannot be read. Nothing
    /// reads it yet; it is the macOS side of a machine pressure signal for later admission
    /// work.
    pub fn pressure(&self) -> Option<PressureLevel> {
        PressureLevel::read()
    }

    fn refresh(&mut self) {
        self.system.refresh_cpu_usage();
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
        let now = Instant::now();
        let logical_cores = self.system.cpus().len().max(1) as f32;
        self.process_cores = self.cpu_times.update(now, &times, logical_cores);
        self.drives.refresh(now);
        self.gpu = self.gpu_times.read(&accelerators(), now);
    }
}

impl Sampler for SystemSampler {
    fn sample(&mut self) -> Result<RawSample> {
        self.refresh();
        let memory = memory::read()
            .map_err(|error| Error::Sampler(format!("reading VM statistics: {error}")))?;
        let gpu = std::mem::take(&mut self.gpu);
        let processes = self
            .system
            .processes()
            .values()
            .map(|process| {
                let pid = process.pid().as_u32();
                let ram_bytes = footprint_bytes(pid).unwrap_or_else(|| process.memory());
                RawProcess {
                    pid,
                    parent_pid: process.parent().map(Pid::as_u32),
                    name: process.name().to_string_lossy().into_owned(),
                    exe: process.exe().map(|path| path.display().to_string()),
                    start_time: process.start_time(),
                    usage: ResourceUsage {
                        cores: self.process_cores.get(&pid).copied().unwrap_or_default(),
                        ram_mb: ram_bytes / BYTES_PER_MB,
                        gpu_pct: gpu.processes.get(&pid).copied().unwrap_or_default(),
                        vram_mb: 0,
                    },
                }
            })
            .collect();
        let taken_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        Ok(RawSample {
            taken_at_ms,
            cpu_total_pct: self.system.global_cpu_usage(),
            per_core_pct: self
                .system
                .cpus()
                .iter()
                .map(|cpu| cpu.cpu_usage())
                .collect(),
            memory: memory.mem_stats(),
            processes,
            gpus: gpu.adapters,
            drives: self.drives.read(),
        })
    }
}

/// The GPU accelerators with their clients. Only Apple Silicon GPUs are read; on an Intel
/// Mac the GPU has dedicated memory and other statistics.
fn accelerators() -> Vec<Entry> {
    if cfg!(target_arch = "aarch64") {
        iokit::entries(c"IOAccelerator", &gpu::ACCELERATOR_KEYS, &gpu::CLIENT_KEYS)
    } else {
        Vec::new()
    }
}

/// The physical footprint of `pid`; `None` for another user's process, which macOS does
/// not let a normal user read.
fn footprint_bytes(pid: u32) -> Option<u64> {
    let pid = i32::try_from(pid).ok()?;
    let mut info = MaybeUninit::<libc::rusage_info_v4>::zeroed();
    // SAFETY: `info` is writable and as large as the V4 structure the flavor asks for.
    let result =
        unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V4, info.as_mut_ptr().cast()) };
    // SAFETY: the call succeeded and filled the structure, which started zeroed anyway.
    (result == 0).then(|| unsafe { info.assume_init() }.ri_phys_footprint)
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, path::Path, process, thread, time::Duration};

    use allot_shared::types::{Accuracy, GpuSource};

    use super::*;

    #[test]
    fn test_system_sampler_sample_reads_own_process_and_memory() {
        let mut sampler = SystemSampler::new().unwrap();
        thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let raw = sampler.sample().unwrap();

        let own = raw.processes.iter().find(|p| p.pid == process::id());
        let own = own.expect("own process is sampled");
        assert!(own.usage.ram_mb > 0);
        let exe = own.exe.as_deref().expect("own exe path is read");
        assert_eq!(
            fs::canonicalize(exe).unwrap(),
            fs::canonicalize(std::env::current_exe().unwrap()).unwrap()
        );
        assert!(!raw.per_core_pct.is_empty());
        assert!(raw.memory.total_mb > 0 && raw.memory.available_mb <= raw.memory.total_mb);
        assert_eq!(
            raw.memory.used_mb,
            raw.memory.total_mb - raw.memory.available_mb
        );
        assert!(raw.memory.commit_used_mb <= raw.memory.commit_limit_mb);
    }

    #[test]
    fn test_footprint_reads_own_memory_but_not_launchd() {
        let own = footprint_bytes(process::id()).expect("own footprint is readable");
        assert!(own > 0);
        // launchd belongs to root; a normal user cannot read it.
        // SAFETY: getuid has no preconditions.
        if unsafe { libc::getuid() } != 0 {
            assert_eq!(footprint_bytes(1), None);
        }
    }

    #[test]
    fn test_system_sampler_counts_touched_memory_in_the_footprint() {
        const TOUCHED_MB: usize = 256;
        let mut sampler = SystemSampler::new().unwrap();
        let before = own_ram_mb(&mut sampler);
        let block = vec![1_u8; TOUCHED_MB * 1024 * 1024];
        let after = own_ram_mb(&mut sampler);
        assert!(std::hint::black_box(&block)[block.len() - 1] == 1);
        drop(block);
        assert!(
            after >= before + TOUCHED_MB as u64 / 2,
            "footprint went from {before} to {after} MB"
        );
    }

    fn own_ram_mb(sampler: &mut SystemSampler) -> u64 {
        let raw = sampler.sample().unwrap();
        let own = raw.processes.iter().find(|p| p.pid == process::id());
        own.expect("own process is sampled").usage.ram_mb
    }

    #[test]
    fn test_system_sampler_lists_well_formed_gpus() {
        // CI runners are virtual machines whose GPU may publish no statistics, so only the
        // shape is checked; a Mac's numbers are checked by hand.
        let mut sampler = SystemSampler::new().unwrap();
        let raw = sampler.sample().unwrap();
        for gpu in &raw.gpus {
            assert_eq!(gpu.source, GpuSource::Iokit, "{gpu:?}");
            assert!(gpu.unified_memory);
            assert_eq!(gpu.process_load, Accuracy::Estimated);
            assert_eq!(gpu.process_memory, Accuracy::Unavailable);
            assert!(gpu.engines.busiest() <= 100.0, "{gpu:?}");
        }
        assert!(raw.processes.iter().all(|p| p.usage.gpu_pct <= 100.0));
        assert!(raw.processes.iter().all(|p| p.usage.vram_mb == 0));
    }

    #[test]
    fn test_system_sampler_lists_the_startup_volume_with_plausible_usage() {
        let mut sampler = SystemSampler::new().unwrap();
        let raw = sampler.sample().unwrap();
        let root = raw.drives.iter().find(|drive| drive.name == "/");
        let root = root.unwrap_or_else(|| panic!("/ is a fixed drive: {:?}", raw.drives));
        assert!(root.total_mb > 1_024, "/ is {} MB", root.total_mb);
        assert!(root.used_mb > 0 && root.used_mb < root.total_mb, "{root:?}");
        assert!(raw.drives.iter().all(|drive| drive.name.starts_with('/')));
    }

    /// Bytes written during the activity test; enough to stand out from background writes.
    const WRITTEN_BYTES: usize = 64 * 1024 * 1024;

    #[test]
    fn test_system_sampler_reads_write_activity_on_the_startup_volume() {
        let exe = std::env::current_exe().unwrap();
        let dir = exe.parent().unwrap();
        let path = dir.join(format!("allot-sampler-io-{}.bin", process::id()));

        let mut sampler = SystemSampler::new().unwrap();
        let mut file = fs::File::create(&path).unwrap();
        let chunk = vec![0x5a_u8; 1024 * 1024];
        for _ in 0..WRITTEN_BYTES / chunk.len() {
            file.write_all(&chunk).unwrap();
        }
        // Without the flush the data could still sit in the page cache when it is read.
        file.sync_all().unwrap();
        drop(file);
        thread::sleep(Duration::from_millis(250));
        let raw = sampler.sample().unwrap();
        fs::remove_file(&path).ok();

        // The build directory is on the Data volume, which shares the startup disk with `/`;
        // a build on an external volume is not checked.
        let root = raw.drives.iter().find(|drive| drive.name == "/");
        let root = root.unwrap_or_else(|| panic!("/ is listed: {:?}", raw.drives));
        assert!(
            Path::new(dir).starts_with("/Volumes") || root.write_bytes_per_sec > 0,
            "/ read no write activity: {root:?}"
        );
    }

    #[test]
    fn test_system_sampler_pressure_is_readable() {
        let sampler = SystemSampler::new().unwrap();
        assert!(sampler.pressure().is_some());
    }

    const BURN_MS_VAR: &str = "ALLOT_SAMPLER_TEST_BURN_MS";
    /// CPU time a child must have used before it counts as busy; start-up alone stays below.
    const BUSY_CPU_MS: u64 = 100;

    /// Not a test: the child process of [`busy_child`]. Keeps one core busy for
    /// `ALLOT_SAMPLER_TEST_BURN_MS` milliseconds and does nothing without it.
    #[test]
    #[ignore = "helper process for the CPU sampling tests"]
    fn burn_cpu_helper() {
        let Some(millis) = std::env::var(BURN_MS_VAR)
            .ok()
            .and_then(|ms| ms.parse().ok())
        else {
            return;
        };
        let end = Instant::now() + Duration::from_millis(millis);
        while Instant::now() < end {
            std::hint::spin_loop();
        }
    }

    /// This test binary running [`burn_cpu_helper`] for `millis`. Returns only once the child
    /// is burning, because start-up on a loaded CI machine can outlast a fixed sleep.
    fn busy_child(millis: u64) -> process::Child {
        let child = process::Command::new(std::env::current_exe().unwrap())
            .args([
                "macos::system::tests::burn_cpu_helper",
                "--exact",
                "--ignored",
            ])
            .env(BURN_MS_VAR, millis.to_string())
            .stdout(process::Stdio::null())
            .spawn()
            .unwrap();
        wait_until_busy(child.id());
        child
    }

    fn wait_until_busy(pid: u32) {
        let pid = Pid::from_u32(pid);
        let mut system = System::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().with_cpu(),
            );
            let used = system.process(pid).map_or(0, |p| p.accumulated_cpu_time());
            if used >= BUSY_CPU_MS {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the burner used only {used} ms of CPU in 30 s"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn sampled_cores(raw: &RawSample, pid: u32) -> f32 {
        let child = raw.processes.iter().find(|p| p.pid == pid);
        child.expect("the child is sampled").usage.cores
    }

    #[test]
    fn test_system_sampler_counts_a_process_started_since_the_previous_sample() {
        let mut sampler = SystemSampler::new().unwrap();
        let mut burner = busy_child(3_000);
        thread::sleep(Duration::from_millis(1_500));
        let raw = sampler.sample().unwrap();
        burner.kill().ok();
        burner.wait().ok();

        let cores = sampled_cores(&raw, burner.id());
        assert!(cores > 0.3, "a busy new process read {cores} cores");
    }

    #[test]
    fn test_system_sampler_first_sample_reads_a_process_that_was_already_running() {
        let mut burner = busy_child(4_000);
        thread::sleep(Duration::from_millis(1_000));
        let mut sampler = SystemSampler::new().unwrap();
        thread::sleep(Duration::from_millis(1_000));
        let raw = sampler.sample().unwrap();
        burner.kill().ok();
        burner.wait().ok();

        let cores = sampled_cores(&raw, burner.id());
        assert!(
            cores > 0.5,
            "a busy running process read {cores} cores in the first sample"
        );
    }
}
