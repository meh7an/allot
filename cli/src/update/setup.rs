//! Windows: the release's setup, run silently, and its `install.log` read the way
//! `install.ps1` reads it. The setup exits 0 whatever `allot.exe install` did, so the log is the
//! only account of it.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, bail};

use crate::guard::ALLOW_DOWNGRADE_VAR;

/// The flags `install.ps1` runs the setup with.
const SETUP_FLAGS: [&str; 4] = ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-"];
/// How long the setup has to finish. Stopping allotd alone can take 90 s.
const SETUP_WAIT: Duration = Duration::from_secs(20 * 60);
const POLL: Duration = Duration::from_millis(200);
/// How much older than the run a log may be and still count as this run's, for clocks that
/// disagree by a little.
const LOG_SLACK: Duration = Duration::from_secs(2);

/// The setup at `setup`, with the Claude Code task `claude` says this install has. Without a
/// task list the setup would take the last wizard choice, which an install that did not come
/// from a setup never made. The launch-at-logon task stays with that memory: nothing here can
/// read it.
fn command(setup: &Path, claude: bool, allow_downgrade: bool) -> Command {
    let mut command = Command::new(setup);
    command
        .args(SETUP_FLAGS)
        .arg(format!(
            "/MERGETASKS={}",
            if claude { "claude" } else { "!claude" }
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if allow_downgrade {
        command.env(ALLOW_DOWNGRADE_VAR, "1");
    }
    command
}

/// `%LOCALAPPDATA%\Programs\Allot\install.log`, where the setup writes what `allot.exe install`
/// printed.
fn install_log() -> anyhow::Result<PathBuf> {
    let local = env::var_os("LOCALAPPDATA")
        .filter(|value| !value.is_empty())
        .context("LOCALAPPDATA is not set")?;
    Ok(PathBuf::from(local)
        .join("Programs")
        .join("Allot")
        .join("install.log"))
}

/// Runs the setup at `setup` and reads its log, which is printed.
///
/// # Errors
///
/// Fails when the setup cannot start, does not finish in time, exits with a failure, leaves no
/// log from this run, or the log holds an `Error:` line.
pub(super) fn run(setup: &Path, claude: bool, allow_downgrade: bool) -> anyhow::Result<()> {
    run_command(
        command(setup, claude, allow_downgrade),
        &install_log()?,
        SETUP_WAIT,
    )
}

fn run_command(mut command: Command, log: &Path, wait: Duration) -> anyhow::Result<()> {
    let started = SystemTime::now();
    let mut child = command
        .spawn()
        .with_context(|| format!("starting {}", command.get_program().to_string_lossy()))?;
    let deadline = Instant::now() + wait;
    let status = loop {
        if let Some(status) = child.try_wait().context("waiting for the setup")? {
            break status;
        }
        if Instant::now() >= deadline {
            // It is left running: ending a setup half way would leave a worse install.
            bail!(
                "the setup did not finish within {} minutes; it may still be running",
                wait.as_secs() / 60
            );
        }
        thread::sleep(POLL);
    };
    if !status.success() {
        bail!("the setup exited with {status}");
    }
    let text = read_log(log, started)?;
    print!("{text}");
    let errors = error_lines(&text);
    if !errors.is_empty() {
        bail!("allot.exe install failed: {}", errors.join("; "));
    }
    Ok(())
}

/// The log, when this run wrote it: an older one belongs to an earlier install.
fn read_log(log: &Path, started: SystemTime) -> anyhow::Result<String> {
    let written = fs::metadata(log)
        .and_then(|meta| meta.modified())
        .map_err(|_| anyhow::anyhow!("the setup left no log at {}", log.display()))?;
    if written + LOG_SLACK < started {
        bail!(
            "the setup left no new log: {} is from an earlier run",
            log.display()
        );
    }
    let bytes = fs::read(log).with_context(|| format!("reading {}", log.display()))?;
    // Written through cmd's redirection, so in the console's code page rather than UTF-8.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The lines `allot.exe` printed with its `Error:` prefix, as `install.ps1` finds them.
fn error_lines(log: &str) -> Vec<&str> {
    log.lines()
        .map(str::trim)
        .filter(|line| {
            line.get(..6)
                .is_some_and(|start| start.eq_ignore_ascii_case("Error:"))
        })
        .collect()
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

    #[test]
    fn test_command_runs_the_setup_silently_with_install_ps1s_flags() {
        let command = command(Path::new("allot-0.7.0-setup.exe"), true, false);

        assert_eq!(command.get_program(), "allot-0.7.0-setup.exe");
        assert_eq!(
            args(&command),
            [
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/SP-",
                "/MERGETASKS=claude"
            ]
        );
        assert_eq!(command.get_envs().count(), 0);
    }

    #[test]
    fn test_command_unselects_the_claude_task_of_an_install_without_it() {
        let command = command(Path::new("setup.exe"), false, false);

        assert_eq!(
            args(&command).last().map(String::as_str),
            Some("/MERGETASKS=!claude")
        );
    }

    #[test]
    fn test_command_hands_the_downgrade_override_to_the_setup_by_variable() {
        let command = command(Path::new("setup.exe"), true, true);

        let envs: Vec<_> = command.get_envs().collect();
        assert_eq!(
            envs,
            [(OsStr::new(ALLOW_DOWNGRADE_VAR), Some(OsStr::new("1")))]
        );
    }

    #[test]
    fn test_error_lines_finds_what_install_ps1_calls_a_failure() {
        let log = "installed\r\nError: the old allotd did not stop\r\nwarning: x\r\n";

        assert_eq!(error_lines(log), ["Error: the old allotd did not stop"]);
        assert!(error_lines("installed\nan Error: in the middle\n").is_empty());
        assert!(error_lines("").is_empty());
    }

    #[cfg(windows)]
    mod run {
        use std::{fs::File, time::Duration};

        use super::*;
        use crate::test_support::TempDir;

        /// A batch file standing in for the setup: it runs `body`, with `LOG` the log's path.
        fn stand_in(dir: &TempDir, body: &str) -> (Command, PathBuf) {
            let log = dir.path().join("install.log");
            let setup = dir.path().join("setup.bat");
            fs::write(&setup, format!("@echo off\r\n{body}\r\n")).unwrap();
            let mut command = command(&setup, true, false);
            command.env("LOG", &log);
            (command, log)
        }

        fn run_stand_in(body: &str) -> anyhow::Result<()> {
            let dir = TempDir::new();
            let (command, log) = stand_in(&dir, body);
            run_command(command, &log, Duration::from_secs(60))
        }

        #[test]
        fn test_run_command_accepts_a_log_without_errors() {
            run_stand_in("echo installed 0.7.0>\"%LOG%\"\r\nexit /b 0").unwrap();
        }

        #[test]
        fn test_run_command_fails_on_an_error_line_although_the_setup_exits_0() {
            let message = format!(
                "{:#}",
                run_stand_in("echo Error: the old allotd did not stop>\"%LOG%\"\r\nexit /b 0")
                    .unwrap_err()
            );

            assert!(message.contains("the old allotd did not stop"), "{message}");
        }

        #[test]
        fn test_run_command_fails_when_the_setup_exits_with_a_failure() {
            let message = format!("{:#}", run_stand_in("exit /b 5").unwrap_err());

            assert!(message.contains("exited with"), "{message}");
            assert!(message.contains('5'), "{message}");
        }

        #[test]
        fn test_run_command_fails_when_the_setup_left_no_log() {
            let message = format!("{:#}", run_stand_in("exit /b 0").unwrap_err());

            assert!(message.contains("left no log"), "{message}");
        }

        #[test]
        fn test_run_command_does_not_take_an_earlier_runs_log_for_its_own() {
            let dir = TempDir::new();
            let (command, log) = stand_in(&dir, "exit /b 0");
            let old = File::create(&log).unwrap();
            old.set_modified(SystemTime::now() - Duration::from_secs(3_600))
                .unwrap();
            drop(old);

            let message = format!(
                "{:#}",
                run_command(command, &log, Duration::from_secs(60)).unwrap_err()
            );

            assert!(message.contains("from an earlier run"), "{message}");
        }

        #[test]
        fn test_run_command_gives_up_on_a_setup_that_never_ends() {
            let dir = TempDir::new();
            let (command, log) = stand_in(&dir, "ping -n 6 127.0.0.1 >nul");

            let message = format!(
                "{:#}",
                run_command(command, &log, Duration::from_millis(300)).unwrap_err()
            );

            assert!(message.contains("did not finish"), "{message}");
        }

        #[test]
        fn test_run_command_names_a_setup_that_cannot_start() {
            let dir = TempDir::new();
            let log = dir.path().join("install.log");

            let message = format!(
                "{:#}",
                run_command(
                    command(&dir.path().join("missing.exe"), true, false),
                    &log,
                    Duration::from_secs(1)
                )
                .unwrap_err()
            );

            assert!(message.contains("starting"), "{message}");
        }
    }
}
