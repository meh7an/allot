//! Pressure stall information (PSI) from `/proc/pressure`: the share of time tasks waited
//! for CPU, memory, or I/O.

use std::{fs, path::Path};

/// Stall time of one resource, as `/proc/pressure/<resource>` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PressureLine {
    /// Some tasks were stalled.
    pub some: Stall,
    /// All non-idle tasks were stalled at once. Kernels before 5.13 report no `full` line for
    /// CPU, and the machine-wide CPU `full` line reads zero.
    pub full: Option<Stall>,
}

/// One `some` or `full` line.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stall {
    /// Percentage of the last 10 seconds.
    pub avg10: f32,
    /// Percentage of the last 60 seconds.
    pub avg60: f32,
    /// Percentage of the last 300 seconds.
    pub avg300: f32,
    /// Total stall time since boot, in microseconds.
    pub total_us: u64,
}

/// CPU, memory, and I/O pressure. A resource is `None` when the kernel reports no PSI for it
/// (built without PSI, or booted with `psi=0`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pressure {
    /// Waiting for a CPU.
    pub cpu: Option<PressureLine>,
    /// Waiting for memory: reclaim, swap-in, or refaults.
    pub memory: Option<PressureLine>,
    /// Waiting for block I/O.
    pub io: Option<PressureLine>,
}

impl Pressure {
    /// Reads `proc/pressure/{cpu,memory,io}` below `root`.
    pub(crate) fn read(root: &Path) -> Self {
        let line = |resource: &str| {
            let text = fs::read_to_string(root.join("proc/pressure").join(resource)).ok()?;
            PressureLine::parse(&text)
        };
        Self {
            cpu: line("cpu"),
            memory: line("memory"),
            io: line("io"),
        }
    }
}

impl PressureLine {
    /// Parses one pressure file. `None` without a valid `some` line.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let mut some = None;
        let mut full = None;
        for line in text.lines() {
            let Some((kind, fields)) = line.split_once(' ') else {
                continue;
            };
            match kind {
                "some" => some = Stall::parse(fields),
                "full" => full = Stall::parse(fields),
                _ => {}
            }
        }
        Some(Self { some: some?, full })
    }
}

impl Stall {
    /// Parses `avg10=0.12 avg60=0.05 avg300=0.01 total=123456`.
    fn parse(fields: &str) -> Option<Self> {
        let mut avg10 = None;
        let mut avg60 = None;
        let mut avg300 = None;
        let mut total_us = None;
        for field in fields.split_whitespace() {
            let (key, value) = field.split_once('=')?;
            match key {
                "avg10" => avg10 = value.parse().ok(),
                "avg60" => avg60 = value.parse().ok(),
                "avg300" => avg300 = value.parse().ok(),
                "total" => total_us = value.parse().ok(),
                _ => {}
            }
        }
        Some(Self {
            avg10: avg10?,
            avg60: avg60?,
            avg300: avg300?,
            total_us: total_us?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::tests::fixture;

    #[test]
    fn test_pressure_read_desktop_fixture_reads_every_resource() {
        let pressure = Pressure::read(&fixture("desktop"));
        let memory = pressure.memory.unwrap();
        assert_eq!(
            memory.some,
            Stall {
                avg10: 12.5,
                avg60: 4.02,
                avg300: 0.87,
                total_us: 98_765_432,
            }
        );
        assert_eq!(memory.full.unwrap().avg10, 6.25);
        let cpu = pressure.cpu.unwrap();
        assert_eq!(cpu.some.avg60, 31.4);
        assert_eq!(cpu.full.unwrap().total_us, 0);
        assert_eq!(pressure.io.unwrap().some.total_us, 4_567_890);
    }

    #[test]
    fn test_pressure_read_without_psi_is_none_for_every_resource() {
        assert_eq!(Pressure::read(&fixture("wsl")), Pressure::default());
    }

    #[test]
    fn test_pressure_line_parse_without_full_line_keeps_some() {
        let line = PressureLine::parse("some avg10=1.00 avg60=2.00 avg300=3.00 total=4\n");
        let line = line.unwrap();
        assert_eq!(line.some.avg300, 3.0);
        assert_eq!(line.full, None);
    }

    #[test]
    fn test_pressure_line_parse_rejects_a_broken_some_line() {
        assert_eq!(PressureLine::parse("some avg10=1.00 avg60=x\n"), None);
        assert_eq!(PressureLine::parse(""), None);
    }
}
