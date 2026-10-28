//! The running processes as sysinfo reports them on every OS: the process table a kill plans
//! against, and the current process's ancestry, which the CLI and MCP server use to find their
//! Claude session. Linux threads are left out; sysinfo would list each as a process.

use allot_core::{
    ancestry::{ProcessNode, ancestry},
    process::ProcessTable,
    sampler::RawProcess,
};
use allot_shared::{Error, Result, types::ResourceUsage};
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Every running process with its parent, name, and start time; no usage or exe paths.
pub fn process_table() -> ProcessTable {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    let processes: Vec<RawProcess> = system
        .processes()
        .values()
        .map(|process| RawProcess {
            pid: process.pid().as_u32(),
            parent_pid: process.parent().map(Pid::as_u32),
            name: process.name().to_string_lossy().into_owned(),
            exe: None,
            start_time: process.start_time(),
            usage: ResourceUsage::default(),
        })
        .collect();
    ProcessTable::new(&processes)
}

/// This process followed by its ancestors, nearest first. Under Claude Code the parent is
/// the session's `claude.exe`.
///
/// # Errors
///
/// Returns [`Error::Sampler`] when the OS does not report this process's PID.
pub fn current_ancestry() -> Result<Vec<ProcessNode>> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .without_tasks(),
    );
    let own_pid = sysinfo::get_current_pid().map_err(|error| Error::Sampler(error.into()))?;
    Ok(ancestry(own_pid.as_u32(), |pid| {
        system.process(Pid::from_u32(pid)).map(to_node)
    }))
}

fn to_node(process: &Process) -> ProcessNode {
    ProcessNode {
        pid: process.pid().as_u32(),
        parent_pid: process.parent().map(Pid::as_u32),
        name: process.name().to_string_lossy().into_owned(),
        exe: process.exe().map(|path| path.display().to_string()),
        start_time: process.start_time(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_table_lists_own_process_with_its_start_time() {
        let table = process_table();
        let own = table.get(std::process::id()).unwrap();
        assert!(own.start_time > 0);
        assert!(own.parent_pid.is_some());
    }

    #[test]
    fn test_current_ancestry_starts_at_own_process() {
        let chain = current_ancestry().unwrap();
        assert_eq!(chain[0].pid, std::process::id());
        assert!(
            chain.len() >= 2,
            "test runner should have a parent: {chain:?}"
        );
    }
}
