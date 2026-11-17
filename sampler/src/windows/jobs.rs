//! A Job Object per session holder, so the CPU time of the holder's short-lived descendants
//! counts for its session.
//!
//! A process's load is the change in its CPU time between two refreshes, so a process that
//! exits after a refresh loses its last stretch, and one that starts and exits between two
//! refreshes is never seen. A clean workspace build runs over 300 processes, more than half of
//! them unseen, and its session measured 76 to 81% of its CPU time; the rest read as load from
//! outside the session. Every process a holder starts after joining its job is a member, and
//! the job's accounting keeps the CPU time of members that exited. Each refresh, the job's CPU
//! time minus what its live members were measured with is credited to the holder itself.
//!
//! A process cannot leave a job. The job sets no limits and allows breakaway, so the holder and
//! the processes it starts behave as before. It goes away with its last member.

use std::collections::HashMap;

use allot_shared::types::Holder;
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
                JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
                JOBOBJECT_BASIC_LIMIT_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
                QueryInformationJobObject, SetInformationJobObject,
            },
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
                PROCESS_TERMINATE,
            },
        },
    },
    core::{BOOL, Owned, PCWSTR},
};

use crate::windows::process;

/// FILETIME ticks (100 ns) per millisecond.
const TICKS_PER_MS: f64 = 10_000.0;

/// A holder process, told apart from a later process with its PID by its start time.
type HolderKey = (u32, u64);

/// The jobs of the holders the sampler was last told about.
#[derive(Default)]
pub(crate) struct HolderJobs {
    holders: HashMap<HolderKey, Tracked>,
}

// Job handles would print as bare addresses; the count says more.
impl std::fmt::Debug for HolderJobs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HolderJobs")
            .field("holders", &self.holders.len())
            .finish()
    }
}

enum Tracked {
    Job(HolderJob),
    /// No job: the holder could not be opened or assigned, or it already runs in another
    /// holder's job, which credits the exited processes of both to that other holder. Not
    /// retried while it stays a holder.
    Untracked,
}

struct HolderJob {
    job: Owned<HANDLE>,
    /// The job's total CPU time at the previous refresh, or when it was created.
    cpu_ms: Option<f64>,
    /// CPU time live members were measured with beyond the job's total, owed by later credits.
    owed_ms: f64,
}

impl HolderJobs {
    /// Starts tracking the holders not tracked yet and forgets the ones left out.
    pub(crate) fn track(&mut self, holders: &[Holder]) {
        let keys: Vec<HolderKey> = holders
            .iter()
            .map(|holder| (holder.pid, holder.pid_start_time))
            .collect();
        self.holders.retain(|key, _| keys.contains(key));
        for key in keys {
            if !self.holders.contains_key(&key) {
                let tracked = self.join(key).map_or(Tracked::Untracked, Tracked::Job);
                self.holders.insert(key, tracked);
            }
        }
    }

    /// Puts the holder in a new job, unless it already runs in one of the others' jobs.
    fn join(&self, (pid, start_time): HolderKey) -> Option<HolderJob> {
        let access = PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION;
        // SAFETY: plain FFI call; `Owned` closes the handle.
        let holder = unsafe { Owned::new(OpenProcess(access, false, pid).ok()?) };
        if process::start_time(*holder).ok()? != start_time || self.contains(*holder) {
            return None;
        }
        // SAFETY: no attributes and no name make an unnamed job with default security.
        let job = unsafe { Owned::new(CreateJobObjectW(None, PCWSTR::null()).ok()?) };
        // Breakaway is an extended limit: the basic class refuses the flag.
        let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_BREAKAWAY_OK,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: `limits` is the extended limit class's structure, and its size is passed.
        unsafe {
            SetInformationJobObject(
                *job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of_u32::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>(),
            )
        }
        .ok()?;
        // SAFETY: both handles are open with the access assigning needs.
        unsafe { AssignProcessToJobObject(*job, *holder) }.ok()?;
        let cpu_ms = cpu_ms(*job);
        Some(HolderJob {
            job,
            cpu_ms,
            owed_ms: 0.0,
        })
    }

    /// Whether `process` runs in one of the tracked jobs.
    fn contains(&self, process: HANDLE) -> bool {
        self.jobs().any(|job| is_member(process, *job.job))
    }

    fn jobs(&self) -> impl Iterator<Item = &HolderJob> {
        self.holders.values().filter_map(|tracked| match tracked {
            Tracked::Job(job) => Some(job),
            Tracked::Untracked => None,
        })
    }

