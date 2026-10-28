//! Machine memory from the Mach VM statistics, counted the way Activity Monitor does, and the
//! kernel's memory pressure level.

use allot_shared::types::MemStats;

const BYTES_PER_MB: u64 = 1024 * 1024;

/// The VM statistics the sampler uses, with the RAM and swap sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Memory {
    pub(crate) total_bytes: u64,
    pub(crate) page_bytes: u64,
    /// Anonymous pages, purgeable ones included.
    pub(crate) internal_pages: u64,
    pub(crate) purgeable_pages: u64,
    pub(crate) wired_pages: u64,
    /// Pages holding compressed memory.
    pub(crate) compressor_pages: u64,
    pub(crate) swap_total_bytes: u64,
    pub(crate) swap_used_bytes: u64,
}

impl Memory {
    /// Memory in use is app memory (anonymous, not purgeable) plus wired plus compressed,
    /// as Activity Monitor's "Memory Used"; file cache and purgeable memory count as
    /// available. macOS has no commit charge, so memory in use plus swap in use stands in
    /// for it, out of memory plus swap.
    pub(crate) fn mem_stats(&self) -> MemStats {
        let mb = |pages: u64| pages.saturating_mul(self.page_bytes) / BYTES_PER_MB;
        let app_pages = self.internal_pages.saturating_sub(self.purgeable_pages);
        let total_mb = self.total_bytes / BYTES_PER_MB;
        let used_mb = mb(app_pages + self.wired_pages + self.compressor_pages).min(total_mb);
        MemStats {
            total_mb,
            used_mb,
            available_mb: total_mb - used_mb,
            commit_used_mb: used_mb + self.swap_used_bytes / BYTES_PER_MB,
            commit_limit_mb: total_mb + self.swap_total_bytes / BYTES_PER_MB,
        }
    }
}

/// How short of memory the kernel considers the machine, from
/// `kern.memorystatus_vm_pressure_level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PressureLevel {
    Normal,
    /// The kernel asks processes to free memory.
    Warning,
    /// The kernel is about to end processes to free memory.
    Critical,
}

