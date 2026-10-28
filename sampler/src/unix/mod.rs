//! Pieces shared by the Linux and macOS backends: the tree kill.

mod kill;

pub use crate::unix::kill::kill_tree;
