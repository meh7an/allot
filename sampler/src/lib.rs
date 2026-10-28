//! Machine and process sampling for Allot, with one backend per OS behind the same API:
//! [`SystemSampler`], [`process_table`], [`current_ancestry`], and [`kill_tree`].

#[cfg(any(windows, target_os = "linux", target_os = "macos", test))]
mod cadence;
mod cpu;
// The Linux and macOS parsers take plain data, so their fixture tests run on every OS.
#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(any(target_os = "macos", test))]
mod macos;
#[cfg(any(target_os = "linux", target_os = "macos", test))]
mod rates;
mod table;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use crate::linux::{Pressure, PressureLine, Stall, SystemSampler, cgroup};
#[cfg(target_os = "macos")]
pub use crate::macos::{PressureLevel, SystemSampler};
pub use crate::table::{current_ancestry, process_table};
#[cfg(unix)]
pub use crate::unix::kill_tree;
#[cfg(windows)]
pub use crate::windows::{SystemSampler, keep_std_handles_private, kill_tree};
