//! The current process's standard handles, and process start times read on a handle.

use std::{io, os::windows::io::AsRawHandle};

use windows::Win32::{
    Foundation::{
        CloseHandle, FILETIME, HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation,
    },
    System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
};

/// FILETIME ticks per second.
const TICKS_PER_SEC: u64 = 10_000_000;

/// Seconds from the FILETIME epoch (1601) to the Unix epoch.
const FILETIME_TO_UNIX_SECS: u64 = 11_644_473_600;

/// The process's start time in whole seconds since the Unix epoch, rounded down as sysinfo
/// does, so it compares equal to the process table's.
pub(crate) fn start_time(handle: HANDLE) -> windows::core::Result<u64> {
    let mut creation = FILETIME::default();
    let mut unused = [FILETIME::default(); 3];
    let [exit, kernel, user] = &mut unused;
    // SAFETY: `handle` has limited query access and every pointer is a valid FILETIME.
    unsafe { GetProcessTimes(handle, &mut creation, exit, kernel, user) }?;
    let ticks = (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
    Ok((ticks / TICKS_PER_SEC).saturating_sub(FILETIME_TO_UNIX_SECS))
}

/// The start time of the process `pid` as [`start_time`] reads it, or `None` when it cannot
/// be opened.
pub(crate) fn start_time_of(pid: u32) -> Option<u64> {
    // SAFETY: plain FFI call; the returned handle is closed below.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let read = start_time(handle);
    // SAFETY: `handle` came from `OpenProcess` and is closed exactly once.
    let _ = unsafe { CloseHandle(handle) };
    read.ok()
}

/// Stops the processes this one starts from inheriting its standard input, output, and error.
/// Windows hands a child every inheritable handle, even when the child gets other standard
/// handles, so a long-lived child such as allotd would keep a log file or pipe this process
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_start_time_of_own_process_matches_sysinfo() {
        let own = crate::table::process_table()
            .get(std::process::id())
            .unwrap()
            .start_time;
        assert_eq!(start_time_of(std::process::id()), Some(own));
    }

    #[test]
    fn test_start_time_of_idle_process_is_none() {
        assert_eq!(start_time_of(0), None);
    }
}
