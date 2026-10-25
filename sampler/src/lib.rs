//! Machine and process sampling for Allot, with one backend per OS behind the same API:
//! [`SystemSampler`], [`process_table`], [`current_ancestry`], and [`kill_tree`].

mod cpu;
mod windows;

pub use crate::windows::*;
