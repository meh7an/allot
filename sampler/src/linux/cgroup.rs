//! A lease scope's cgroup v2 files, and the procfs details allotd reads to move a command's
//! processes into the scope. Everything reads below a root path, normally `/`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// What a cgroup has used so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CgroupUsage {
    /// CPU time of every process that ran in it, exited ones included.
    pub cpu_usage_usec: u64,
    /// Memory charged to it now without the file cache, which a build writing its output
    /// fills without needing it.
    pub memory_bytes: u64,
}

/// A process as `/proc/<pid>/stat` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcStat {
    /// The process name, at most 15 bytes.
    pub comm: String,
    /// The session the process belongs to.
    pub session: u32,
}

/// Whether the unified cgroup v2 hierarchy is mounted at `sys/fs/cgroup` below `root`.
pub fn is_cgroup2(root: &Path) -> bool {
    root.join("sys/fs/cgroup/cgroup.controllers").is_file()
}

/// Reads the usage of `cgroup`, a path like `/user.slice/.../app.slice/x.scope`.
///
/// # Errors
///
/// Fails when the cgroup is gone or its files cannot be parsed.
pub fn read_usage(root: &Path, cgroup: &str) -> io::Result<CgroupUsage> {
    let dir = cgroup_dir(root, cgroup);
    let cpu = fs::read_to_string(dir.join("cpu.stat"))?;
    let current = fs::read_to_string(dir.join("memory.current"))?;
    let memory = fs::read_to_string(dir.join("memory.stat"))?;
    let cpu_usage_usec =
        stat_field(&cpu, "usage_usec").ok_or_else(|| invalid("cpu.stat has no usage_usec"))?;
    let current: u64 = current
        .trim()
        .parse()
        .map_err(|_| invalid("memory.current is not a number"))?;
    let file = stat_field(&memory, "file").unwrap_or_default();
    Ok(CgroupUsage {
        cpu_usage_usec,
        memory_bytes: current.saturating_sub(file),
    })
}

/// Every process in `cgroup` and the cgroups below it; empty once it is gone.
pub fn procs(root: &Path, cgroup: &str) -> Vec<u32> {
    let mut pids = Vec::new();
    let mut pending = vec![cgroup_dir(root, cgroup)];
    while let Some(dir) = pending.pop() {
        if let Ok(text) = fs::read_to_string(dir.join("cgroup.procs")) {
            pids.extend(
                text.split_whitespace()
                    .filter_map(|pid| pid.parse::<u32>().ok()),
            );
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        pending.extend(
            entries
                .flatten()
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|entry| entry.path()),
        );
    }
    pids
}

/// Ends every process in `cgroup` and below it at once (kernel 5.14 and later).
///
/// # Errors
///
/// Fails when the cgroup is gone or the kernel has no `cgroup.kill`.
pub fn kill(root: &Path, cgroup: &str) -> io::Result<()> {
    fs::write(cgroup_dir(root, cgroup).join("cgroup.kill"), "1")
}

/// The cgroup v2 path of `pid`; `None` once it exited.
pub fn cgroup_of(root: &Path, pid: u32) -> Option<String> {
    let text = fs::read_to_string(proc_dir(root, pid).join("cgroup")).ok()?;
    parse_cgroup(&text)
}

/// The direct children of every thread of `pid`.
pub fn children(root: &Path, pid: u32) -> Vec<u32> {
    let Ok(tasks) = fs::read_dir(proc_dir(root, pid).join("task")) else {
        return Vec::new();
    };
    tasks
        .flatten()
        .filter_map(|task| fs::read_to_string(task.path().join("children")).ok())
        .flat_map(|text| {
            text.split_whitespace()
                .filter_map(|child| child.parse().ok())
                .collect::<Vec<u32>>()
        })
        .collect()
}

/// The name and session of `pid`; `None` once it exited.
pub fn stat(root: &Path, pid: u32) -> Option<ProcStat> {
    let text = fs::read_to_string(proc_dir(root, pid).join("stat")).ok()?;
    parse_stat(&text)
}

/// The arguments `pid` was started with; `None` once it exited.
pub fn cmdline(root: &Path, pid: u32) -> Option<Vec<String>> {
    let bytes = fs::read(proc_dir(root, pid).join("cmdline")).ok()?;
    Some(
        bytes
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .map(|arg| String::from_utf8_lossy(arg).into_owned())
            .collect(),
    )
}

