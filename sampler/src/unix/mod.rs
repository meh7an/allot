//! Pieces shared by the Linux and macOS backends.

mod kill;

pub use crate::unix::kill::kill_tree;
