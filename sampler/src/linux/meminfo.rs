//! Machine memory from `/proc/meminfo`.

use std::{fs, io, path::Path};

use allot_shared::types::MemStats;

const KIB_PER_MB: u64 = 1024;

/// The `/proc/meminfo` fields the sampler uses, in KiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct MemInfo {
    total_kib: u64,
    available_kib: u64,
    swap_total_kib: u64,
    swap_free_kib: u64,
}

impl MemInfo {
    /// Reads `proc/meminfo` below `root`.
    pub(crate) fn read(root: &Path) -> io::Result<Self> {
        let text = fs::read_to_string(root.join("proc/meminfo"))?;
        Self::parse(&text).ok_or_else(|| io::Error::other("/proc/meminfo has no MemTotal"))
    }

    /// Parses the text of `/proc/meminfo`. `None` when `MemTotal` is missing.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let mut total = None;
        let mut available = None;
        let mut free = 0;
        let mut info = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let Some(kib) = value.split_whitespace().next().and_then(|v| v.parse().ok()) else {
                continue;
            };
            match key {
                "MemTotal" => total = Some(kib),
                "MemAvailable" => available = Some(kib),
                "MemFree" => free = kib,
                "SwapTotal" => info.swap_total_kib = kib,
                "SwapFree" => info.swap_free_kib = kib,
                _ => {}
            }
        }
        info.total_kib = total?;
        // Kernels before 3.14 have no MemAvailable; free memory is the safe lower bound.
        info.available_kib = available.unwrap_or(free).min(info.total_kib);
        Some(info)
    }

    /// Memory in use is what is not available. Linux has no commit charge, so memory in use
    /// plus swap in use stands in for it, out of memory plus swap.
    pub(crate) fn mem_stats(&self) -> MemStats {
        let total_mb = self.total_kib / KIB_PER_MB;
        let available_mb = self.available_kib / KIB_PER_MB;
        let used_mb = total_mb - available_mb;
        let swap_total_mb = self.swap_total_kib / KIB_PER_MB;
        let swap_used_mb = self.swap_total_kib.saturating_sub(self.swap_free_kib) / KIB_PER_MB;
        MemStats {
            total_mb,
            used_mb,
            available_mb,
            commit_used_mb: used_mb + swap_used_mb,
            commit_limit_mb: total_mb + swap_total_mb,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::tests::fixture;

    #[test]
    fn test_mem_info_read_desktop_fixture_gives_mem_stats() {
        let info = MemInfo::read(&fixture("desktop")).unwrap();
        assert_eq!(
            info.mem_stats(),
            MemStats {
                total_mb: 31_999,
                used_mb: 12_475,
                available_mb: 19_524,
                commit_used_mb: 12_475 + 1_024,
                commit_limit_mb: 31_999 + 16_383,
            }
        );
    }

    #[test]
    fn test_mem_info_read_wsl_fixture_without_swap_in_use_commits_memory_only() {
        let stats = MemInfo::read(&fixture("wsl")).unwrap().mem_stats();
        assert_eq!((stats.total_mb, stats.used_mb), (15_921, 1_749));
        assert_eq!(
            (stats.commit_used_mb, stats.commit_limit_mb),
            (1_749, 15_921 + 4_096)
        );
    }

    #[test]
    fn test_mem_info_parse_without_mem_available_uses_mem_free() {
        let info = MemInfo::parse("MemTotal: 4096000 kB\nMemFree: 1024000 kB\n").unwrap();
        let stats = info.mem_stats();
        assert_eq!((stats.total_mb, stats.available_mb), (4_000, 1_000));
        assert_eq!(
            (stats.commit_used_mb, stats.commit_limit_mb),
            (3_000, 4_000)
        );
    }

    #[test]
    fn test_mem_info_parse_without_mem_total_is_none() {
        assert_eq!(MemInfo::parse("MemFree: 1024 kB\nnot a line\n"), None);
    }

    #[test]
    fn test_mem_info_read_missing_file_fails() {
        assert!(MemInfo::read(&fixture("missing")).is_err());
    }
}
