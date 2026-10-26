//! Machine and process sampling for Allot, with one backend per OS behind the same API:
//! [`SystemSampler`], [`process_table`], [`current_ancestry`], and [`kill_tree`].

mod cpu;
mod table;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use crate::table::{current_ancestry, process_table};
#[cfg(unix)]
pub use crate::unix::{SystemSampler, kill_tree};
#[cfg(windows)]
pub use crate::windows::{SystemSampler, keep_std_handles_private, kill_tree};
