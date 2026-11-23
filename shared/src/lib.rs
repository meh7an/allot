//! Types shared by every crate in the allot workspace.

pub mod api;
pub mod atomic_file;
pub mod autostart;
pub mod config;
pub mod config_write;
#[cfg(test)]
mod contract;
mod error;
pub mod explain;
pub mod mutate;
pub mod paths;
pub mod platform;
pub mod release;
pub mod settings;
pub mod stop;
#[cfg(test)]
mod test_support;
pub mod types;
pub mod ui;
pub mod update;

pub use error::{Error, Result};
