//! The current process and its ancestors, read with sysinfo.

use rh_core::ancestry::{ProcessNode, ancestry};
use rh_shared::{Error, Result};
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

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
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
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
    fn test_current_ancestry_starts_at_own_process() {
        let chain = current_ancestry().unwrap();
        assert_eq!(chain[0].pid, std::process::id());
        assert!(
            chain.len() >= 2,
            "test runner should have a parent: {chain:?}"
        );
    }
}
