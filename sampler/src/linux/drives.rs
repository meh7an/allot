//! Fixed drives: filesystems on local, non-removable block devices, one per device, with
//! space from statvfs and read and write rates from `/proc/diskstats`.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use allot_shared::types::DriveUsage;

use crate::{
    linux::{diskstats::DiskCounters, mounts::Mount},
    rates::rate,
};

/// Prefixes of block devices that hold no disk: image files, compressed RAM, RAM disks,
/// network block devices, and optical drives.
const NOT_DISKS: [&str; 5] = ["/dev/loop", "/dev/zram", "/dev/ram", "/dev/nbd", "/dev/sr"];

/// Read-only image filesystems, which a block device can still hold.
const IMAGE_FILESYSTEMS: [&str; 3] = ["squashfs", "iso9660", "udf"];

/// A fixed drive and the device behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixedMount {
    /// The mount point, which names the drive.
    pub(crate) name: String,
    pub(crate) major: u32,
    pub(crate) minor: u32,
    /// Kernel name of the block device, for example `nvme0n1p2` or `dm-0`.
    pub(crate) device: String,
}

/// The mounts that are fixed drives, sorted by mount point. Mounts of the same device
/// (btrfs subvolumes, bind mounts) share their space, so each device is listed once, under
/// its shortest mount point.
pub(crate) fn fixed_mounts(root: &Path, mounts: &[Mount]) -> Vec<FixedMount> {
    let mut by_source: HashMap<&str, &Mount> = HashMap::new();
    for mount in mounts.iter().filter(|mount| is_disk(mount)) {
        let kept = by_source.entry(mount.source.as_str()).or_insert(mount);
        if shorter(&mount.mount_point, &kept.mount_point) {
            *kept = mount;
        }
    }
    let mut fixed: Vec<FixedMount> = by_source
        .into_values()
        .map(|mount| FixedMount {
            name: mount.mount_point.clone(),
            major: mount.major,
            minor: mount.minor,
            device: device_name(root, &mount.source),
        })
        .filter(|mount| !is_removable(root, &mount.device))
        .collect();
    fixed.sort_by(|a, b| a.name.cmp(&b.name));
    fixed
}

fn is_disk(mount: &Mount) -> bool {
    mount.source.starts_with("/dev/")
        && !NOT_DISKS
            .iter()
            .any(|prefix| mount.source.starts_with(prefix))
        && !IMAGE_FILESYSTEMS.contains(&mount.fs_type.as_str())
}

fn shorter(candidate: &str, kept: &str) -> bool {
    (candidate.len(), candidate) < (kept.len(), kept)
}

/// The kernel name of a mount source. `/dev/mapper/*` and `/dev/disk/by-*` are links to the
/// real node, so the link is followed when it exists below `root`.
fn device_name(root: &Path, source: &str) -> String {
    let node = root.join(source.trim_start_matches('/'));
    let resolved = fs::canonicalize(&node).unwrap_or(node);
    resolved
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| source.to_owned())
}

/// Whether `device`, or the disk it is a partition of, says it is removable (USB sticks and
/// card readers). `sys/block` holds every disk, with its partitions as subdirectories.
fn is_removable(root: &Path, device: &str) -> bool {
    let Ok(disks) = fs::read_dir(root.join("sys/block")) else {
        return false;
    };
    let disk: Option<PathBuf> = disks.flatten().map(|entry| entry.path()).find(|disk| {
        disk.file_name().is_some_and(|name| name == device) || disk.join(device).is_dir()
    });
    disk.and_then(|disk| fs::read_to_string(disk.join("removable")).ok())
        .is_some_and(|flag| flag.trim() == "1")
}

/// The counters of the device behind `mount`: by device number, which also finds the
/// device when the source is a name such as `/dev/root` that has no node, or else by name,
/// for btrfs, whose mounts carry an anonymous device number.
pub(crate) fn counters_of<'a>(
    mount: &FixedMount,
    counters: &'a [DiskCounters],
) -> Option<&'a DiskCounters> {
    let by_number = counters
        .iter()
        .find(|c| mount.major != 0 && (c.major, c.minor) == (mount.major, mount.minor));
    by_number.or_else(|| counters.iter().find(|c| c.name == mount.device))
}

