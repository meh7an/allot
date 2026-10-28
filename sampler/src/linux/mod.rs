//! Linux implementation of [`allot_core::sampler::Sampler`]: sysinfo for CPU, processes,
//! and exe paths; `/proc/meminfo` for memory; the mount table, statvfs, and
//! `/proc/diskstats` for drives; `/proc/pressure` for stall information. The parsers read
//! below a root path, normally `/`, so fixtures in `fixtures/linux` stand in for a machine.

// Off Linux only the fixture tests use the parsers, and the live sampler is not built.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod diskstats;
mod drives;
mod meminfo;
mod mounts;
mod pressure;
#[cfg(target_os = "linux")]
mod system;

#[cfg(target_os = "linux")]
pub use crate::linux::{
    pressure::{Pressure, PressureLine, Stall},
    system::SystemSampler,
};

#[cfg(test)]
pub(crate) mod tests {
    use std::path::{Path, PathBuf};

    /// The root of a fixture machine in `fixtures/linux`.
    pub(crate) fn fixture(case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/linux")
            .join(case)
    }
}
