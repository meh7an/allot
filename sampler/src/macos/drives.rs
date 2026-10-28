//! Fixed drives: browsable, writable volumes on local disks, with space from statfs and read
//! and write rates from the `IOBlockStorageDriver` of the disk behind each volume.

use allot_shared::types::DriveUsage;

use crate::{macos::value::Entry, rates::rate};

const BYTES_PER_MB: u64 = 1024 * 1024;

// Mount flags from <sys/mount.h>, here so the fixture tests build on every OS.
const MNT_RDONLY: u32 = 0x0000_0001;
const MNT_REMOVABLE: u32 = 0x0000_0200;
const MNT_LOCAL: u32 = 0x0000_1000;
const MNT_ROOTFS: u32 = 0x0000_4000;
const MNT_DONTBROWSE: u32 = 0x0010_0000;

/// The fields of one `statfs` entry the sampler uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mount {
    pub(crate) mount_point: String,
    /// The device, for example `/dev/disk3s1s1`.
    pub(crate) source: String,
    pub(crate) flags: u32,
    pub(crate) block_bytes: u64,
    pub(crate) blocks: u64,
    /// Blocks free for unprivileged use.
    pub(crate) available_blocks: u64,
}

/// A fixed drive and the device behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixedMount {
    /// The mount point, which names the drive.
    pub(crate) name: String,
    /// BSD name of the volume, for example `disk3s1s1`.
    pub(crate) device: String,
    pub(crate) space: Space,
}

/// Space of one fixed drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Space {
    pub(crate) total_mb: u64,
    pub(crate) used_mb: u64,
}

/// The mounts that are fixed drives, sorted by mount point: on a local disk, shown in the
/// Finder (system volumes such as `/System/Volumes/Data` are not), not removable (disk
/// images are), and writable, except for the read-only system snapshot at `/`.
///
/// The volumes of an APFS container share its space, so a volume's used space is the
/// container's (total minus available), as the Finder shows it; its own blocks in use would
/// count only that volume.
pub(crate) fn fixed_mounts(mounts: &[Mount]) -> Vec<FixedMount> {
    let mut fixed: Vec<FixedMount> = mounts
        .iter()
        .filter(|mount| is_fixed(mount))
        .map(|mount| FixedMount {
            name: mount.mount_point.clone(),
            device: mount.source.trim_start_matches("/dev/").to_owned(),
            space: Space {
                total_mb: mount.blocks.saturating_mul(mount.block_bytes) / BYTES_PER_MB,
                used_mb: mount
                    .blocks
                    .saturating_sub(mount.available_blocks)
                    .saturating_mul(mount.block_bytes)
                    / BYTES_PER_MB,
            },
        })
        .collect();
    fixed.sort_by(|a, b| a.name.cmp(&b.name));
    fixed
}

fn is_fixed(mount: &Mount) -> bool {
    let has = |flag: u32| mount.flags & flag != 0;
    has(MNT_LOCAL)
        && mount.source.starts_with("/dev/disk")
        && !has(MNT_DONTBROWSE)
        && !has(MNT_REMOVABLE)
        && (!has(MNT_RDONLY) || has(MNT_ROOTFS))
}

/// Cumulative counters of one `IOBlockStorageDriver`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DriverCounters {
    /// Registry ID of the driver. Some drivers have no BSD name, so the ID is the key.
    pub(crate) id: u64,
    pub(crate) read_bytes: u64,
    pub(crate) written_bytes: u64,
}

/// The driver property the reader uses.
pub(crate) const DRIVER_KEYS: [&str; 1] = ["Statistics"];

/// The counters of a driver entry; `None` without its `Statistics`.
pub(crate) fn driver_counters(driver: &Entry) -> Option<DriverCounters> {
    let statistics = driver.properties.get("Statistics")?;
    Some(DriverCounters {
        id: driver.id,
        read_bytes: statistics.u64_at("Bytes (Read)")?,
        written_bytes: statistics.u64_at("Bytes (Write)")?,
    })
}

