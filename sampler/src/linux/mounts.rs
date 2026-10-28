//! The mount table from `/proc/self/mountinfo`.

use std::{fs, io, path::Path};

/// One line of `/proc/self/mountinfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mount {
    /// Device number of the filesystem. btrfs and other multi-device filesystems report an
    /// anonymous device with major 0 here.
    pub(crate) major: u32,
    pub(crate) minor: u32,
    /// Where it is mounted, unescaped.
    pub(crate) mount_point: String,
    /// Filesystem type, for example `ext4`.
    pub(crate) fs_type: String,
    /// What is mounted, for example `/dev/nvme0n1p2`, `tmpfs`, or `server:/export`.
    pub(crate) source: String,
}

/// Reads `proc/self/mountinfo` below `root`.
pub(crate) fn read(root: &Path) -> io::Result<Vec<Mount>> {
    Ok(parse(&fs::read_to_string(
        root.join("proc/self/mountinfo"),
    )?))
}

/// Parses the text of `/proc/self/mountinfo`, skipping lines it cannot read.
pub(crate) fn parse(text: &str) -> Vec<Mount> {
    text.lines().filter_map(parse_line).collect()
}

/// `36 35 98:0 /mnt1 /mnt2 rw,noatime master:1 - ext3 /dev/root rw,errors=continue`: ID,
/// parent ID, device, root, mount point, options, optional fields up to `-`, then type,
/// source, and superblock options.
fn parse_line(line: &str) -> Option<Mount> {
    let (head, tail) = line.split_once(" - ")?;
    let mut head = head.split(' ');
    let (major, minor) = head.nth(2)?.split_once(':')?;
    let mount_point = head.nth(1)?;
    let mut tail = tail.split(' ');
    Some(Mount {
        major: major.parse().ok()?,
        minor: minor.parse().ok()?,
        mount_point: unescape(mount_point),
        fs_type: tail.next()?.to_owned(),
        source: unescape(tail.next()?),
    })
}

/// Undoes the kernel's octal escapes for space, tab, newline, and backslash (`\040`).
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match octal_escape(&bytes[i..]) {
            Some(byte) => {
                out.push(byte);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The byte a leading `\ooo` escape stands for.
fn octal_escape(bytes: &[u8]) -> Option<u8> {
    let [b'\\', digits @ ..] = bytes.get(..4)? else {
        return None;
    };
    digits.iter().try_fold(0u8, |value, &digit| match digit {
        b'0'..=b'7' => value.checked_mul(8)?.checked_add(digit - b'0'),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::tests::fixture;

    #[test]
    fn test_parse_line_reads_device_mount_point_type_and_source() {
        let mount =
            parse_line("36 35 98:0 /mnt1 /mnt2 rw,noatime master:1 shared:7 - ext3 /dev/root rw");
        assert_eq!(
            mount,
            Some(Mount {
                major: 98,
                minor: 0,
                mount_point: "/mnt2".into(),
                fs_type: "ext3".into(),
                source: "/dev/root".into(),
            })
        );
    }

    #[test]
    fn test_parse_line_without_separator_is_skipped() {
        assert_eq!(parse_line("36 35 98:0 / / rw ext4 /dev/sda1 rw"), None);
        assert_eq!(parse_line("36 35 x / / rw - ext4 /dev/sda1 rw"), None);
    }

    #[test]
    fn test_unescape_turns_octal_escapes_back_into_characters() {
        assert_eq!(unescape(r"/mnt/my\040data"), "/mnt/my data");
        assert_eq!(unescape(r"/a\011b\012c\134d"), "/a\tb\nc\\d");
        assert_eq!(unescape(r"/odd\04"), r"/odd\04");
        assert_eq!(unescape(r"/no\999"), r"/no\999");
    }

    #[test]
    fn test_read_desktop_fixture_lists_every_mount() {
        let mounts = read(&fixture("desktop")).unwrap();
        assert!(mounts.len() > 10, "{mounts:?}");
        let data = mounts.iter().find(|m| m.mount_point == "/mnt/my data");
        assert_eq!(data.unwrap().source, "/dev/sda1");
    }
}
