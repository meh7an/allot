//! The downgrade guard of `allot install`, and the version every part of `allot update` reads
//! from an `allot` executable.

use std::{
    env,
    ffi::OsString,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use allot_shared::{paths::Paths, platform::CLI_EXE};
use allot_updater::Version;
use anyhow::bail;

/// Set to `1` to let `allot install` replace a newer install with an older one, as
/// `--allow-downgrade` does. A variable so `allot update` can pass it on to a release whose own
/// installer predates the flag, and so the setup on Windows, which has no flags to forward, gets it.
pub(crate) const ALLOW_DOWNGRADE_VAR: &str = "ALLOT_ALLOW_DOWNGRADE";
/// How long a `--version` has to answer.
const VERSION_WAIT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(20);

/// What `~/.allot/bin` holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Installed {
    /// No `allot` there.
    Missing,
    /// An `allot` that did not print a version, such as one Gatekeeper stops.
    Unreadable,
    Version(Version),
}

/// This build's version.
pub(crate) fn running_version() -> Version {
    // Cargo refuses a package whose version is not semver, so this cannot fail.
    Version::parse(env!("CARGO_PKG_VERSION")).expect("cargo checked the package version")
}

/// The `allot` in the bin directory of `paths`, and its version.
pub(crate) fn installed(paths: &Paths) -> Installed {
    let exe = paths.bin_dir().join(CLI_EXE);
    if !exe.is_file() {
        return Installed::Missing;
    }
    version_of(&exe).map_or(Installed::Unreadable, Installed::Version)
}

/// What `exe --version` prints as a version, or `None` when it does not run, fails, or does not
/// answer in time.
pub(crate) fn version_of(exe: &Path) -> Option<Version> {
    version_within(exe, VERSION_WAIT)
}

fn version_within(exe: &Path, wait: Duration) -> Option<Version> {
    let mut child = Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + wait;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() >= deadline => {
                // Nothing to do when it already exited: the wait reaps it either way.
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => thread::sleep(POLL),
        }
    }
    let mut printed = String::new();
    child.stdout.take()?.read_to_string(&mut printed).ok()?;
    parse_printed(&printed)
}

/// The version in `allot --version`'s output, such as `allot 0.6.1`.
fn parse_printed(output: &str) -> Option<Version> {
    let mut words = output.lines().next()?.split_whitespace();
    let (program, version, rest) = (words.next()?, words.next()?, words.next());
    (program == "allot" && rest.is_none())
        .then(|| Version::parse(version).ok())
        .flatten()
}

/// Whether the environment allows a downgrade.
pub(crate) fn allowed_by_env() -> bool {
    allowed_by(env::var_os(ALLOW_DOWNGRADE_VAR))
}

fn allowed_by(value: Option<OsString>) -> bool {
    value.is_some_and(|value| !value.is_empty() && value != "0")
}

