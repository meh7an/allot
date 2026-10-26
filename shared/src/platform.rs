//! What differs between operating systems: executable names, the processes Claude Code and
//! consoles show up as, where the home directory comes from, and how paths compare.

/// `name` with the executable suffix of the target OS.
macro_rules! exe {
    ($name:literal) => {
        if cfg!(windows) {
            concat!($name, ".exe")
        } else {
            $name
        }
    };
}

/// Suffix of executable file names: `.exe` on Windows, empty elsewhere.
pub const EXE_SUFFIX: &str = std::env::consts::EXE_SUFFIX;

/// The `allot` CLI's executable.
pub const CLI_EXE: &str = exe!("allot");

/// The daemon's executable.
pub const DAEMON_EXE: &str = exe!("allotd");

/// The MCP server's executable.
pub const MCP_EXE: &str = exe!("allot-mcp");

/// The coordinator's own executables, which binding skips and a kill never touches.
pub const ALLOT_BINARIES: [&str; 3] = [CLI_EXE, DAEMON_EXE, MCP_EXE];

/// Process names of a Claude Code session, the parent of its commands and MCP servers.
pub const SESSION_ROOTS: &[&str] = &[exe!("claude")];

/// Console hosts the OS starts beside console programs; never a command itself.
pub const CONSOLE_HOSTS: &[&str] = if cfg!(windows) { &["conhost.exe"] } else { &[] };

/// The environment variable that holds the user's home directory.
pub const HOME_VAR: &str = if cfg!(windows) { "USERPROFILE" } else { "HOME" };

/// Whether two paths that differ only in letter case name the same file: true on Windows and
/// on macOS, whose default APFS volumes ignore case, false on Linux.
pub const CASE_INSENSITIVE_PATHS: bool = cfg!(any(windows, target_os = "macos"));
