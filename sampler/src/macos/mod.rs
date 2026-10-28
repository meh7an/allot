//! macOS implementation of [`allot_core::sampler::Sampler`]: sysinfo for CPU, processes,
//! and exe paths; `proc_pid_rusage` for each process's footprint; the Mach VM statistics and
//! sysctls for memory and pressure; IOKit for GPU use and disk I/O; and the mount list with
//! statfs for drives. The parsers work on plain values, so fixtures in `fixtures/macos`
//! stand in for a Mac.

// Off macOS only the fixture tests use the parsers, and the live sampler is not built.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

mod drives;
mod gpu;
#[cfg(target_os = "macos")]
mod iokit;
mod memory;
#[cfg(target_os = "macos")]
mod system;
mod value;

#[cfg(target_os = "macos")]
pub use crate::macos::{memory::PressureLevel, system::SystemSampler};

#[cfg(test)]
pub(crate) mod tests {
    use std::path::{Path, PathBuf};

    use crate::macos::value::Entry;

    /// The directory of a fixture Mac in `fixtures/macos`.
    pub(crate) fn fixture(case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/macos")
            .join(case)
    }

    pub(crate) fn fixture_json(case: &str, file: &str) -> serde_json::Value {
        let path = fixture(case).join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
        serde_json::from_str(&text).unwrap()
    }

    /// The registry entries of an `ioreg -a` capture, saved as JSON.
    pub(crate) fn ioreg_fixture(case: &str, file: &str) -> Vec<Entry> {
        let json = fixture_json(case, file);
        json.as_array()
            .expect("ioreg -a writes an array of entries")
            .iter()
            .map(Entry::from_ioreg)
            .collect()
    }
}
