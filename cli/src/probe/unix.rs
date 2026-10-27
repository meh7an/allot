//! Process groups, sessions and terminals of the probe and its ancestors on Linux and macOS:
//! what binding needs to tell a command's shell apart from the Claude session around it.

use std::{
    fmt::{self, Write as _},
    io, mem,
    time::{SystemTime, UNIX_EPOCH},
};

use allot_core::ancestry::ProcessNode;

/// How much of a command line the report keeps; Claude's own command lines can be long.
const ARGV_LIMIT: usize = 300;

/// What the report shows for a process the OS would not describe.
const UNKNOWN: &str = "?";

/// The terminal, `comm`, and command line of one process.
#[derive(Debug)]
struct Details {
    tty: String,
    comm: String,
    argv: String,
}

/// Appends the probe's standard streams, a millisecond timestamp for ordering hooks, and the
/// process group, session, terminal, `comm` and command line of every process in `chain`.
pub fn write_details(report: &mut String, chain: &[ProcessNode]) -> fmt::Result {
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    writeln!(report, "-- unix unix_ms={unix_ms}")?;
    for fd in 0..=2 {
        writeln!(report, "  fd{fd}: {}", fd_kind(fd))?;
    }
    writeln!(report, "-- unix ancestry: pid pgid sid tty comm | argv")?;
    for node in chain {
        let Ok(pid) = libc::pid_t::try_from(node.pid) else {
            continue;
        };
        // SAFETY: getpgid and getsid only read the kernel's ids for `pid`.
        let pgid = id_or_errno(unsafe { libc::getpgid(pid) });
        // SAFETY: as above.
        let sid = id_or_errno(unsafe { libc::getsid(pid) });
        let details = details(pid);
        writeln!(
            report,
            "  {} {pgid} {sid} {} {} | {}",
            node.pid, details.tty, details.comm, details.argv,
        )?;
    }
    Ok(())
}

/// Must run right after the call that returned `id`, before anything else can set `errno`.
fn id_or_errno(id: libc::pid_t) -> String {
    if id == -1 {
        let errno = io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or_default();
        format!("errno{errno}")
    } else {
        id.to_string()
    }
}

fn fd_kind(fd: libc::c_int) -> &'static str {
    // SAFETY: an all-zero `stat` is a valid value for fstat to overwrite.
    let mut stat: libc::stat = unsafe { mem::zeroed() };
    // SAFETY: fstat writes only into `stat`; a closed descriptor makes it fail.
    if unsafe { libc::fstat(fd, &raw mut stat) } != 0 {
        return "closed";
    }
    match stat.st_mode & libc::S_IFMT {
        libc::S_IFIFO => "pipe",
        libc::S_IFSOCK => "socket",
        libc::S_IFREG => "file",
        // SAFETY: isatty only inspects the descriptor.
        libc::S_IFCHR if unsafe { libc::isatty(fd) } == 1 => "tty",
        libc::S_IFCHR => "char device",
        _ => "other",
    }
}

fn join_args<'a>(args: impl Iterator<Item = &'a [u8]>) -> String {
    let joined = args
        .filter(|arg| !arg.is_empty())
        .map(String::from_utf8_lossy)
        .collect::<Vec<_>>()
        .join(" ");
    joined.chars().take(ARGV_LIMIT).collect()
}

#[cfg(target_os = "linux")]
fn details(pid: libc::pid_t) -> Details {
    let read = |name: &str| std::fs::read(format!("/proc/{pid}/{name}")).unwrap_or_default();
    let stat = String::from_utf8_lossy(&read("stat")).into_owned();
    // `comm` may hold spaces and parentheses, so the fields after it start at the last ')'.
    let tty_nr = stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().nth(4))
        .and_then(|field| field.parse::<u32>().ok());
    let cmdline = read("cmdline");
    Details {
        tty: tty_nr.map_or_else(|| UNKNOWN.to_owned(), linux_tty),
        comm: String::from_utf8_lossy(&read("comm")).trim_end().to_owned(),
        argv: join_args(cmdline.split(|&byte| byte == 0)),
    }
}

/// Formats `tty_nr` from `/proc/PID/stat` as `major:minor`, the encoding `proc(5)` describes.
#[cfg(target_os = "linux")]
fn linux_tty(tty_nr: u32) -> String {
    if tty_nr == 0 {
        return "-".to_owned();
    }
    let major = (tty_nr >> 8) & 0xfff;
    let minor = (tty_nr & 0xff) | ((tty_nr >> 12) & 0xf_ff00);
    format!("{major}:{minor}")
}

