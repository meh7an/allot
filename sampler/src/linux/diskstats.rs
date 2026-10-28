//! Cumulative block device I/O from `/proc/diskstats`.

use std::{fs, io, path::Path};

/// `/proc/diskstats` counts sectors of 512 bytes whatever the device's sector size.
const SECTOR_BYTES: u64 = 512;

/// Bytes read and written by one block device or partition since boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiskCounters {
    pub(crate) major: u32,
    pub(crate) minor: u32,
    /// Kernel name, for example `nvme0n1p2` or `dm-0`.
    pub(crate) name: String,
    pub(crate) read_bytes: u64,
    pub(crate) written_bytes: u64,
}

/// Reads `proc/diskstats` below `root`.
pub(crate) fn read(root: &Path) -> io::Result<Vec<DiskCounters>> {
    Ok(parse(&fs::read_to_string(root.join("proc/diskstats"))?))
}

/// Parses the text of `/proc/diskstats`, skipping lines it cannot read.
pub(crate) fn parse(text: &str) -> Vec<DiskCounters> {
    text.lines().filter_map(parse_line).collect()
}

/// `259 2 nvme0n1p2 r rm sectors-read ms w wm sectors-written ...`; kernels add fields at the
/// end (discard, flush), never in between.
fn parse_line(line: &str) -> Option<DiskCounters> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let number = |index: usize| fields.get(index)?.parse::<u64>().ok();
    Some(DiskCounters {
        major: fields.first()?.parse().ok()?,
        minor: fields.get(1)?.parse().ok()?,
        name: (*fields.get(2)?).to_owned(),
        read_bytes: number(5)?.saturating_mul(SECTOR_BYTES),
        written_bytes: number(9)?.saturating_mul(SECTOR_BYTES),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::tests::fixture;

    #[test]
    fn test_parse_line_reads_sectors_as_bytes() {
        let line = "259       2 nvme0n1p2 81234 1200 6543210 40000 52345 9000 2000000 70000 0 \
                    60000 110000 0 0 0 0 1500 300";
        assert_eq!(
            parse_line(line),
            Some(DiskCounters {
                major: 259,
                minor: 2,
                name: "nvme0n1p2".into(),
                read_bytes: 6_543_210 * 512,
                written_bytes: 2_000_000 * 512,
            })
        );
    }

    #[test]
    fn test_parse_line_accepts_the_old_eleven_field_format() {
        let counters = parse_line("8 0 sda 1 2 30 4 5 6 70 8 0 9 10").unwrap();
        assert_eq!(
            (counters.read_bytes, counters.written_bytes),
            (30 * 512, 70 * 512)
        );
    }

    #[test]
    fn test_parse_line_too_short_or_not_numeric_is_skipped() {
        assert_eq!(parse_line("8 0 sda 1 2 3"), None);
        assert_eq!(parse_line("8 x sda 1 2 3 4 5 6 7 8 9 10 11"), None);
    }

    #[test]
    fn test_read_desktop_fixture_lists_disks_partitions_and_mapper_devices() {
        let counters = read(&fixture("desktop")).unwrap();
        let names: Vec<&str> = counters.iter().map(|c| c.name.as_str()).collect();
        assert!(
            names.contains(&"nvme0n1p2") && names.contains(&"dm-0"),
            "{names:?}"
        );
    }
}
