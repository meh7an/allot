//! Starting `allotd` as a detached background process.

#[cfg(unix)]
use std::thread;
use std::{
    env,
    ffi::OsStr,
    io,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

use allot_shared::platform::DAEMON_EXE;

const DEFAULT_READY_TIMEOUT: Duration = Duration::from_secs(5);
/// Set to `1` by Claude Code inside its Bash sandbox.
const SANDBOX_VAR: &str = "SANDBOX_RUNTIME";

// Win32 process creation flags. DETACHED_PROCESS gives allotd no console, so closing the
// terminal that started it does not stop it; a new process group keeps Ctrl+C in that
// terminal from reaching it.
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// How the client starts the daemon when it is not running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnOptions {
    /// The `allotd` executable.
    pub daemon_exe: PathBuf,
    /// How long to wait for `/health` to answer after starting it.
    pub ready_timeout: Duration,
}

impl SpawnOptions {
    /// `allotd` from the directory of the running executable, where the install puts both
    /// binaries; plain `allotd` (found on `PATH`) when that directory has none.
    pub fn beside_current_exe() -> Self {
        let beside = env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(DAEMON_EXE)))
            .filter(|path| path.is_file());
        Self::at(beside.unwrap_or_else(|| PathBuf::from(DAEMON_EXE)))
    }

    /// Starts `daemon_exe`, with the default ready timeout.
    pub fn at(daemon_exe: PathBuf) -> Self {
        Self {
            daemon_exe,
            ready_timeout: DEFAULT_READY_TIMEOUT,
        }
    }

    /// Starts the daemon without waiting for it; it outlives the caller.
    pub(crate) fn spawn_detached(&self) -> io::Result<()> {
        let mut command = Command::new(&self.daemon_exe);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
            command.spawn().map(drop)
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // A session of its own: allot-mcp runs in the process group and session of its
            // `claude`, so Ctrl+C there, a closed terminal, or the desktop app ending its
            // group would otherwise stop allotd too. The root directory keeps no project
            // directory busy.
            command.current_dir("/");
            // SAFETY: the closure runs in the forked child before exec and only calls
            // `setsid`, which is async-signal-safe.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let mut child = command.spawn()?;
            // allot-mcp lives as long as its session, so an allotd that stops before it must
            // not stay behind as a zombie.
            thread::Builder::new()
                .name("allotd-reaper".into())
                .spawn(move || {
                    let _ = child.wait();
                })
                .map(drop)
        }
    }
}

/// Whether this process runs inside Claude Code's sandbox, which has no network at all, so
/// an allotd it started could never be reached. Hooks and MCP servers run outside it.
pub(crate) fn in_claude_sandbox() -> bool {
    sandbox_flag(env::var_os(SANDBOX_VAR).as_deref())
}

fn sandbox_flag(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_flag_is_set_only_by_one() {
        assert!(sandbox_flag(Some(OsStr::new("1"))));
        assert!(!sandbox_flag(Some(OsStr::new("0"))));
        assert!(!sandbox_flag(Some(OsStr::new(""))));
        assert!(!sandbox_flag(None));
    }
}