#[cfg(target_os = "macos")]
fn details(pid: libc::pid_t) -> Details {
    // SAFETY: an all-zero `proc_bsdinfo` is a valid value for proc_pidinfo to overwrite.
    let mut info: libc::proc_bsdinfo = unsafe { mem::zeroed() };
    let Ok(size) = libc::c_int::try_from(mem::size_of::<libc::proc_bsdinfo>()) else {
        return unknown_details(pid);
    };
    // SAFETY: proc_pidinfo writes at most `size` bytes into `info`.
    let written =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) };
    if written != size {
        return unknown_details(pid);
    }
    let comm: Vec<u8> = info
        .pbi_comm
        .iter()
        .take_while(|&&raw| raw != 0)
        .map(|&raw| raw as u8)
        .collect();
    Details {
        tty: macos_tty(info.e_tdev),
        comm: String::from_utf8_lossy(&comm).into_owned(),
        argv: macos_args(pid),
    }
}

#[cfg(target_os = "macos")]
fn unknown_details(pid: libc::pid_t) -> Details {
    Details {
        tty: UNKNOWN.to_owned(),
        comm: UNKNOWN.to_owned(),
        argv: macos_args(pid),
    }
}

/// Formats a BSD `dev_t` as `major:minor`; `NODEV` (all bits set) means no terminal.
#[cfg(target_os = "macos")]
fn macos_tty(dev: u32) -> String {
    if dev == u32::MAX {
        return "-".to_owned();
    }
    format!("{}:{}", dev >> 24, dev & 0xff_ffff)
}

#[cfg(target_os = "macos")]
fn macos_args(pid: libc::pid_t) -> String {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: libc::size_t = 0;
    // SAFETY: a null buffer asks sysctl only for the size it needs.
    let sized = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            std::ptr::null_mut(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if sized != 0 {
        return UNKNOWN.to_owned();
    }
    let mut buffer = vec![0_u8; size];
    // SAFETY: `buffer` holds `size` bytes, and sysctl writes at most that many.
    let filled = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if filled != 0 {
        return UNKNOWN.to_owned();
    }
    buffer.truncate(size);
    procargs_argv(&buffer).unwrap_or_else(|| UNKNOWN.to_owned())
}

/// Reads argv out of a `KERN_PROCARGS2` buffer: argc, the exec path, NUL padding, then the
/// arguments followed by the environment.
#[cfg(any(target_os = "macos", test))]
fn procargs_argv(buffer: &[u8]) -> Option<String> {
    let (argc, rest) = buffer.split_first_chunk::<4>()?;
    let argc = usize::try_from(i32::from_ne_bytes(*argc)).ok()?;
    let args = rest
        .split(|&byte| byte == 0)
        .filter(|part| !part.is_empty())
        .skip(1)
        .take(argc);
    Some(join_args(args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_args_skips_empty_parts_and_caps_length() {
        let raw = b"claude\0--print\0\0";
        assert_eq!(join_args(raw.split(|&byte| byte == 0)), "claude --print");
        let long = vec![b'x'; ARGV_LIMIT * 2];
        assert_eq!(
            join_args(std::iter::once(long.as_slice())).len(),
            ARGV_LIMIT
        );
    }

    #[test]
    fn test_procargs_argv_skips_exec_path_and_stops_before_env() {
        let mut buffer = 2_i32.to_ne_bytes().to_vec();
        buffer.extend_from_slice(b"/opt/bin/claude\0\0\0\0claude\0-p\0HOME=/Users/x\0");
        assert_eq!(procargs_argv(&buffer).as_deref(), Some("claude -p"));
    }

    #[test]
    fn test_procargs_argv_short_buffer_is_none() {
        assert_eq!(procargs_argv(&[1, 0]), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_tty_decodes_pts_and_none() {
        assert_eq!(linux_tty(0), "-");
        assert_eq!(linux_tty((136 << 8) | 3), "136:3");
    }

    #[test]
    fn test_write_details_lists_own_process() {
        let chain = allot_sampler::current_ancestry().unwrap();
        let mut report = String::new();
        write_details(&mut report, &chain).unwrap();
        let own = format!("\n  {} ", std::process::id());
        assert!(report.contains(&own), "{report}");
    }
}