/// Fails when the install holds a newer allot than `incoming` and `allow` is not set. An equal
/// version passes, since installing it again repairs an install, and so does an install whose
/// version cannot be read.
///
/// # Errors
///
/// Fails with what to run instead when `installed` is newer than `incoming` and `allow` is off.
pub(crate) fn refuse_downgrade(
    installed: &Installed,
    incoming: &Version,
    bin: &Path,
    allow: bool,
) -> anyhow::Result<()> {
    match installed {
        Installed::Version(have) if have > incoming && !allow => bail!(
            "{} holds allot {have}, which is newer than the {incoming} being installed; \
             `allot update` brings it to the latest release, and --allow-downgrade (or \
             {ALLOW_DOWNGRADE_VAR}=1) installs {incoming} anyway. A config.toml saved by {have} \
             can hold settings that {incoming} rejects",
            bin.display()
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::TempDir;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn test_parse_printed_reads_what_allot_version_prints() {
        assert_eq!(parse_printed("allot 0.6.1\n"), Some(version("0.6.1")));
        assert_eq!(
            parse_printed("allot 0.7.0-beta.1"),
            Some(version("0.7.0-beta.1"))
        );
    }

    #[test]
    fn test_parse_printed_refuses_anything_else() {
        for output in [
            "",
            "allot",
            "allotd 0.6.1",
            "allot 0.6",
            "allot 0.6.1 extra",
            "error: unexpected argument",
            "\nallot 0.6.1",
        ] {
            assert_eq!(parse_printed(output), None, "{output:?}");
        }
    }

    #[test]
    fn test_allowed_by_takes_any_value_but_empty_and_zero() {
        assert!(allowed_by(Some("1".into())));
        assert!(allowed_by(Some("yes".into())));
        assert!(!allowed_by(Some("0".into())));
        assert!(!allowed_by(Some(OsString::new())));
        assert!(!allowed_by(None));
    }

    #[test]
    fn test_refuse_downgrade_stops_only_a_newer_install() {
        let bin = Path::new("bin");
        let newer = Installed::Version(version("0.7.0"));
        let message = refuse_downgrade(&newer, &version("0.6.1"), bin, false)
            .unwrap_err()
            .to_string();
        assert!(message.contains("0.7.0"), "{message}");
        assert!(message.contains("--allow-downgrade"), "{message}");
        assert!(message.contains(ALLOW_DOWNGRADE_VAR), "{message}");

        assert!(refuse_downgrade(&newer, &version("0.7.0"), bin, false).is_ok());
        assert!(refuse_downgrade(&newer, &version("0.8.0"), bin, false).is_ok());
        assert!(refuse_downgrade(&newer, &version("0.7.0-beta.1"), bin, false).is_err());
    }

    #[test]
    fn test_refuse_downgrade_lets_the_override_and_an_unknown_install_through() {
        let bin = Path::new("bin");
        let newer = Installed::Version(version("0.7.0"));
        assert!(refuse_downgrade(&newer, &version("0.6.1"), bin, true).is_ok());
        for installed in [Installed::Missing, Installed::Unreadable] {
            assert!(refuse_downgrade(&installed, &version("0.1.0"), bin, false).is_ok());
        }
    }

    #[test]
    fn test_running_version_is_the_package_version() {
        assert_eq!(running_version().to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_installed_without_an_allot_is_missing_and_with_a_broken_one_unreadable() {
        let home = TempDir::new();
        let paths = Paths::under(home.path());
        assert_eq!(installed(&paths), Installed::Missing);

        fs::create_dir_all(paths.bin_dir()).unwrap();
        fs::write(paths.bin_dir().join(CLI_EXE), "not a program").unwrap();
        assert_eq!(installed(&paths), Installed::Unreadable);
    }

    #[cfg(unix)]
    fn script(dir: &Path, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let exe = dir.join("allot");
        fs::write(&exe, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        exe
    }

    /// [`version_of`], tried again a few times: a script written a moment ago can still be open
    /// for writing in a process another test forked, which makes the first exec fail with ETXTBSY.
    #[cfg(unix)]
    fn version_of_script(exe: &Path) -> Option<Version> {
        for _ in 0..5 {
            if let Some(found) = version_of(exe) {
                return Some(found);
            }
            thread::sleep(Duration::from_millis(50));
        }
        None
    }

    #[cfg(unix)]
    #[test]
    fn test_version_of_runs_the_executable_and_reads_its_version() {
        let dir = TempDir::new();
        let exe = script(dir.path(), "echo allot 0.9.2");
        assert_eq!(version_of_script(&exe), Some(version("0.9.2")));
    }

    #[cfg(unix)]
    #[test]
    fn test_version_of_gives_up_on_a_failing_or_silent_executable() {
        let dir = TempDir::new();
        let failing = script(dir.path(), "echo allot 0.9.2; exit 3");
        assert_eq!(version_of(&failing), None);

        let hanging = script(dir.path(), "exec sleep 30");
        let started = Instant::now();
        assert_eq!(version_within(&hanging, Duration::from_millis(200)), None);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn test_installed_reads_the_version_of_the_installed_allot() {
        let home = TempDir::new();
        let paths = Paths::under(home.path());
        fs::create_dir_all(paths.bin_dir()).unwrap();
        let exe = script(&paths.bin_dir(), "echo allot 0.9.2");
        version_of_script(&exe);
        assert_eq!(installed(&paths), Installed::Version(version("0.9.2")));
    }
}
