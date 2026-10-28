//! Terminating a planned process tree with signals: SIGTERM to every process, root first,
//! then SIGKILL to whatever still runs after a grace period.
//!
//! A dying parent's children are re-parented, so every sweep also plans the members of the
//! sessions the tree leads ([`KillPlan::sweep_sessions`]). Every process's start time is
//! checked before it is signalled. On Linux the check is made on a pidfd, which keeps naming
//! that process even if its PID is reused; elsewhere the PID could be reused between the
//! check and the signal, a window of microseconds.

use std::{
    collections::HashMap,
    io, thread,
    time::{Duration, Instant},
};

use allot_core::kill::{KillFailure, KillPlan, KillReport, KillTarget};
use libc::c_int;

use crate::table::{process_table_with_sessions, running_start_time};

/// Sweeps per signal at most; each takes a fresh process list and catches what the previous
/// one missed.
const SWEEPS: u32 = 3;

/// Pause between sweeps, so processes started by dying parents show up.
const SWEEP_PAUSE: Duration = Duration::from_millis(100);

/// How long the processes get to exit after SIGTERM before they are sent SIGKILL.
const GRACE: Duration = Duration::from_secs(3);

/// How long to wait for the processes sent SIGKILL to exit before reporting.
const EXIT_WAIT: Duration = Duration::from_millis(500);

/// How often waiting checks whether the signalled processes have exited.
const EXIT_POLL: Duration = Duration::from_millis(20);

/// A process identified across PID reuse.
type ProcessKey = (u32, u64);

/// What happened to one signal.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Signalled,
    /// It exited, or its PID now belongs to another process.
    Gone,
    Failed(String),
}

/// Sends SIGTERM to the processes `plan` covers, root first, gives them [`GRACE`] to exit,
/// and sends SIGKILL to the rest.
///
/// Each signal is sent in up to three sweeps, and every sweep reads the running processes
/// again, so processes a dying parent started at the last moment get it too.
pub fn kill_tree(plan: &mut KillPlan) -> KillReport {
    let mut kill = Kill::default();
    kill.send(plan, libc::SIGTERM);
    kill.wait_for_exit(GRACE);
    kill.send(plan, libc::SIGKILL);
    kill.wait_for_exit(EXIT_WAIT);
    kill.report(plan)
}

/// The state of one [`kill_tree`] run.
#[derive(Debug, Default)]
struct Kill {
    /// Every process signalled so far, with the last signal it was sent.
    held: HashMap<ProcessKey, (Held, c_int)>,
    report: KillReport,
    failures: HashMap<ProcessKey, KillFailure>,
}

impl Kill {
    /// Sends `signal` to every planned process that runs and has not been sent it yet.
    fn send(&mut self, plan: &mut KillPlan, signal: c_int) {
        for sweep in 0..SWEEPS {
            if sweep > 0 {
                thread::sleep(SWEEP_PAUSE);
            }
            let (table, sessions) = process_table_with_sessions();
            let targets: Vec<KillTarget> = plan
                .sweep_sessions(&table, &sessions)
                .into_iter()
                .filter(|target| {
                    let key = (target.pid, target.start_time);
                    self.held.get(&key).is_none_or(|&(_, sent)| sent != signal)
                })
                .collect();
            if targets.is_empty() {
                break;
            }
            self.report.sweeps += 1;
            for target in targets {
                self.signal(target, signal);
            }
        }
    }

    fn signal(&mut self, target: KillTarget, signal: c_int) {
        let key = (target.pid, target.start_time);
        let outcome = match self.held.get_mut(&key) {
            Some((held, sent)) => {
                *sent = signal;
                held.signal(signal)
            }
            None => match Held::open(&target) {
                Ok(Some(held)) => {
                    let outcome = held.signal(signal);
                    self.held.insert(key, (held, signal));
                    outcome
                }
                Ok(None) => Outcome::Gone,
                Err(error) => Outcome::Failed(error.to_string()),
            },
        };
        match outcome {
            Outcome::Signalled => {
                self.failures.remove(&key);
                if !self.report.terminated.contains(&target) {
                    self.report.terminated.push(target);
                }
            }
            Outcome::Gone => {
                self.failures.remove(&key);
            }
            Outcome::Failed(error) => {
                self.failures.insert(key, KillFailure { target, error });
            }
        }
    }

    /// Waits up to `limit` for every signalled process to exit.
    fn wait_for_exit(&self, limit: Duration) {
        let deadline = Instant::now() + limit;
        while self.held.values().any(|(held, _)| !held.has_exited()) {
            if Instant::now() >= deadline {
                return;
            }
            thread::sleep(EXIT_POLL);
        }
    }

    fn report(self, plan: &KillPlan) -> KillReport {
        let mut report = self.report;
        report.failed = self.failures.into_values().collect();
        report.failed.sort_by_key(|failure| failure.target.depth);
        report.spared = plan.spared().to_vec();
        report
    }
}

