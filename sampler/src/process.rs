//! The current process: its ancestors, read with sysinfo, and its standard handles.

use std::{io, os::windows::io::AsRawHandle};

use rh_core::ancestry::{ProcessNode, ancestry};
use rh_shared::{Error, Result};
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use windows::Win32::Foundation::{HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation};

/// Stops the processes this one starts from inheriting its standard input, output, and error.
/// Windows hands a child every inheritable handle, even when the child gets other standard
/// handles, so a long-lived child such as rhd would keep a log file or pipe this process
/// writes to open for as long as it runs. A child is still given the standard handles it is
/// started with.
pub fn keep_std_handles_private() {
    let handles = [
        io::stdin().as_raw_handle(),
        io::stdout().as_raw_handle(),
        io::stderr().as_raw_handle(),
    ];
    for handle in handles.into_iter().filter(|handle| !handle.is_null()) {
        // Fails only for a handle no child could inherit anyway, such as a console pseudo
        // handle.
        let _ =
            unsafe { SetHandleInformation(HANDLE(handle), HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) };
    }
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

    #[test]
    fn test_keep_std_handles_private_clears_the_inherit_flag() {
        use windows::Win32::Foundation::GetHandleInformation;

        keep_std_handles_private();

        let stdout = io::stdout().as_raw_handle();
        let mut flags = 0;
        // The test harness's output is a real handle (a pipe or a file), so its flags read.
        unsafe { GetHandleInformation(HANDLE(stdout), &mut flags) }.unwrap();
        assert_eq!(flags & HANDLE_FLAG_INHERIT.0, 0, "flags {flags:#x}");
    }
}
