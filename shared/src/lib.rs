//! Types shared by every crate in the allot workspace.

pub mod api;
pub mod autostart;
pub mod config;
#[cfg(test)]
mod contract;
mod error;
pub mod mutate;
pub mod paths;
pub mod platform;
#[cfg(test)]
mod test_support;
pub mod types;
pub mod ui;

pub use error::{Error, Result};
