//! The current process's standard handles.

use std::{io, os::windows::io::AsRawHandle};

use windows::Win32::Foundation::{HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation};

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
}