/// A planned process whose start time was checked, ready for signals.
#[derive(Debug)]
struct Held {
    pid: c_int,
    start_time: u64,
    /// Keeps naming the checked process, so a reused PID is never signalled; `None` where
    /// the kernel has no pidfds (before Linux 5.3).
    #[cfg(target_os = "linux")]
    pidfd: Option<std::os::fd::OwnedFd>,
}

impl Held {
    /// Checks `target`'s start time, or `None` when it exited or its PID was reused.
    fn open(target: &KillTarget) -> io::Result<Option<Self>> {
        let pid = c_int::try_from(target.pid)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "the PID is out of range"))?;
        let held = Self {
            pid,
            start_time: target.start_time,
            #[cfg(target_os = "linux")]
            pidfd: match pidfd::open(pid) {
                Ok(pidfd) => Some(pidfd),
                Err(error) if error.raw_os_error() == Some(libc::ESRCH) => return Ok(None),
                Err(error) if error.raw_os_error() == Some(libc::ENOSYS) => None,
                Err(error) => return Err(error),
            },
        };
        // Opened first, the pidfd names this process if the start time still matches now.
        Ok(held.is_running().then_some(held))
    }

    fn signal(&self, signal: c_int) -> Outcome {
        #[cfg(target_os = "linux")]
        if let Some(pidfd) = &self.pidfd {
            return outcome(pidfd::send_signal(pidfd, signal));
        }
        if !self.is_running() {
            return Outcome::Gone;
        }
        // SAFETY: plain FFI call with a PID and a signal number.
        let result = unsafe { libc::kill(self.pid, signal) };
        outcome(if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        })
    }

    fn has_exited(&self) -> bool {
        #[cfg(target_os = "linux")]
        if let Some(pidfd) = &self.pidfd {
            return pidfd::has_exited(pidfd);
        }
        !self.is_running()
    }

    fn is_running(&self) -> bool {
        u32::try_from(self.pid)
            .ok()
            .and_then(running_start_time)
            .is_some_and(|start_time| start_time == self.start_time)
    }
}

fn outcome(result: io::Result<()>) -> Outcome {
    match result {
        Ok(()) => Outcome::Signalled,
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => Outcome::Gone,
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

/// Process file descriptors, called through `syscall` so they work with any glibc.
#[cfg(target_os = "linux")]
mod pidfd {
    use std::{
        io,
        os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        ptr,
    };

    use libc::c_int;

    /// A pidfd for the process `pid`.
    pub(super) fn open(pid: c_int) -> io::Result<OwnedFd> {
        // SAFETY: pidfd_open takes a PID and flags, and returns a new descriptor or -1.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = RawFd::try_from(fd).map_err(io::Error::other)?;
        // SAFETY: the kernel just returned this descriptor, and nothing else owns it.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    pub(super) fn send_signal(pidfd: &OwnedFd, signal: c_int) -> io::Result<()> {
        // SAFETY: an open pidfd, a signal number, no siginfo, and no flags.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                pidfd.as_raw_fd(),
                signal,
                ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Whether the process has exited; a pidfd turns readable when it does.
    pub(super) fn has_exited(pidfd: &OwnedFd) -> bool {
        let mut poll = libc::pollfd {
            fd: pidfd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd, and a zero timeout.
        let ready = unsafe { libc::poll(&mut poll, 1, 0) };
        ready > 0
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command, Stdio};

    use super::*;
    use crate::table::process_table;

    /// A process this test owns that sleeps until it is signalled.
    fn sleeper() -> Child {
        Command::new("sleep")
            .arg("60")
            .stdin(Stdio::null())
            .spawn()
            .unwrap()
    }

    fn target_of(child: &Child) -> KillTarget {
        let node = process_table().get(child.id()).unwrap().clone();
        KillTarget {
            pid: node.pid,
            start_time: node.start_time,
            name: node.name,
            depth: 0,
        }
    }

    #[test]
    fn test_running_start_time_of_own_process_matches_the_process_table() {
        let own = process_table().get(std::process::id()).unwrap().start_time;
        assert_eq!(running_start_time(std::process::id()), Some(own));
    }

    #[test]
    fn test_open_reports_a_missing_process_as_gone() {
        let target = KillTarget {
            pid: 0x3fff_fff0,
            start_time: 1,
            name: "missing".into(),
            depth: 0,
        };
        assert!(Held::open(&target).unwrap().is_none());
    }

    #[test]
    fn test_open_checks_the_start_time_before_signalling() {
        let mut child = sleeper();
        let target = target_of(&child);
        let reused = KillTarget {
            start_time: target.start_time - 1,
            ..target.clone()
        };
        assert!(Held::open(&reused).unwrap().is_none());
        assert!(
            child.try_wait().unwrap().is_none(),
            "the child must still run"
        );

        let held = Held::open(&target).unwrap().unwrap();
        assert_eq!(held.signal(libc::SIGTERM), Outcome::Signalled);
        let status = child.wait().unwrap();
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&status),
            Some(libc::SIGTERM)
        );
        assert!(held.has_exited());
    }
}