    /// Cores each tracked holder's job used over the last `interval_ms` that `live_cores`,
    /// the measured load of every process in the latest snapshot, does not account for.
    /// Without an interval, on the sampler's first refresh, only the baselines are recorded.
    ///
    /// Membership is asked per busy process of the snapshot rather than read as the job's
    /// member list: a process that exits between the snapshot and the list would be missing
    /// from it, and its last stretch counted both live and in the job's total.
    pub(crate) fn unseen_cores(
        &mut self,
        interval_ms: Option<f64>,
        live_cores: &HashMap<u32, f32>,
    ) -> Vec<(u32, f32)> {
        if !self.jobs().any(|_| true) {
            return Vec::new();
        }
        let busy: Vec<(Owned<HANDLE>, f32)> = live_cores
            .iter()
            .filter(|&(_, &cores)| cores > 0.0)
            .filter_map(|(&pid, &cores)| {
                // SAFETY: plain FFI call; `Owned` closes the handle. The sampler's process
                // table holds a handle to every process it listed, so one that exited since
                // still opens under its PID.
                let opened = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
                Some((unsafe { Owned::new(opened.ok()?) }, cores))
            })
            .collect();
        let mut credits = Vec::new();
        for (&(pid, _), tracked) in &mut self.holders {
            let Tracked::Job(job) = tracked else {
                continue;
            };
            let Some(total_ms) = cpu_ms(*job.job) else {
                continue;
            };
            let previous_ms = job.cpu_ms.replace(total_ms);
            let (Some(previous_ms), Some(interval_ms)) = (previous_ms, interval_ms) else {
                continue;
            };
            let live_ms: f64 = busy
                .iter()
                .filter(|(process, _)| is_member(**process, *job.job))
                .map(|&(_, cores)| f64::from(cores) * interval_ms)
                .sum();
            let credit_ms = settle(total_ms - previous_ms, live_ms, &mut job.owed_ms);
            if interval_ms > 0.0 {
                credits.push((pid, (credit_ms / interval_ms) as f32));
            }
        }
        credits
    }
}

/// The CPU time of the job's members that `live_ms` misses, and the debt carried when the
/// live measure exceeds the job's: the two are read a few milliseconds apart, so either can
/// run ahead for one refresh. Settling the debt later keeps the sum of credits exact.
fn settle(job_ms: f64, live_ms: f64, owed_ms: &mut f64) -> f64 {
    let unseen_ms = job_ms - live_ms - *owed_ms;
    *owed_ms = (-unseen_ms).max(0.0);
    unseen_ms.max(0.0)
}

/// The job's total user and kernel time, including members that exited.
fn cpu_ms(job: HANDLE) -> Option<f64> {
    let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: `info` is the accounting class's structure, and its size is passed.
    unsafe {
        QueryInformationJobObject(
            Some(job),
            JobObjectBasicAccountingInformation,
            (&raw mut info).cast(),
            size_of_u32::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>(),
            None,
        )
    }
    .ok()?;
    Some((info.TotalUserTime + info.TotalKernelTime) as f64 / TICKS_PER_MS)
}

/// Whether `process` runs in `job` or a job nested in it. A process that exited still
/// answers while a handle to it is open.
fn is_member(process: HANDLE, job: HANDLE) -> bool {
    let mut member = BOOL::default();
    // SAFETY: both handles are open and `member` is a valid BOOL.
    unsafe { IsProcessInJob(process, Some(job), &mut member) }.is_ok() && member.as_bool()
}