/// Space of one fixed drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Space {
    pub(crate) total_mb: u64,
    pub(crate) used_mb: u64,
}

/// Joins each drive with its space and its rates since the previous reading.
pub(crate) fn usage(
    mounts: &[(FixedMount, Space)],
    before: &[DiskCounters],
    after: &[DiskCounters],
    elapsed_secs: f64,
) -> Vec<DriveUsage> {
    mounts
        .iter()
        .map(|(mount, space)| {
            let (read, write) = match (counters_of(mount, before), counters_of(mount, after)) {
                (Some(before), Some(after)) => (
                    rate(before.read_bytes, after.read_bytes, elapsed_secs),
                    rate(before.written_bytes, after.written_bytes, elapsed_secs),
                ),
                _ => (0, 0),
            };
            DriveUsage {
                name: mount.name.clone(),
                total_mb: space.total_mb,
                used_mb: space.used_mb,
                read_bytes_per_sec: read,
                write_bytes_per_sec: write,
            }
        })
        .collect()
}

#[cfg(target_os = "linux")]
pub(crate) use live::Drives;

#[cfg(target_os = "linux")]
mod live {
    use std::{
        ffi::CString,
        mem::MaybeUninit,
        os::unix::ffi::OsStrExt,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };

    use allot_shared::types::DriveUsage;

    use super::{FixedMount, Space, fixed_mounts, usage};
    use crate::{
        cadence::Cadence,
        linux::{
            diskstats::{self, DiskCounters},
            mounts,
        },
    };

    /// How often the mount table and free space are read again; space changes slowly.
    const SPACE_REFRESH: Duration = Duration::from_secs(10);

    const BYTES_PER_MB: u64 = 1024 * 1024;

    /// The fixed drives below a root, with the I/O counters of the previous reading.
    #[derive(Debug)]
    pub(crate) struct Drives {
        root: PathBuf,
        refresh: Cadence,
        mounts: Vec<(FixedMount, Space)>,
        before: Vec<DiskCounters>,
        after: Vec<DiskCounters>,
        elapsed_secs: f64,
        read_at: Option<Instant>,
    }

    impl Drives {
        pub(crate) fn new(root: &Path) -> Self {
            Self {
                root: root.to_owned(),
                refresh: Cadence::new(SPACE_REFRESH),
                mounts: Vec::new(),
                before: Vec::new(),
                after: Vec::new(),
                elapsed_secs: 0.0,
                read_at: None,
            }
        }

        /// Reads the I/O counters, and the drive list and space when they are due at `now`.
        /// Both are best effort: a drive whose space cannot be read is left out, and without
        /// `/proc/diskstats` drives read as idle.
        pub(crate) fn refresh(&mut self, now: Instant) {
            if self.refresh.due(now) {
                let mounts = mounts::read(&self.root).unwrap_or_default();
                self.mounts = fixed_mounts(&self.root, &mounts)
                    .into_iter()
                    .filter_map(|mount| {
                        let space = space(&self.root.join(mount.name.trim_start_matches('/')))?;
                        Some((mount, space))
                    })
                    .collect();
            }
            self.before = std::mem::take(&mut self.after);
            self.after = diskstats::read(&self.root).unwrap_or_default();
            self.elapsed_secs = self
                .read_at
                .map_or(0.0, |at| now.saturating_duration_since(at).as_secs_f64());
            self.read_at = Some(now);
        }

        /// Every fixed drive with its read and write rates since the previous refresh.
        pub(crate) fn read(&self) -> Vec<DriveUsage> {
            usage(&self.mounts, &self.before, &self.after, self.elapsed_secs)
        }
    }

