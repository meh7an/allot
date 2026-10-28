//! What lease scopes read and do on the machine itself: procfs, the scope's cgroup files, and
//! the anchor process. Behind a trait so tests can fake the machine.

use std::{fmt, io};

use allot_shared::types::EnforcedLimits;

/// A process as procfs shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcInfo {
    /// The process name, at most 15 bytes.
    pub(crate) comm: String,
    /// The session the process belongs to.
    pub(crate) session: u32,
}

/// What a scope's cgroup has used so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CgroupReading {
    /// CPU time of every process that ran in it.
    pub(crate) cpu_usage_usec: u64,
    /// Memory charged to it now, without the file cache.
    pub(crate) memory_bytes: u64,
}

/// The machine a lease scope lives on.
pub(crate) trait Host: Send + Sync + fmt::Debug {
    /// The direct children of every thread of `pid`.
    fn children(&self, pid: u32) -> Vec<u32>;

    /// The name and session of `pid`; `None` once it exited.
    fn process(&self, pid: u32) -> Option<ProcInfo>;

    /// The cgroup of `pid`; `None` once it exited.
    fn cgroup_of(&self, pid: u32) -> Option<String>;

    /// The usage of `cgroup`.
    ///
    /// # Errors
    ///
    /// Fails when the cgroup is gone or its files cannot be read.
    fn usage(&self, cgroup: &str) -> io::Result<CgroupReading>;

    /// The CPU and memory caps the kernel enforces on `cgroup`.
    fn limits(&self, cgroup: &str) -> EnforcedLimits;

    /// Every process in `cgroup` and below it.
    fn procs(&self, cgroup: &str) -> Vec<u32>;

    /// The arguments `pid` was started with; `None` once it exited.
    fn cmdline(&self, pid: u32) -> Option<Vec<String>>;

    /// Ends every process in `cgroup` at once.
    ///
    /// # Errors
    ///
    /// Fails when the cgroup is gone or the kernel cannot kill a cgroup.
    fn kill_cgroup(&self, cgroup: &str) -> io::Result<()>;

    /// Sends SIGKILL to `pid`.
    ///
    /// # Errors
    ///
    /// Fails when the process is gone.
    fn kill_process(&self, pid: u32) -> io::Result<()>;

    /// Starts an idle anchor process whose last argument is `scope`, and returns its PID.
    ///
    /// # Errors
    ///
    /// Fails when the process cannot start.
    fn spawn_anchor(&self, scope: &str) -> io::Result<u32>;
}

/// The anchor of `scope` among `pids`: the process whose last argument names the scope.
pub(crate) fn find_anchor(host: &dyn Host, pids: &[u32], scope: &str) -> Option<u32> {
    pids.iter().copied().find(|pid| {
        host.cmdline(*pid)
            .is_some_and(|args| args.last().is_some_and(|last| last == scope))
    })
}

#[cfg(target_os = "linux")]
pub(crate) use self::linux::LinuxHost;

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        fs, io,
        os::unix::process::CommandExt,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        thread,
    };

    use allot_sampler::cgroup;
    use allot_shared::types::EnforcedLimits;

    use super::{CgroupReading, Host, ProcInfo};

    const BYTES_PER_MB: u64 = 1024 * 1024;

    /// The real machine, with the anchor started as `program` followed by `args` and the
    /// scope name.
    #[derive(Debug)]
    pub(crate) struct LinuxHost {
        root: PathBuf,
        program: PathBuf,
        args: Vec<String>,
    }

    impl LinuxHost {
        /// The machine at `/`, with anchors run as `program args.. <scope>`.
        pub(crate) fn new(program: PathBuf, args: Vec<String>) -> Self {
            Self {
                root: PathBuf::from("/"),
                program,
                args,
            }
        }

        fn cgroup_file(&self, cgroup: &str, name: &str) -> Option<String> {
            let dir = self
                .root
                .join("sys/fs/cgroup")
                .join(cgroup.trim_start_matches('/'));
            fs::read_to_string(dir.join(name)).ok()
        }
    }

    impl Host for LinuxHost {
        fn children(&self, pid: u32) -> Vec<u32> {
            cgroup::children(&self.root, pid)
        }

        fn process(&self, pid: u32) -> Option<ProcInfo> {
            cgroup::stat(&self.root, pid).map(|stat| ProcInfo {
                comm: stat.comm,
                session: stat.session,
            })
        }

        fn cgroup_of(&self, pid: u32) -> Option<String> {
            cgroup::cgroup_of(&self.root, pid)
        }

        fn usage(&self, cgroup: &str) -> io::Result<CgroupReading> {
            let usage = cgroup::read_usage(&self.root, cgroup)?;
            Ok(CgroupReading {
                cpu_usage_usec: usage.cpu_usage_usec,
                memory_bytes: usage.memory_bytes,
            })
        }

        fn limits(&self, cgroup: &str) -> EnforcedLimits {
            let cpu_cores = self
                .cgroup_file(cgroup, "cpu.max")
                .and_then(|text| parse_cpu_max(&text));
            let memory_high_mb = self
                .cgroup_file(cgroup, "memory.high")
                .and_then(|text| text.trim().parse::<u64>().ok())
                .map(|bytes| bytes / BYTES_PER_MB);
            EnforcedLimits {
                cpu_cores,
                memory_high_mb,
                memory_max_mb: None,
            }
        }

        fn procs(&self, cgroup: &str) -> Vec<u32> {
            cgroup::procs(&self.root, cgroup)
        }

        fn cmdline(&self, pid: u32) -> Option<Vec<String>> {
            cgroup::cmdline(&self.root, pid)
        }

        fn kill_cgroup(&self, cgroup: &str) -> io::Result<()> {
            cgroup::kill(&self.root, cgroup)
        }

        fn kill_process(&self, pid: u32) -> io::Result<()> {
            cgroup::kill_process(pid)
        }

        fn spawn_anchor(&self, scope: &str) -> io::Result<u32> {
            let mut child = Command::new(&self.program)
                .args(&self.args)
                .arg(scope)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                // Its own process group, so Ctrl+C on an allotd run from a terminal leaves
                // the scope alone.
                .process_group(0)
                .current_dir(Path::new("/"))
                .spawn()?;
            let pid = child.id();
            // Reaps the anchor once a release or kill ends it; after a restart systemd does.
            thread::Builder::new()
                .name("allot-anchor".into())
                .spawn(move || {
                    let _ = child.wait();
                })?;
            Ok(pid)
        }
    }

    /// Cores from `cpu.max` (`<quota> <period>` in microseconds, or `max` uncapped).
    fn parse_cpu_max(text: &str) -> Option<f32> {
        let mut fields = text.split_whitespace();
        let quota: f64 = fields.next()?.parse().ok()?;
        let period: f64 = fields.next()?.parse().ok()?;
        (period > 0.0).then(|| (quota / period) as f32)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_parse_cpu_max_reads_cores_and_skips_max() {
            assert_eq!(parse_cpu_max("200000 100000\n"), Some(2.0));
            assert_eq!(parse_cpu_max("max 100000\n"), None);
        }
    }
}
