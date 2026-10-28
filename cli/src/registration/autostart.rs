//! Starting allotd at login, the same steps on every OS: the scheduled task on Windows
//! ([`task`](super::task)), a systemd user unit on Linux ([`systemd`](super::systemd)), and a
//! launchd LaunchAgent on macOS ([`launchd`](super::launchd)). Each backend offers the
//! functions re-exported here.

use std::path::Path;

#[cfg(target_os = "macos")]
pub(super) use super::launchd::{manual_remove, manual_set_up, remove, set_up, title};
#[cfg(target_os = "linux")]
pub(super) use super::systemd::{manual_remove, manual_set_up, remove, set_up, title};
#[cfg(windows)]
pub(super) use super::task::{manual_remove, manual_set_up, remove, set_up, title};
use crate::registered::same_path;

/// What the OS knows about an autostart entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Found {
    Missing,
    /// It exists and runs this command.
    Runs(String),
    /// The OS could not be asked, or answered something else.
    Unknown,
}

impl Found {
    /// Whether the entry exists and starts `allotd`.
    pub(super) fn starts(&self, allotd: &Path) -> bool {
        matches!(self, Self::Runs(command) if same_path(command, allotd))
    }

    /// The note for an entry that was set up in place of this one: what it started before,
    /// when that was something else.
    pub(super) fn replaced_note(&self, allotd: &Path, set_up: &str) -> String {
        match self {
            Self::Runs(other) if !same_path(other, allotd) => {
                format!("{set_up}; it started {other} before")
            }
            _ => set_up.to_owned(),
        }
    }
}