/// Joins each drive, paired with the registry ID of its disk's driver when one was found,
/// with its space and its rates since the previous reading.
pub(crate) fn usage(
    drives: &[(FixedMount, Option<u64>)],
    before: &[DriverCounters],
    after: &[DriverCounters],
    elapsed_secs: f64,
) -> Vec<DriveUsage> {
    let find = |counters: &[DriverCounters], id: u64| {
        counters.iter().find(|counter| counter.id == id).copied()
    };
    drives
        .iter()
        .map(|(mount, driver)| {
            let counters = driver.and_then(|id| Some((find(before, id)?, find(after, id)?)));
            let (read, write) = counters.map_or((0, 0), |(before, after)| {
                (
                    rate(before.read_bytes, after.read_bytes, elapsed_secs),
                    rate(before.written_bytes, after.written_bytes, elapsed_secs),
                )
            });
            DriveUsage {
                name: mount.name.clone(),
                total_mb: mount.space.total_mb,
                used_mb: mount.space.used_mb,
                read_bytes_per_sec: read,
                write_bytes_per_sec: write,
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
pub(crate) use live::Drives;

#[cfg(target_os = "macos")]
mod live {
    use std::{
        ffi::CStr,
        time::{Duration, Instant},
    };

    use allot_shared::types::DriveUsage;

    use super::{DriverCounters, FixedMount, Mount, driver_counters, fixed_mounts, usage};
    use crate::{cadence::Cadence, macos::iokit};

    /// How often the mount list and free space are read again; space changes slowly.
    const SPACE_REFRESH: Duration = Duration::from_secs(10);

    /// The fixed drives, with the driver counters of the previous reading.
    #[derive(Debug)]
    pub(crate) struct Drives {
        refresh: Cadence,
        drives: Vec<(FixedMount, Option<u64>)>,
        before: Vec<DriverCounters>,
        after: Vec<DriverCounters>,
        elapsed_secs: f64,
        read_at: Option<Instant>,
    }

    impl Drives {
        pub(crate) fn new() -> Self {
            Self {
                refresh: Cadence::new(SPACE_REFRESH),
                drives: Vec::new(),
                before: Vec::new(),
                after: Vec::new(),
                elapsed_secs: 0.0,
                read_at: None,
            }
        }

        /// Reads the driver counters, and the drive list and space when they are due at
        /// `now`. A drive whose disk driver is not found reads as idle.
        pub(crate) fn refresh(&mut self, now: Instant) {
            if self.refresh.due(now) {
                self.drives = fixed_mounts(&mounts())
                    .into_iter()
                    .map(|mount| {
                        let driver = iokit::storage_driver_id(&mount.device);
                        (mount, driver)
                    })
                    .collect();
            }
            self.before = std::mem::take(&mut self.after);
            self.after = iokit::entries(c"IOBlockStorageDriver", &super::DRIVER_KEYS, &[])
                .iter()
                .filter_map(driver_counters)
                .collect();
            self.elapsed_secs = self
                .read_at
                .map_or(0.0, |at| now.saturating_duration_since(at).as_secs_f64());
            self.read_at = Some(now);
        }

        /// Every fixed drive with its read and write rates since the previous refresh.
        pub(crate) fn read(&self) -> Vec<DriveUsage> {
            usage(&self.drives, &self.before, &self.after, self.elapsed_secs)
        }
    }

    /// The mounted filesystems, without waiting on unresponsive network mounts.
    fn mounts() -> Vec<Mount> {
        let mut entries: *mut libc::statfs = std::ptr::null_mut();
        // SAFETY: getmntinfo stores a pointer to a buffer it owns in `entries` and returns
        // how many entries it holds.
        let count = unsafe { libc::getmntinfo(&mut entries, libc::MNT_NOWAIT) };
        let Ok(count) = usize::try_from(count) else {
            return Vec::new();
        };
        if entries.is_null() {
            return Vec::new();
        }
        // SAFETY: the buffer holds `count` initialized entries and stays valid until the
        // next getmntinfo call on this thread; the sampler makes none while it is read.
        let entries = unsafe { std::slice::from_raw_parts(entries, count) };
        entries
            .iter()
            .map(|entry| Mount {
                mount_point: text(&entry.f_mntonname),
                source: text(&entry.f_mntfromname),
                flags: entry.f_flags,
                block_bytes: u64::from(entry.f_bsize),
                blocks: entry.f_blocks,
                available_blocks: entry.f_bavail,
            })
            .collect()
    }

    fn text(field: &[libc::c_char]) -> String {
        // SAFETY: c_char and u8 have the same size and alignment.
        let bytes = unsafe { &*(std::ptr::from_ref(field) as *const [u8]) };
        CStr::from_bytes_until_nul(bytes)
            .map(|text| text.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::{
        tests::{fixture_json, ioreg_fixture},
        value::Value,
    };

    fn mounts(case: &str) -> Vec<Mount> {
        let json = fixture_json(case, "mounts.json");
        let number = |mount: &serde_json::Value, key: &str| mount[key].as_u64().unwrap();
        json.as_array()
            .unwrap()
            .iter()
            .map(|mount| Mount {
                mount_point: mount["mount_point"].as_str().unwrap().to_owned(),
                source: mount["source"].as_str().unwrap().to_owned(),
                flags: u32::try_from(number(mount, "flags")).unwrap(),
                block_bytes: number(mount, "block_bytes"),
                blocks: number(mount, "blocks"),
                available_blocks: number(mount, "available_blocks"),
            })
            .collect()
    }

    fn names(mounts: &[FixedMount]) -> Vec<&str> {
        mounts.iter().map(|mount| mount.name.as_str()).collect()
    }

    fn external(mount_point: &str, flags: u32) -> Mount {
        Mount {
            mount_point: mount_point.into(),
            source: "/dev/disk6s1".into(),
            flags,
            block_bytes: 4_096,
            blocks: 488_281_250,
            available_blocks: 244_140_625,
        }
    }

    #[test]
    fn test_fixed_mounts_royalmac_lists_only_the_startup_volume() {
        // System volumes and the second container's Data volume are nobrowse, its System
        // volume is read-only, and disk images are removable; devfs, autofs, nullfs, and
        // devicefs have no disk.
        let fixed = fixed_mounts(&mounts("m3max"));
        assert_eq!(names(&fixed), ["/"]);
        assert!(fixed[0].device.starts_with("disk"), "{}", fixed[0].device);
    }

    #[test]
    fn test_fixed_mounts_count_the_containers_space_as_used() {
        let fixed = fixed_mounts(&mounts("m3max"));
        let root = &fixed[0].space;
        // The sealed system snapshot alone holds about 13 GB; the container holds far more.
        assert!(root.used_mb > 100_000, "{root:?}");
        assert!(root.used_mb < root.total_mb, "{root:?}");
    }

    #[test]
    fn test_fixed_mounts_keep_writable_external_disks() {
        let flags = MNT_LOCAL;
        let fixed = fixed_mounts(&[
            external("/Volumes/Backup", flags),
            external("/Volumes/Archive", flags | MNT_RDONLY),
            external("/Volumes/Installer", flags | MNT_REMOVABLE),
        ]);
        assert_eq!(names(&fixed), ["/Volumes/Backup"]);
        assert_eq!(fixed[0].device, "disk6s1");
        assert_eq!(
            fixed[0].space,
            Space {
                total_mb: 1_907_348,
                used_mb: 953_674
            }
        );
    }

    #[test]
    fn test_driver_counters_read_every_driver_with_statistics() {
        let drivers = ioreg_fixture("m3max", "storage-t0.json");
        let counters: Vec<DriverCounters> = drivers.iter().filter_map(driver_counters).collect();
        assert_eq!(counters.len(), drivers.len());
        assert!(counters.iter().all(|counter| counter.id != 0));
        assert!(counters.iter().any(|counter| counter.written_bytes > 0));

        let mut empty = drivers[0].clone();
        empty.properties = Value::Dict(Vec::new());
        assert_eq!(driver_counters(&empty), None);
    }

    #[test]
    fn test_usage_reads_rates_from_the_drives_driver_and_idles_without_one() {
        let before: Vec<DriverCounters> = ioreg_fixture("m3max", "storage-t0.json")
            .iter()
            .filter_map(driver_counters)
            .collect();
        let after: Vec<DriverCounters> = ioreg_fixture("m3max", "storage-t1.json")
            .iter()
            .filter_map(driver_counters)
            .collect();
        let busiest = after
            .iter()
            .max_by_key(|counter| {
                let old = before.iter().find(|old| old.id == counter.id).unwrap();
                counter.written_bytes - old.written_bytes
            })
            .unwrap();
        let root = fixed_mounts(&mounts("m3max")).remove(0);
        let backup = fixed_mounts(&[external("/Volumes/Backup", MNT_LOCAL)]).remove(0);
        let drives = [(root, Some(busiest.id)), (backup, None)];

        let usage = usage(&drives, &before, &after, 1.0);
        assert_eq!(usage[0].name, "/");
        assert!(usage[0].write_bytes_per_sec > 0, "{:?}", usage[0]);
        assert_eq!(usage[0].total_mb, drives[0].0.space.total_mb);
        assert_eq!(usage[1].name, "/Volumes/Backup");
        assert_eq!(
            (usage[1].read_bytes_per_sec, usage[1].write_bytes_per_sec),
            (0, 0)
        );
    }
}
