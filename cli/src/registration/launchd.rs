//! The launchd LaunchAgent that starts allotd at login, managed with `launchctl` in the user's
//! GUI domain. launchd loads the agents in `~/Library/LaunchAgents` at login and starts allotd
//! then (`RunAtLoad`), as the logon task does on Windows.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use allot_shared::{
    autostart::{LAUNCHD_LABEL, launch_agent, launch_agent_file, launch_agent_program},
    paths::Paths,
};
use anyhow::{Context, bail};

use super::{autostart::Found, read_text, write_atomic};

/// How often to retry loading the agent right after unloading it: launchd can refuse with an
/// I/O error while the old job is still being torn down.
const BOOTSTRAP_TRIES: u32 = 5;
const BOOTSTRAP_RETRY: Duration = Duration::from_millis(200);

fn plist_file(paths: &Paths) -> PathBuf {
    launch_agent_file(paths.home())
}

/// The user's GUI domain, where agents of a login session live.
fn domain() -> String {
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    format!("gui/{uid}")
}

fn service() -> String {
    format!("{}/{LAUNCHD_LABEL}", domain())
}

/// The setup report's title for the agent.
pub(super) fn title(paths: &Paths) -> String {
    format!(
        "allotd at login, LaunchAgent {}",
        plist_file(paths).display()
    )
}

/// Writes the agent for `allotd` where it differs, loads it (which starts allotd), or starts
/// allotd when it is loaded already (a no-op when it runs). An agent of the same label that
/// starts another allotd, such as a development build, is replaced. Returns what changed,
/// `None` when nothing did.
///
/// # Errors
///
/// Fails when the plist cannot be written or `launchctl` fails, such as outside a GUI login.
pub(super) fn set_up(paths: &Paths, allotd: &Path) -> anyhow::Result<Option<String>> {
    let file = plist_file(paths);
    let plist = launch_agent(allotd);
    let old = read_text(&file)?;
    let rewritten = old.as_deref() != Some(plist.as_str());
    let mut loaded = is_loaded();
    if rewritten {
        write_atomic(&file, &plist)?;
        if loaded {
            launchctl(&["bootout", &service()])?;
            loaded = false;
        }
    }
    let was_loaded = loaded;
    if loaded {
        launchctl(&["kickstart", &service()])?;
    } else {
        bootstrap(&file, rewritten && old.is_some())?;
    }
    Ok(if old.is_none() {
        Some("registered and started".to_owned())
    } else if rewritten {
        Some(found(old.as_deref()).replaced_note(allotd, "updated and started"))
    } else if !was_loaded {
        Some("loaded and started".to_owned())
    } else {
        None
    })
}

/// How to set the agent up by hand: write it where it is missing or differs, then load it.
pub(super) fn manual_set_up(paths: &Paths, allotd: &Path) -> Vec<String> {
    let file = plist_file(paths);
    let plist = launch_agent(allotd);
    let mut manual = Vec::new();
    if read_text(&file).ok().flatten().as_deref() != Some(plist.as_str()) {
        manual.push(format!("Write this to {}:", file.display()));
        manual.extend(plist.lines().map(|line| format!("  {line}")));
    }
    manual.push(format!(
        "launchctl bootstrap {} \"{}\"",
        domain(),
        file.display()
    ));
    manual.push(format!("launchctl kickstart {}", service()));
    manual
}

/// Unloads the agent, which stops allotd, and deletes it, when it starts `allotd`; one for
/// another allotd is not ours to remove. Returns what was removed.
///
/// # Errors
///
/// Fails when `launchctl` fails or the file cannot be deleted.
pub(super) fn remove(paths: &Paths, allotd: &Path) -> anyhow::Result<Option<String>> {
    let file = plist_file(paths);
    if !found(read_text(&file)?.as_deref()).starts(allotd) {
        return Ok(None);
    }
    if is_loaded() {
        launchctl(&["bootout", &service()])?;
    }
    fs::remove_file(&file).with_context(|| format!("deleting {}", file.display()))?;
    Ok(Some(format!("removed the LaunchAgent {}", file.display())))
}

/// How to remove the agent by hand.
pub(super) fn manual_remove(paths: &Paths) -> String {
    format!(
        "launchctl bootout {}, then delete {}",
        service(),
        plist_file(paths).display()
    )
}

fn found(plist: Option<&str>) -> Found {
    match plist {
        None => Found::Missing,
        Some(plist) => launch_agent_program(plist).map_or(Found::Unknown, Found::Runs),
    }
}

/// Loads the agent from `file`. After an unload (`retry`), launchd may still be tearing the old
/// job down, so a failure is retried a few times.
fn bootstrap(file: &Path, retry: bool) -> anyhow::Result<()> {
    let file = file.to_string_lossy();
    let tries = if retry { BOOTSTRAP_TRIES } else { 1 };
    let mut attempt = 1;
    loop {
        match launchctl(&["bootstrap", &domain(), &file]) {
            Err(_) if attempt < tries => {
                attempt += 1;
                thread::sleep(BOOTSTRAP_RETRY);
            }
            result => return result,
        }
    }
}

/// Whether launchd has the agent loaded in the GUI domain.
fn is_loaded() -> bool {
    Command::new("launchctl")
        .args(["print", &service()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn launchctl(args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("launchctl")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("running launchctl")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let reason = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        };
        bail!("launchctl {} failed: {}", args.join(" "), reason.trim());
    }
    Ok(())
}
