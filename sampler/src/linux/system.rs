//! The live Linux sampler over the real `/proc` and mounted filesystems.

use std::{
    collections::HashMap,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use allot_core::sampler::{RawProcess, RawSample, Sampler};
use allot_shared::{Error, Result, types::ResourceUsage};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::{
    cpu::{CpuTimes, ProcessTime},
    linux::{drives::Drives, meminfo::MemInfo, pressure::Pressure},
};

const BYTES_PER_MB: u64 = 1024 * 1024;

/// The machine's own root; the parsers take it so fixtures can stand in for it.
const ROOT: &str = "/";

/// Reads the local machine.
///
/// Process identity is the PID with the start time sysinfo derives from the `starttime`
/// field of `/proc/PID/stat` and the boot time in `/proc/stat`, so it does not drift
/// between samples.
#[derive(Debug)]
pub struct SystemSampler {
    system: System,
    cpu_times: CpuTimes,
    /// Logical cores per PID since the previous refresh.
    process_cores: HashMap<u32, f32>,
    drives: Drives,
}

impl SystemSampler {
    /// Takes a first reading, so the next [`Sampler::sample`] has a baseline for its rates.
    ///
    /// Drives are best effort: without the mount table or `/proc/diskstats` samples list
    /// fewer drives or read them as idle.
    ///
    /// # Errors
    ///
    /// Never fails; the `Result` matches the Windows sampler, whose counters can fail to open.
    pub fn new() -> Result<Self> {
        let mut sampler = Self {
            system: System::new(),
            cpu_times: CpuTimes::default(),
            process_cores: HashMap::new(),
            drives: Drives::new(Path::new(ROOT)),
        };
        sampler.refresh();
        Ok(sampler)
    }

    /// CPU, memory, and I/O pressure right now. Nothing reads it yet; it is the Linux side of
    /// a machine pressure signal for later admission work.
    pub fn pressure(&self) -> Pressure {
        Pressure::read(Path::new(ROOT))
    }

    fn refresh(&mut self) {
        self.system.refresh_cpu_usage();
        // Without tasks, threads are not listed as processes of their own.
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_exe(UpdateKind::OnlyIfNotSet)
                .without_tasks(),
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
    }
}

impl Sampler for SystemSampler {
    fn sample(&mut self) -> Result<RawSample> {
        self.refresh();
        let memory = MemInfo::read(Path::new(ROOT))
            .map_err(|error| Error::Sampler(format!("reading /proc/meminfo: {error}")))?;
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
            gpus: Vec::new(),
            drives: self.drives.read(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, process, thread, time::Duration};

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
        assert_eq!(Path::new(exe), std::env::current_exe().unwrap());
        assert!(!raw.per_core_pct.is_empty());
        assert!(raw.memory.total_mb > 0 && raw.memory.available_mb <= raw.memory.total_mb);
        assert_eq!(
            raw.memory.used_mb,
            raw.memory.total_mb - raw.memory.available_mb
        );
        assert!(raw.memory.commit_used_mb <= raw.memory.commit_limit_mb);
        assert!(raw.gpus.is_empty());
    }

    #[test]
    fn test_system_sampler_lists_no_threads_as_processes() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            // SAFETY: gettid has no preconditions.
            sender.send(unsafe { libc::gettid() }).unwrap();
            thread::sleep(Duration::from_secs(1));
        });
        let thread_id = u32::try_from(receiver.recv().unwrap()).unwrap();
        let mut sampler = SystemSampler::new().unwrap();
        let raw = sampler.sample().unwrap();
        worker.join().unwrap();

        assert!(raw.processes.iter().all(|p| p.pid != thread_id));
        assert!(raw.processes.iter().any(|p| p.pid == process::id()));
    }

    #[test]
    fn test_system_sampler_lists_root_drive_with_plausible_usage() {
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
    fn test_system_sampler_reads_write_activity_during_a_file_write() {
        // Not the temp dir, which can be a tmpfs with no device behind it.
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
        let dir = fs::canonicalize(dir).unwrap();
        fs::remove_file(&path).ok();

        let usage = raw
            .drives
            .iter()
            .filter(|drive| dir.starts_with(&drive.name))
            .max_by_key(|drive| drive.name.len());
        let usage = usage.unwrap_or_else(|| panic!("{dir:?} is on a drive: {:?}", raw.drives));
        assert!(
            usage.write_bytes_per_sec > 0,
            "{} read no write activity: {usage:?}",
            usage.name
        );
    }

    #[test]
    fn test_system_sampler_pressure_matches_what_the_kernel_offers() {
        let sampler = SystemSampler::new().unwrap();
        let pressure = sampler.pressure();
        let offered = Path::new("/proc/pressure/memory").exists();
        assert_eq!(pressure.memory.is_some(), offered, "{pressure:?}");
        if let Some(memory) = pressure.memory {
            assert!((0.0..=100.0).contains(&memory.some.avg10), "{memory:?}");
        }
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
                "linux::system::tests::burn_cpu_helper",
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