/// Sends SIGKILL to `pid`.
///
/// # Errors
///
/// Fails when the process is gone or belongs to another user.
#[cfg(target_os = "linux")]
pub fn kill_process(pid: u32) -> io::Result<()> {
    let pid = libc::pid_t::try_from(pid).map_err(|_| invalid("PID out of range"))?;
    // SAFETY: kill takes plain integers and touches no memory of this process.
    if unsafe { libc::kill(pid, libc::SIGKILL) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn cgroup_dir(root: &Path, cgroup: &str) -> PathBuf {
    root.join("sys/fs/cgroup")
        .join(cgroup.trim_start_matches('/'))
}

fn proc_dir(root: &Path, pid: u32) -> PathBuf {
    root.join("proc").join(pid.to_string())
}

/// The value of `key` in a flat keyed file such as `cpu.stat` or `memory.stat`.
fn stat_field(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once(' ')?;
        (name == key).then(|| value.trim().parse().ok()).flatten()
    })
}

/// The path on the `0::` line of `/proc/<pid>/cgroup`, which cgroup v2 writes.
fn parse_cgroup(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("0::"))
        .map(str::to_owned)
}

/// Parses `/proc/<pid>/stat`. The name sits in parentheses and may hold spaces and
/// parentheses itself, so the fields after it are counted from the last `)`.
fn parse_stat(text: &str) -> Option<ProcStat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let comm = text.get(open + 1..close)?.to_owned();
    // state, ppid, pgrp, session
    let session = text
        .get(close + 1..)?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()?;
    Some(ProcStat { comm, session })
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::tests::fixture;

    const SCOPE: &str = concat!(
        "/user.slice/user-1000.slice/user@1000.service/app.slice/",
        "allot-7-0a1b2c3d.scope"
    );

    #[test]
    fn test_read_usage_leaves_out_the_file_cache() {
        let usage = read_usage(&fixture("scope"), SCOPE).unwrap();
        assert_eq!(usage.cpu_usage_usec, 1_036_023);
        // 71245824 charged, 2007040 of it file cache.
        assert_eq!(usage.memory_bytes, 69_238_784);
    }

    #[test]
    fn test_read_usage_fails_for_a_missing_cgroup() {
        assert!(read_usage(&fixture("scope"), "/app.slice/gone.scope").is_err());
    }

    #[test]
    fn test_procs_includes_child_cgroups() {
        let mut pids = procs(&fixture("scope"), SCOPE);
        pids.sort_unstable();
        assert_eq!(pids, [4100, 4242, 4243, 4250]);
        assert!(procs(&fixture("scope"), "/app.slice/gone.scope").is_empty());
    }

    #[test]
    fn test_is_cgroup2_needs_the_unified_hierarchy() {
        assert!(is_cgroup2(&fixture("scope")));
        assert!(!is_cgroup2(&fixture("desktop")));
    }

    #[test]
    fn test_process_details_read_from_procfs() {
        let root = fixture("scope");
        let mut kids = children(&root, 4242);
        kids.sort_unstable();
        assert_eq!(kids, [4243, 4244, 4250]);
        assert_eq!(
            stat(&root, 4242),
            Some(ProcStat {
                comm: "zsh".into(),
                session: 4242
            })
        );
        assert_eq!(cgroup_of(&root, 4242).as_deref(), Some(SCOPE));
        assert_eq!(
            cmdline(&root, 4100).unwrap(),
            ["/usr/bin/allotd", "--anchor", "allot-7-0a1b2c3d.scope"]
        );
        assert_eq!(stat(&root, 9999), None);
        assert!(children(&root, 9999).is_empty());
    }

    #[test]
    fn test_parse_stat_counts_fields_from_the_last_parenthesis() {
        let text = "77 (a) (b c) S 1 70 71 0 -1 4194304 0 0 0 0 0 0 0 0 20 0 1 0 900 0 0";
        assert_eq!(
            parse_stat(text),
            Some(ProcStat {
                comm: "a) (b c".into(),
                session: 71
            })
        );
        assert_eq!(parse_stat("77 (a S 1"), None);
    }

    #[test]
    fn test_parse_cgroup_takes_the_unified_line() {
        let text = "1:name=systemd:/old\n0::/user.slice/session-2.scope\n";
        assert_eq!(
            parse_cgroup(text).as_deref(),
            Some("/user.slice/session-2.scope")
        );
        assert_eq!(parse_cgroup("1:name=systemd:/old\n"), None);
    }
}
