//! Linux and macOS: the new release installs itself, with its own `allot install`.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use allot_shared::platform::CLI_EXE;
use anyhow::{Context, bail};

use crate::{guard::ALLOW_DOWNGRADE_VAR, registration::Setup};

/// The release's own `allot install --from <its folder>`, keeping what `setup` says this install
/// has. The downgrade override goes by environment variable, because a release from before the
/// guard does not know the flag.
fn installer(root: &Path, setup: Setup, allow_downgrade: bool) -> Command {
    let mut command = Command::new(exe(root));
    command.arg("install").arg("--from").arg(root);
    if !setup.claude {
        command.arg("--no-claude");
    }
    if !setup.autostart {
        command.arg("--no-autostart");
    }
    if allow_downgrade {
        command.env(ALLOW_DOWNGRADE_VAR, "1");
    }
    command
}

fn exe(root: &Path) -> PathBuf {
    root.join(CLI_EXE)
}

/// Runs the installer of the release unpacked in `root`, with its output on this one's.
///
/// # Errors
///
/// Fails when the installer cannot start or exits with a failure.
pub(super) fn run(root: &Path, setup: Setup, allow_downgrade: bool) -> anyhow::Result<()> {
    let program = exe(root);
    let status = installer(root, setup, allow_downgrade)
        .status()
        .with_context(|| format!("starting {}", program.display()))?;
    if !status.success() {
        bail!("`{} install` exited with {status}", program.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    fn args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn setup(claude: bool, autostart: bool) -> Setup {
        Setup { claude, autostart }
    }

    #[test]
    fn test_installer_runs_the_releases_own_allot_from_its_folder() {
        let root = Path::new("allot-0.7.0-x86_64-unknown-linux-gnu");

        let command = installer(root, setup(true, true), false);

        assert_eq!(command.get_program(), root.join(CLI_EXE).as_os_str());
        assert_eq!(
            args(&command),
            ["install", "--from", "allot-0.7.0-x86_64-unknown-linux-gnu"]
        );
        assert_eq!(command.get_envs().count(), 0);
    }

    #[test]
    fn test_installer_leaves_out_what_this_install_does_not_have() {
        let root = Path::new("release");

        assert_eq!(
            args(&installer(root, setup(false, true), false)),
            ["install", "--from", "release", "--no-claude"]
        );
        assert_eq!(
            args(&installer(root, setup(true, false), false)),
            ["install", "--from", "release", "--no-autostart"]
        );
        assert_eq!(
            args(&installer(root, setup(false, false), false)),
            [
                "install",
                "--from",
                "release",
                "--no-claude",
                "--no-autostart"
            ]
        );
    }

    #[test]
    fn test_installer_passes_the_downgrade_override_as_a_variable_not_a_flag() {
        let command = installer(Path::new("release"), setup(true, true), true);

        assert_eq!(args(&command), ["install", "--from", "release"]);
        let envs: Vec<_> = command.get_envs().collect();
        assert_eq!(
            envs,
            [(OsStr::new(ALLOW_DOWNGRADE_VAR), Some(OsStr::new("1")))]
        );
    }

    #[cfg(unix)]
    mod run {
        use std::{fs, os::unix::fs::PermissionsExt, thread, time::Duration};

        use super::*;
        use crate::test_support::TempDir;

        /// A release folder whose `allot` records its arguments and exits with `code`.
        fn release(dir: &TempDir, code: i32) -> PathBuf {
            let root = dir.path().join("release");
            fs::create_dir_all(&root).unwrap();
            let log = dir.path().join("args");
            let script = format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit {code}\n",
                log.display()
            );
            let exe = root.join(CLI_EXE);
            fs::write(&exe, script).unwrap();
            fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
            root
        }

        /// [`run`], again when the script another test wrote a moment ago is still open for
        /// writing in a process it forked, which fails the first exec with ETXTBSY.
        fn run_settled(root: &Path) -> anyhow::Result<()> {
            let mut result = run(root, setup(true, false), false);
            for _ in 0..5 {
                let busy = result
                    .as_ref()
                    .err()
                    .is_some_and(|error| format!("{error:#}").contains("busy"));
                if !busy {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
                result = run(root, setup(true, false), false);
            }
            result
        }

        #[test]
        fn test_run_starts_the_installer_with_the_install_arguments() {
            let dir = TempDir::new();
            let root = release(&dir, 0);

            run_settled(&root).unwrap();

            let recorded = fs::read_to_string(dir.path().join("args")).unwrap();
            assert_eq!(
                recorded.lines().collect::<Vec<_>>(),
                [
                    "install",
                    "--from",
                    root.to_str().unwrap(),
                    "--no-autostart"
                ]
            );
        }

        #[test]
        fn test_run_fails_when_the_installer_does() {
            let dir = TempDir::new();
            let root = release(&dir, 3);

            let message = format!("{:#}", run_settled(&root).unwrap_err());

            assert!(message.contains("exited with"), "{message}");
        }

        #[test]
        fn test_run_fails_when_the_release_has_no_allot() {
            let dir = TempDir::new();

            let message = format!(
                "{:#}",
                run(dir.path(), setup(true, true), false).unwrap_err()
            );

            assert!(message.contains("starting"), "{message}");
        }
    }
}
