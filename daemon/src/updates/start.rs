//! Starting `allot update` so that it outlives allotd.
//!
//! The update restarts allotd, and the supervisors allotd runs under end what it started:
//! systemd stops every process of the unit's cgroup, also when allotd just exits, and launchd
//! kills what is left of the job's process group. So the run is detached on each OS the way the
//! trials of M0.2, M0.3 and M2.3 proved:
//!
//! - Windows: a console exe with `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` and no standard
//!   handle of allotd's; the logon task's job object lets it stay.
//! - Linux: its own transient unit, `systemd-run --user --collect`, which no stop of allotd's
//!   unit touches; where no user manager runs, a process of its own group, as on macOS.
//! - macOS: a process of its own group, which launchd leaves alone when the job ends, whether
//!   the job exits by itself, is booted out or is kicked.

use std::{
    ffi::OsString,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
};

/// What to run, and where its output goes.
#[derive(Debug, Clone)]
pub struct UpdateRun {
    /// The `allot` to run.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<OsString>,
    /// The file that receives its output, made new for every run: `println!` of a detached
    /// process with no output handle panics.
    pub log: PathBuf,
}

/// Why the run did not start.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    /// The program is not where it should be.
    #[error("{} does not exist; run `allot install` to put allot there", .0.display())]
    Missing(PathBuf),
    /// The log or the process could not be created.
    #[error("starting {program}: {source}")]
    Io {
        /// The program that did not start.
        program: String,
        /// What failed.
        source: io::Error,
    },
}

/// Starts the run detached and returns once it has been started, not once it ends.
///
/// # Errors
///
/// Fails when the program is missing, the log cannot be created or the process cannot be
/// started.
pub fn launch(run: &UpdateRun) -> Result<(), LaunchError> {
    if !run.program.is_file() {
        return Err(LaunchError::Missing(run.program.clone()));
    }
    let io_error = |source| LaunchError::Io {
        program: run.program.display().to_string(),
        source,
    };
    create_log(&run.log).map_err(io_error)?;
    #[cfg(target_os = "linux")]
    match through_systemd(run) {
        Ok(()) => return Ok(()),
        Err(reason) => tracing::info!("not starting allot update as a systemd unit: {reason}"),
    }
    let child = spawn_detached(run).map_err(io_error)?;
    reap(child, &run.log);
    Ok(())
}

/// The log, empty.
fn create_log(path: &Path) -> io::Result<File> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    File::create(path)
}

/// The command with its input closed and its output in the log.
fn command(run: &UpdateRun) -> io::Result<Command> {
    let log = File::options().append(true).open(&run.log)?;
    let mut command = Command::new(&run.program);
    command
        .args(&run.args)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    Ok(command)
}

#[cfg(windows)]
fn spawn_detached(run: &UpdateRun) -> io::Result<Child> {
    use std::os::windows::process::CommandExt;

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command(run)?
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
}

#[cfg(unix)]
fn spawn_detached(run: &UpdateRun) -> io::Result<Child> {
    use std::os::unix::process::CommandExt;

    command(run)?.process_group(0).spawn()
}

/// Waits for the child on a thread of its own, so it does not stay a zombie while allotd
/// runs, and notes how it ended.
fn reap(mut child: Child, log: &Path) {
    let log = log.to_path_buf();
    let started = thread::Builder::new()
        .name("allot-update-wait".to_owned())
        .spawn(move || match child.wait() {
            Ok(status) if status.success() => tracing::info!("allot update ended: {status}"),
            Ok(status) => {
                tracing::warn!("allot update ended: {status}; its log is {}", log.display())
            }
            Err(error) => tracing::warn!("waiting for allot update: {error}"),
        });
    if let Err(error) = started {
        tracing::warn!("not waiting for allot update: {error}");
    }
}