impl PressureLevel {
    /// The level the sysctl reports: 1, 2, or 4.
    pub(crate) fn from_sysctl(level: i64) -> Option<Self> {
        match level {
            1 => Some(Self::Normal),
            2 => Some(Self::Warning),
            4 => Some(Self::Critical),
            _ => None,
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use live::read;

#[cfg(target_os = "macos")]
mod live {
    // libc marks the Mach host calls deprecated in favour of the mach2 crate; they are the
    // same calls, and one more dependency for two of them is not worth it.
    #![allow(deprecated)]

    use std::{
        ffi::{CStr, c_void},
        io,
        mem::{self, MaybeUninit},
        sync::OnceLock,
    };

    use super::{Memory, PressureLevel};

    /// The machine's memory and swap right now.
    pub(crate) fn read() -> io::Result<Memory> {
        let vm = vm_statistics()?;
        let swap: libc::xsw_usage = sysctl(c"vm.swapusage")?;
        // SAFETY: sysconf has no preconditions.
        let page_bytes = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        Ok(Memory {
            total_bytes: sysctl(c"hw.memsize")?,
            page_bytes: u64::try_from(page_bytes).map_err(|_| io::Error::last_os_error())?,
            internal_pages: u64::from(vm.internal_page_count),
            purgeable_pages: u64::from(vm.purgeable_count),
            wired_pages: u64::from(vm.wire_count),
            compressor_pages: u64::from(vm.compressor_page_count),
            swap_total_bytes: swap.xsu_total,
            swap_used_bytes: swap.xsu_used,
        })
    }

    impl PressureLevel {
        /// The current level; `None` when the sysctl is missing or reports something else.
        pub(crate) fn read() -> Option<Self> {
            let level: i32 = sysctl(c"kern.memorystatus_vm_pressure_level").ok()?;
            Self::from_sysctl(i64::from(level))
        }
    }

    fn vm_statistics() -> io::Result<libc::vm_statistics64> {
        let mut vm = MaybeUninit::<libc::vm_statistics64>::zeroed();
        let mut count = (mem::size_of::<libc::vm_statistics64>() / mem::size_of::<i32>())
            as libc::mach_msg_type_number_t;
        // SAFETY: `vm` is writable for `count` integers, the size of the structure, and both
        // pointers outlive the call.
        let result = unsafe {
            libc::host_statistics64(
                host(),
                libc::HOST_VM_INFO64,
                vm.as_mut_ptr().cast(),
                &mut count,
            )
        };
        if result != libc::KERN_SUCCESS {
            return Err(io::Error::other(format!(
                "host_statistics64 failed with {result}"
            )));
        }
        // SAFETY: the call succeeded and filled the structure, which started zeroed anyway.
        Ok(unsafe { vm.assume_init() })
    }

    /// The host port. Every `mach_host_self` call adds a reference to it, so it is taken once.
    fn host() -> libc::mach_port_t {
        static HOST: OnceLock<libc::mach_port_t> = OnceLock::new();
        // SAFETY: mach_host_self has no preconditions.
        *HOST.get_or_init(|| unsafe { libc::mach_host_self() })
    }

    /// A sysctl whose value is a plain `T`.
    fn sysctl<T: Copy>(name: &CStr) -> io::Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let mut size = mem::size_of::<T>();
        // SAFETY: `value` is writable for `size` bytes, and every pointer outlives the call.
        let result = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                value.as_mut_ptr().cast::<c_void>(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        if size != mem::size_of::<T>() {
            return Err(io::Error::other(format!(
                "{} has {size} bytes, not {}",
                name.to_string_lossy(),
                mem::size_of::<T>()
            )));
        }
        // SAFETY: the kernel wrote exactly `size_of::<T>()` bytes, and every T used here is
        // plain data that any bytes make valid.
        Ok(unsafe { value.assume_init() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pages per MB with royalmac's 16 KiB pages.
    const PAGES_PER_MB: u64 = 64;

    /// royalmac in the M2.5 re-check: internal 8070 MB, purgeable 130, wired 18518,
    /// compressor 6009; Activity Monitor's figure was 32467 MB used.
    fn royalmac() -> Memory {
        Memory {
            total_bytes: 38_654_705_664,
            page_bytes: 16_384,
            internal_pages: 8_070 * PAGES_PER_MB,
            purgeable_pages: 130 * PAGES_PER_MB,
            wired_pages: 18_518 * PAGES_PER_MB,
            compressor_pages: 6_009 * PAGES_PER_MB,
            swap_total_bytes: 8_192 * BYTES_PER_MB,
            swap_used_bytes: 7_279 * BYTES_PER_MB,
        }
    }

    #[test]
    fn test_mem_stats_counts_app_wired_and_compressed_memory_as_used() {
        let stats = royalmac().mem_stats();
        assert_eq!(stats.total_mb, 36_864);
        assert_eq!(stats.used_mb, 32_467);
        assert_eq!(stats.available_mb, 4_397);
        assert_eq!(stats.commit_used_mb, 32_467 + 7_279);
        assert_eq!(stats.commit_limit_mb, 36_864 + 8_192);
    }

    #[test]
    fn test_mem_stats_never_reports_more_used_than_installed() {
        let memory = Memory {
            wired_pages: 30_000 * PAGES_PER_MB,
            ..royalmac()
        };
        let stats = memory.mem_stats();
        assert_eq!(stats.used_mb, stats.total_mb);
        assert_eq!(stats.available_mb, 0);
    }

    #[test]
    fn test_pressure_level_from_sysctl_knows_the_three_levels() {
        assert_eq!(PressureLevel::from_sysctl(1), Some(PressureLevel::Normal));
        assert_eq!(PressureLevel::from_sysctl(2), Some(PressureLevel::Warning));
        assert_eq!(PressureLevel::from_sysctl(4), Some(PressureLevel::Critical));
        assert_eq!(PressureLevel::from_sysctl(0), None);
        assert_eq!(PressureLevel::from_sysctl(3), None);
    }
}
