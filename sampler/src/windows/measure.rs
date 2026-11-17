//! A measurement, not a test: how much of a clean workspace build's CPU time the sampler
//! attributes to the session that ran it, with and without a holder job.
//!
//! Run it under an Allot lease, with a fresh target directory each time:
//!
//! ```text
//! ALLOT_MEASURE_TARGET_DIR=<new dir> cargo test -p allot-sampler --lib \
//!     windows::measure::measure_clean_build_cpu -- --exact --ignored --nocapture
//! ```
//!
//! The test process is the holder. It puts itself in a job of its own first, whose accounting
//! is the exact total, then samples the build with two samplers once a second: one per process
//! only, and one tracking the holder, which nests its job inside the first.

use std::{
    collections::HashMap,
    env,
    path::Path,
    process::{self, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use allot_core::sampler::{Sampler, attribute};
use allot_shared::types::{Holder, SessionKey};
use windows::{
    Win32::{
        Foundation::{FILETIME, HANDLE},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation,
                QueryInformationJobObject,
            },
            Threading::{
                GetCurrentProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    },
    core::{BOOL, Owned, PCWSTR},
};

use crate::windows::{SystemSampler, process::start_time_of};

const TARGET_DIR_VAR: &str = "ALLOT_MEASURE_TARGET_DIR";
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
/// FILETIME ticks per CPU millisecond.
const TICKS_PER_MS: f64 = 10_000.0;

fn job_accounting(job: HANDLE) -> JOBOBJECT_BASIC_ACCOUNTING_INFORMATION {
    let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: `info` is the accounting class's structure, and its size is passed.
    unsafe {
        QueryInformationJobObject(
            Some(job),
            JobObjectBasicAccountingInformation,
            (&raw mut info).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            None,
        )
    }
    .unwrap();
    info
}

fn job_cpu_ms(info: &JOBOBJECT_BASIC_ACCOUNTING_INFORMATION) -> f64 {
    (info.TotalUserTime + info.TotalKernelTime) as f64 / TICKS_PER_MS
}

/// Whether `pid` runs in `job`; a process that exited or cannot be opened counts as inside.
fn in_job(pid: u32, job: HANDLE) -> bool {
    // SAFETY: plain FFI call; `Owned` closes the handle.
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return true;
    };
    let process = unsafe { Owned::new(process) };
    let mut member = BOOL::default();
    // SAFETY: both handles are open and `member` is a valid BOOL.
    unsafe { IsProcessInJob(*process, Some(job), &mut member) }.is_err() || member.as_bool()
}

fn own_cpu_ms() -> f64 {
    let mut times = [FILETIME::default(); 4];
    let [creation, exit, kernel, user] = &mut times;
    // SAFETY: the pseudo handle of the current process has every access right.
    unsafe { GetProcessTimes(GetCurrentProcess(), creation, exit, kernel, user) }.unwrap();
    let ticks =
        |time: &FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    (ticks(kernel) + ticks(user)) as f64 / TICKS_PER_MS
}

/// CPU milliseconds one sampler attributes to the holder, sample by sample, and the part of
/// it on processes outside `job`, by name.
struct Attributed {
    sampler: SystemSampler,
    previous_at_ms: Option<i64>,
    total_ms: f64,
    outside_ms: HashMap<String, f64>,
}

impl Attributed {
    fn new(sampler: SystemSampler) -> Self {
        Self {
            sampler,
            previous_at_ms: None,
            total_ms: 0.0,
            outside_ms: HashMap::new(),
        }
    }

    fn sample(&mut self, holder: &Holder, job: HANDLE) {
        let raw = self.sampler.sample().unwrap();
        let (sessions, _) = attribute(&raw.processes, std::slice::from_ref(holder));
        if let Some(at) = self.previous_at_ms {
            let interval_ms = (raw.taken_at_ms - at) as f64;
            self.total_ms += f64::from(sessions[0].usage.cores) * interval_ms;
            for usage in &sessions[0].processes {
                if usage.usage.cores > 0.0 && !in_job(usage.pid, job) {
                    *self.outside_ms.entry(usage.name.clone()).or_default() +=
                        f64::from(usage.usage.cores) * interval_ms;
                }
            }
        }
        self.previous_at_ms = Some(raw.taken_at_ms);
    }
}

#[test]
#[ignore = "measures a clean workspace build; run by hand under an Allot lease"]
fn measure_clean_build_cpu() {
    let target_dir = env::var(TARGET_DIR_VAR).expect("ALLOT_MEASURE_TARGET_DIR is set");
    assert!(
        !Path::new(&target_dir).exists(),
        "{target_dir} exists; the build would not be clean"
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();

    // SAFETY: no attributes and no name make an unnamed job with default security.
    let job = unsafe { Owned::new(CreateJobObjectW(None, PCWSTR::null()).unwrap()) };
    // SAFETY: both handles are valid; children started from here on join the job.
    unsafe { AssignProcessToJobObject(*job, GetCurrentProcess()) }.unwrap();

    let holder = Holder {
        session: SessionKey("measure".into()),
        pid: process::id(),
        pid_start_time: start_time_of(process::id()).unwrap(),
    };
    let mut plain = Attributed::new(SystemSampler::new().unwrap());
    let mut tracked = Attributed::new(SystemSampler::new().unwrap());
    tracked.sampler.track_holders(std::slice::from_ref(&holder));
    plain.sample(&holder, *job);
    tracked.sample(&holder, *job);
    let job_before = job_accounting(*job);
    let own_before = own_cpu_ms();
    let started = Instant::now();
    let mut build = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--workspace", "--target-dir", &target_dir])
        .current_dir(workspace)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut samples = 0;
    loop {
        thread::sleep(SAMPLE_INTERVAL);
        let finished = build.try_wait().unwrap().is_some();
        plain.sample(&holder, *job);
        tracked.sample(&holder, *job);
        samples += 1;
        if finished {
            break;
        }
    }
    let wall = started.elapsed();
    let job_after = job_accounting(*job);
    let own_ms = own_cpu_ms() - own_before;

    let job_ms = job_cpu_ms(&job_after) - job_cpu_ms(&job_before) - own_ms;
    let processes = job_after.TotalProcesses - job_before.TotalProcesses;
    let row = |name: &str, ms: f64| {
        let share = ms / job_ms * 100.0;
        println!("{name:<26}{:>7.1} s ({share:.0}%)", ms / 1_000.0);
    };
    println!("build status: {:?}", build.wait().unwrap());
    println!("wall: {:.1} s, samples: {samples}", wall.as_secs_f64());
    println!("processes in the job: {processes}");
    println!(
        "measurement's own CPU (left out below): {:.1} s",
        own_ms / 1_000.0
    );
    row("job total (exact):", job_ms);
    row("per process only:", plain.total_ms - own_ms);
    row("with the holder job:", tracked.total_ms - own_ms);
    let mut outside: Vec<_> = tracked.outside_ms.into_iter().collect();
    outside.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (name, ms) in outside.iter().take(8) {
        println!("  outside the job: {name} {:.1} s", ms / 1_000.0);
    }
}