/// What a transient unit needs of allotd's environment: where the home is, which releases to
/// read, and what `allot update` looks for on the path. The user manager's own environment has
/// none of it for a test, and a test must never update the real install.
#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "tested on every OS, used on Linux")
)]
const FORWARDED: [&str; 5] = [
    "HOME",
    "PATH",
    "XDG_DATA_HOME",
    "XDG_CONFIG_HOME",
    "CARGO_HOME",
];

/// The arguments of `systemd-run` for the run, with `forwarded` environment variables named
/// by their names alone, so systemd takes the values from its own caller.
#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "tested on every OS, used on Linux")
)]
fn systemd_run_args(run: &UpdateRun, forwarded: &[String]) -> Vec<OsString> {
    let log = run.log.display();
    let mut args: Vec<OsString> = [
        "--user".to_owned(),
        "--collect".to_owned(),
        "--quiet".to_owned(),
        "--description=allot update".to_owned(),
        format!("--property=StandardOutput=append:{log}"),
        format!("--property=StandardError=append:{log}"),
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.extend(
        forwarded
            .iter()
            .map(|name| OsString::from(format!("--setenv={name}"))),
    );
    args.push("--".into());
    args.push(run.program.clone().into_os_string());
    args.extend(run.args.iter().cloned());
    args
}

/// The variables of allotd's own environment the run keeps: the fixed few and every
/// `ALLOT_` one.
#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "tested on every OS, used on Linux")
)]
fn forwarded_variables(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut forwarded: Vec<String> = names
        .filter(|name| FORWARDED.contains(&name.as_str()) || name.starts_with("ALLOT_"))
        .collect();
    forwarded.sort();
    forwarded
}

/// Starts the run as a transient unit of the user's systemd, or says why it could not.
#[cfg(target_os = "linux")]
fn through_systemd(run: &UpdateRun) -> Result<(), String> {
    let names = std::env::vars_os().filter_map(|(name, _)| name.into_string().ok());
    let output = Command::new("systemd-run")
        .args(systemd_run_args(run, &forwarded_variables(names)))
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("systemd-run: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let reason = String::from_utf8_lossy(&output.stderr);
    Err(format!("systemd-run {}: {}", output.status, reason.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> UpdateRun {
        UpdateRun {
            program: PathBuf::from("/home/me/.allot/bin/allot"),
            args: vec!["update".into()],
            log: PathBuf::from("/home/me/.allot/logs/update.log"),
        }
    }

    #[test]
    fn test_systemd_run_args_put_a_unit_of_its_own_before_the_command() {
        let args = systemd_run_args(&run(), &["ALLOT_UPDATE_BASE".to_owned(), "HOME".to_owned()]);

        let text: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            text,
            [
                "--user",
                "--collect",
                "--quiet",
                "--description=allot update",
                "--property=StandardOutput=append:/home/me/.allot/logs/update.log",
                "--property=StandardError=append:/home/me/.allot/logs/update.log",
                "--setenv=ALLOT_UPDATE_BASE",
                "--setenv=HOME",
                "--",
                "/home/me/.allot/bin/allot",
                "update",
            ]
        );
    }

    #[test]
    fn test_forwarded_variables_keep_the_home_path_and_every_allot_variable() {
        let names = [
            "HOME",
            "SECRET",
            "ALLOT_UPDATE_BASE",
            "PATH",
            "ALLOT_HOME",
            "LANG",
        ]
        .into_iter()
        .map(str::to_owned);

        assert_eq!(
            forwarded_variables(names),
            ["ALLOT_HOME", "ALLOT_UPDATE_BASE", "HOME", "PATH"]
        );
    }

    #[test]
    fn test_launch_a_missing_program_names_where_allot_should_be() {
        let run = UpdateRun {
            program: PathBuf::from("/nonexistent/allot"),
            ..run()
        };

        let error = launch(&run).unwrap_err();

        assert!(matches!(error, LaunchError::Missing(_)), "{error}");
        assert!(error.to_string().contains("allot install"), "{error}");
    }
}
