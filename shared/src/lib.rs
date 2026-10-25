//! Types shared by every crate in the allot workspace.

pub mod api;
pub mod config;
#[cfg(test)]
mod contract;
mod error;
pub mod mutate;
pub mod paths;
#[cfg(test)]
mod test_support;
pub mod types;
pub mod ui;

pub use error::{Error, Result};