    /// Size and use of the filesystem at `path`, the way `df` counts them: blocks reserved
    /// for root count as used.
    fn space(path: &Path) -> Option<Space> {
        let path = CString::new(path.as_os_str().as_bytes()).ok()?;
        let mut stats = MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: `path` is NUL-terminated and `stats` is writable; both outlive the call.
        if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: statvfs returned 0, so it filled `stats`.
        let stats = unsafe { stats.assume_init() };
        // The field types are 32 bits wide on some targets and 64 on others.
        #[allow(clippy::unnecessary_cast)]
        let (block, blocks, free) = (
            stats.f_frsize as u64,
            stats.f_blocks as u64,
            stats.f_bfree as u64,
        );
        Some(Space {
            total_mb: blocks * block / BYTES_PER_MB,
            used_mb: blocks.saturating_sub(free) * block / BYTES_PER_MB,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::{diskstats, mounts, tests::fixture};

    fn fixed(case: &str) -> Vec<FixedMount> {
        let root = fixture(case);
        fixed_mounts(&root, &mounts::read(&root).unwrap())
    }

    fn names(mounts: &[FixedMount]) -> Vec<&str> {
        mounts.iter().map(|mount| mount.name.as_str()).collect()
    }

    #[test]
    fn test_fixed_mounts_desktop_lists_each_local_disk_once() {
        let mounts = fixed("desktop");
        // btrfs subvolumes of nvme0n1p2 fold into `/`; loop, tmpfs, overlay, network, and
        // the removable USB stick are left out.
        assert_eq!(
            names(&mounts),
            ["/", "/boot/efi", "/mnt/my data", "/srv/vault"]
        );
        let root = &mounts[0];
        assert_eq!((root.major, root.device.as_str()), (0, "nvme0n1p2"));
        assert_eq!(mounts[3].device, "luks-3f2a");
    }

    #[test]
    fn test_fixed_mounts_wsl_keeps_the_root_disk_and_drops_windows_drives() {
        assert_eq!(names(&fixed("wsl")), ["/"]);
    }

    #[test]
    fn test_fixed_mounts_without_sys_block_keeps_every_disk() {
        let mounts = mounts::parse(
            "30 1 8:1 / / rw - ext4 /dev/sdb1 rw\n31 30 8:2 / /data rw - xfs /dev/sdb2 rw\n",
        );
        assert_eq!(
            names(&fixed_mounts(&fixture("missing"), &mounts)),
            ["/", "/data"]
        );
    }

    #[test]
    fn test_counters_of_matches_by_number_then_by_name() {
        let root = fixture("desktop");
        let counters = diskstats::read(&root).unwrap();
        let mounts = fixed("desktop");
        let found: Vec<Option<&str>> = mounts
            .iter()
            .map(|mount| counters_of(mount, &counters).map(|c| c.name.as_str()))
            .collect();
        // `/srv/vault` is ext4 on dm-0: its source names the mapper link, its number dm-0.
        assert_eq!(
            found,
            [
                Some("nvme0n1p2"),
                Some("nvme0n1p1"),
                Some("sda1"),
                Some("dm-0")
            ]
        );
    }

    #[test]
    fn test_counters_of_by_number_finds_dev_root() {
        let counters = diskstats::parse("8 1 sda1 1 0 8 0 1 0 16 0 0 0 0\n");
        let mount = FixedMount {
            name: "/".into(),
            major: 8,
            minor: 1,
            device: "root".into(),
        };
        assert_eq!(counters_of(&mount, &counters).unwrap().name, "sda1");
    }

    #[test]
    fn test_usage_joins_space_and_rates_and_idles_drives_without_counters() {
        let root = fixture("desktop");
        let after = diskstats::read(&root).unwrap();
        let mut before = after.clone();
        let nvme = before.iter_mut().find(|c| c.name == "nvme0n1p2").unwrap();
        nvme.read_bytes -= 2_048;
        nvme.written_bytes -= 1_024;
        let space = Space {
            total_mb: 1_000,
            used_mb: 400,
        };
        let mounts: Vec<(FixedMount, Space)> = fixed("desktop")
            .into_iter()
            .map(|mount| (mount, space))
            .collect();

        let drives = usage(&mounts, &before, &after, 2.0);
        assert_eq!(
            drives[0],
            DriveUsage {
                name: "/".into(),
                total_mb: 1_000,
                used_mb: 400,
                read_bytes_per_sec: 1_024,
                write_bytes_per_sec: 512,
            }
        );
        assert!(drives[1..].iter().all(|d| d.read_bytes_per_sec == 0));
        let first = usage(&mounts, &[], &after, 0.0);
        assert!(first.iter().all(|d| d.write_bytes_per_sec == 0));
    }
}
