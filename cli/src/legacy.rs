//! Taking over an install from before the rename to Allot, when it was called Resources Helper
//! and lived in `%USERPROFILE%\.resources-helper\`. Its data is copied, not moved: sessions that
//! are open during the upgrade keep their old MCP server, which reads the old token and config,
//! until they restart, and the new allotd keeps the old port and token. A later install or
//! uninstall deletes what is left of the old folder once nothing runs from it.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use allot_client::{
    BlockingClient, ClientOptions, DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT, TokenSource,
};
use allot_shared::{config::Config, paths::Paths};
use anyhow::{Context, bail};

use crate::{
    daemon,
    install::{self, Successor},
    tray,
};

/// The old data folder in the home directory.
const DIR: &str = ".resources-helper";
/// The name Claude Code knew the old MCP server by.
pub const MCP_NAME: &str = "resources-helper";
/// The old logon task.
pub const TASK_NAME: &str = "resources-helper";
/// The old rule file beside CLAUDE.md, which CLAUDE.md imported.
pub const RULE_FILE: &str = "resources-helper.md";
pub const CLI_EXE: &str = "rh.exe";
pub const DAEMON_EXE: &str = "rhd.exe";
pub const MCP_EXE: &str = "rh-mcp.exe";
const TRAY_EXE: &str = "rh-tray.exe";
const LOCK_FILE: &str = "rhd.lock";
const LOG_DIR: &str = "logs";
/// The old tray app's Launch at logon, a value under this key.
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Resources Helper";
/// The files open sessions' old MCP servers read; they stay until nothing runs from the folder.
const SHARED: [&str; 2] = ["config.toml", "token"];
/// The data files and their new names. Log files are renamed by [`new_log_name`].
const DATA: [(&str, &str); 4] = [
    ("rh.db", "allot.db"),
    ("rh.db-wal", "allot.db-wal"),
    ("rh.db-shm", "allot.db-shm"),
    ("tray.json", "tray.json"),
];

/// The old folder under the home directory of `paths`.
pub fn root(paths: &Paths) -> PathBuf {
    paths.home().join(DIR)
}

/// Where the old install's executables live.
pub fn bin_dir(paths: &Paths) -> PathBuf {
    root(paths).join("bin")
}

/// What [`take_over`] found of the old install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakenOver {
    /// The old daemon was running and was stopped, so the new one starts in its place.
    pub daemon_stopped: bool,
    /// Whether the old tray app started at logon; `None` when it was not installed.
    pub launch_at_logon: Option<bool>,
}

/// Quits the old tray app and turns off its Launch at logon, stops the old daemon so that open
/// sessions cannot start it again, and copies the data into the new folder under the new names.
/// Returns `None` when there is no old install.
///
/// # Errors
///
/// Fails when the old daemon does not stop or a file cannot be copied.
pub fn take_over(paths: &Paths) -> anyhow::Result<Option<TakenOver>> {
    let old = root(paths);
    if !old.is_dir() {
        return Ok(None);
    }
    println!(
        "taking over the Resources Helper install in {}",
        old.display()
    );
    let tray = bin_dir(paths).join("tray").join(TRAY_EXE);
    let launch_at_logon = tray.is_file().then(|| starts_at_logon(&tray));
    if tray.is_file() {
        // The tray app owns its Run entry and its toast registration; quitting drops the latter.
        if let Err(error) = tray::set_launch_at_logon(&tray, false) {
            println!("warning: the old tray app's Launch at logon was not turned off ({error:#})");
        }
        if !tray::quit(&tray) {
            println!("warning: the old tray app did not quit; quit it from its tray menu");
        }
    }
    let daemon_stopped = stop_daemon(paths)?;
    copy_data(&old, paths.root())?;
    Ok(Some(TakenOver {
        daemon_stopped,
        launch_at_logon,
    }))
}

/// Deletes the old executables that nothing runs any more, and the whole old folder once only
/// the files open sessions read are left in it. Returns the files still in use.
pub fn clean_up(paths: &Paths) -> Vec<PathBuf> {
    let old = root(paths);
    if !old.is_dir() {
        return Vec::new();
    }
    let in_use = tray::remove(&bin_dir(paths));
    if in_use.is_empty() && only_shared_left(&old) {
        match fs::remove_dir_all(&old) {
            Ok(()) => println!("removed {}", old.display()),
            Err(error) => println!("warning: {} was not removed ({error})", old.display()),
        }
    } else if in_use.is_empty() {
        println!(
            "kept {}: it holds files that were not copied, because the new folder has its own",
            old.display()
        );
    }
    in_use
}

/// Whether the old tray app `exe` is the one the Run key starts at logon.
fn starts_at_logon(exe: &Path) -> bool {
    let output = Command::new("reg")
        .args(["query", RUN_KEY, "/v", RUN_VALUE])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    output.is_ok_and(|output| {
        output.status.success()
            && String::from_utf8_lossy(&output.stdout)
                .to_lowercase()
                .contains(&exe.to_string_lossy().to_lowercase())
    })
}

/// Stops the old daemon, after renaming its exe aside so that open sessions' old MCP servers,
/// which start it from there, cannot start it again. Returns whether it was running.
fn stop_daemon(paths: &Paths) -> anyhow::Result<bool> {
    let old = root(paths);
    let exe = bin_dir(paths).join(DAEMON_EXE);
    let aside = install::aside_path(&exe);
    let renamed = exe.is_file()
        && fs::rename(&exe, &aside)
            .with_context(|| format!("renaming {} aside", exe.display()))
            .map(|()| true)?;
    let stopped = stop_daemon_at(&old);
    if stopped.is_err() && renamed {
        let _ = fs::rename(&aside, &exe);
    }
    stopped
}