fn size_of_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        io::{BufRead, BufReader, Read, Write},
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    use allot_core::sampler::{Sampler, attribute};
    use allot_shared::types::SessionKey;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    use super::*;
    use crate::windows::SystemSampler;

    const SPAWN_VAR: &str = "ALLOT_SAMPLER_TEST_SPAWN";
    const CPU_MS_VAR: &str = "ALLOT_SAMPLER_TEST_CPU_MS";
    const SPAWNER: &str = "windows::jobs::tests::spawner_helper";
    const BURNER: &str = "windows::jobs::tests::burn_cpu_time_helper";
    /// Children the spawner starts one after another.
    const CHILDREN: u32 = 12;
    /// CPU time each child uses; well under a sample interval, so most are never sampled.
    const CHILD_CPU_MS: u64 = 150;

    #[test]
    fn test_settle_credits_the_unseen_time() {
        let mut owed = 0.0;
        assert_eq!(settle(900.0, 400.0, &mut owed), 500.0);
        assert_eq!(owed, 0.0);
    }

    #[test]
    fn test_settle_carries_a_live_overshoot_into_the_next_credit() {
        let mut owed = 0.0;
        assert_eq!(settle(300.0, 400.0, &mut owed), 0.0);
        assert_eq!(owed, 100.0);
        assert_eq!(settle(700.0, 400.0, &mut owed), 200.0);
        assert_eq!(owed, 0.0);
    }

    fn own_cpu_ms() -> u64 {
        let mut times = [Default::default(); 4];
        let [creation, exit, kernel, user] = &mut times;
        // SAFETY: the pseudo handle of the current process has every access right.
        unsafe { GetProcessTimes(GetCurrentProcess(), creation, exit, kernel, user) }.unwrap();
        let ticks = |time: &windows::Win32::Foundation::FILETIME| {
            (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
        };
        (ticks(kernel) + ticks(user)) / 10_000
    }

    /// Not a test: a child of [`spawner_helper`]. Spins until it used
    /// `ALLOT_SAMPLER_TEST_CPU_MS` of CPU time, and does nothing without it.
    #[test]
    #[ignore = "helper process for the holder job tests"]
    fn burn_cpu_time_helper() {
        let Some(target) = env::var(CPU_MS_VAR).ok().and_then(|ms| ms.parse().ok()) else {
            return;
        };
        while own_cpu_ms() < target {
            std::hint::spin_loop();
        }
    }

    /// Not a test: the holder in [`test_holder_job_counts_children_too_short_to_sample`].
    /// Waits for a line on stdin, runs `ALLOT_SAMPLER_TEST_SPAWN` burners one after another,
    /// says `done`, and exits when stdin closes.
    #[test]
    #[ignore = "helper process for the holder job tests"]
    fn spawner_helper() {
        let Some(children) = env::var(SPAWN_VAR).ok().and_then(|n| n.parse::<u32>().ok()) else {
            return;
        };
        let mut stdin = BufReader::new(std::io::stdin());
        stdin.read_line(&mut String::new()).unwrap();
        for _ in 0..children {
            Command::new(env::current_exe().unwrap())
                .args([BURNER, "--exact", "--ignored"])
                .env(CPU_MS_VAR, CHILD_CPU_MS.to_string())
                .env_remove(SPAWN_VAR)
                .stdout(Stdio::null())
                .status()
                .unwrap();
        }
        println!("done");
        std::io::stdout().flush().unwrap();
        stdin.read_to_end(&mut Vec::new()).unwrap();
    }

    #[test]
    fn test_holder_job_counts_children_too_short_to_sample() {
        let mut spawner = Command::new(env::current_exe().unwrap())
            .args([SPAWNER, "--exact", "--ignored", "--quiet", "--nocapture"])
            .env(SPAWN_VAR, CHILDREN.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let holder = Holder {
            session: SessionKey("spawner".into()),
            pid: spawner.id(),
            pid_start_time: process::start_time_of(spawner.id()).unwrap(),
        };
        let (done, finished) = mpsc::channel();
        let output = spawner.stdout.take().unwrap();
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                if line.unwrap().trim() == "done" {
                    let _ = done.send(());
                }
            }
        });

        let mut sampler = SystemSampler::new().unwrap();
        sampler.track_holders(std::slice::from_ref(&holder));
        let mut previous_at_ms = Some(sampler.sample().unwrap().taken_at_ms);
        let mut stdin = spawner.stdin.take().unwrap();
        writeln!(stdin, "go").unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut measured_ms = 0.0;
        loop {
            thread::sleep(Duration::from_secs(1));
            let last = finished.try_recv().is_ok();
            let raw = sampler.sample().unwrap();
            let (sessions, _) = attribute(&raw.processes, std::slice::from_ref(&holder));
            if let Some(at) = previous_at_ms {
                let interval_ms = (raw.taken_at_ms - at) as f64;
                measured_ms += f64::from(sessions[0].usage.cores) * interval_ms;
            }
            previous_at_ms = Some(raw.taken_at_ms);
            if last {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the spawner did not finish in 60 s"
            );
        }
        drop(stdin);
        spawner.wait().unwrap();

        let burned_ms = f64::from(CHILDREN) * CHILD_CPU_MS as f64;
        // Each child also starts a test binary, so the session may read somewhat more.
        assert!(
            measured_ms >= burned_ms * 0.9 && measured_ms <= burned_ms * 3.0,
            "measured {measured_ms:.0} ms of {burned_ms:.0} ms burned"
        );
    }
}