fn stop_daemon_at(old: &Path) -> anyhow::Result<bool> {
    let config = Config::load(&old.join("config.toml")).unwrap_or_else(|error| {
        eprintln!("warning: {error}; using the default port");
        Config::default()
    });
    let client = BlockingClient::new(ClientOptions {
        base_url: format!("http://127.0.0.1:{}", config.daemon.port),
        token: TokenSource::File(old.join("token")),
        connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        request_timeout: DEFAULT_REQUEST_TIMEOUT,
        spawn: None,
    })?;
    let Some(old_pid) = daemon::shutdown(&client)? else {
        return Ok(false);
    };
    if let Successor::Running { pid } =
        install::wait_for_exit(&old.join(LOCK_FILE), &client, old_pid)?
    {
        bail!("a daemon started again on the old port (pid {pid}); close open Claude sessions");
    }
    println!("stopped the old daemon (pid {old_pid})");
    Ok(true)
}

/// Copies the data from `old` into `new`, keeping whatever `new` already has, and deletes the
/// old copies of everything but the shared files. A file `new` already has stays in `old`.
fn copy_data(old: &Path, new: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(new).with_context(|| format!("creating {}", new.display()))?;
    let mut copied = Vec::new();
    for name in SHARED {
        copy_new(&old.join(name), &new.join(name), &mut copied)?;
    }
    for (from, to) in DATA {
        if copy_new(&old.join(from), &new.join(to), &mut copied)? {
            remove(&old.join(from))?;
        }
    }
    let logs = old.join(LOG_DIR);
    for name in file_names(&logs)? {
        let target = new.join(LOG_DIR).join(new_log_name(&name));
        if copy_new(&logs.join(&name), &target, &mut copied)? {
            remove(&logs.join(&name))?;
        }
    }
    let _ = fs::remove_dir(&logs);
    if !copied.is_empty() {
        println!("copied {} into {}", copied.join(", "), new.display());
    }
    Ok(())
}

/// Copies `source` to `target` unless `source` is missing or `target` exists. Returns whether
/// `source` is now safe to delete: copied, or missing.
fn copy_new(source: &Path, target: &Path, copied: &mut Vec<String>) -> anyhow::Result<bool> {
    if !source.is_file() {
        return Ok(true);
    }
    if target.exists() {
        println!(
            "kept {}: {} exists already",
            source.display(),
            target.display()
        );
        return Ok(false);
    }
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::copy(source, target)
        .with_context(|| format!("copying {} to {}", source.display(), target.display()))?;
    copied.push(
        source
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    );
    Ok(true)
}

/// `rhd.log` and its rotations `rhd.<n>.log` become `allotd.log` and `allotd.<n>.log`; other
/// logs keep their names.
fn new_log_name(name: &str) -> String {
    match name.strip_prefix("rhd.") {
        Some(rest) => format!("allotd.{rest}"),
        None => name.to_owned(),
    }
}

fn file_names(dir: &Path) -> anyhow::Result<Vec<String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", dir.display())),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(names)
}

fn remove(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("removing {}", path.display()))
        }
        _ => Ok(()),
    }
}

/// Whether `old` holds nothing but the shared files.
fn only_shared_left(old: &Path) -> bool {
    fs::read_dir(old).is_ok_and(|entries| {
        entries.filter_map(Result::ok).all(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && SHARED.iter().any(|name| entry.file_name() == *name)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_log_name_renames_the_daemon_logs_only() {
        assert_eq!(new_log_name("rhd.log"), "allotd.log");
        assert_eq!(new_log_name("rhd.3.log"), "allotd.3.log");
        assert_eq!(new_log_name("tray.log"), "tray.log");
    }

    #[test]
    fn test_copy_data_keeps_the_shared_files_and_what_the_new_folder_has() {
        let dir = std::env::temp_dir().join(format!("allot-legacy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (old, new) = (dir.join("old"), dir.join("new"));
        fs::create_dir_all(old.join(LOG_DIR)).unwrap();
        for name in ["token", "config.toml", "rh.db", "tray.json"] {
            fs::write(old.join(name), format!("old {name}")).unwrap();
        }
        fs::write(old.join(LOG_DIR).join("rhd.1.log"), "log").unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join("tray.json"), "new tray.json").unwrap();

        copy_data(&old, &new).unwrap();

        assert_eq!(
            fs::read_to_string(new.join("allot.db")).unwrap(),
            "old rh.db"
        );
        assert_eq!(fs::read_to_string(new.join("token")).unwrap(), "old token");
        assert_eq!(
            fs::read_to_string(new.join(LOG_DIR).join("allotd.1.log")).unwrap(),
            "log"
        );
        assert_eq!(
            fs::read_to_string(new.join("tray.json")).unwrap(),
            "new tray.json"
        );
        assert!(old.join("token").is_file() && old.join("config.toml").is_file());
        assert!(!old.join("rh.db").exists() && !old.join(LOG_DIR).exists());
        assert!(
            old.join("tray.json").is_file(),
            "not copied, so not deleted"
        );
        assert!(!only_shared_left(&old));
        fs::remove_file(old.join("tray.json")).unwrap();
        assert!(only_shared_left(&old));
        let _ = fs::remove_dir_all(&dir);
    }
}
