//! The systemd user unit that starts allotd at login, managed with `systemctl --user`. The
//! user manager runs from the user's first login to the last logout, and allotd with it, as
//! with the logon task on Windows. Once the unit exists, the client starts allotd through it
//! too, so allotd always runs in the unit's cgroup and never in a lease's scope.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use allot_shared::{
    autostart::{
        CONFIG_HOME_VAR, SYSTEMD_UNIT, systemd_unit, systemd_unit_exec, systemd_unit_file,
    },
    paths::Paths,
};
use anyhow::{Context, bail};

use super::{autostart::Found, read_text, write_atomic};

/// The file whose text names the kernel; WSL's says `microsoft`.
const OS_RELEASE: &str = "/proc/sys/kernel/osrelease";

fn unit_file(paths: &Paths) -> PathBuf {
    systemd_unit_file(paths.home(), env::var_os(CONFIG_HOME_VAR).as_deref())
}

/// The setup report's title for the unit.
pub(super) fn title(paths: &Paths) -> String {
    format!(
        "allotd at login, systemd user unit {}",
        unit_file(paths).display()
    )
}

/// Writes the unit for `allotd` where it differs, enables it, and starts allotd (a no-op when
/// it runs). A unit of the same name that starts another allotd, such as a development build,
/// is replaced. Returns what changed, `None` when nothing did.
///
/// # Errors
///
/// Fails when the unit cannot be written or `systemctl --user` fails, such as without a user
/// manager.
pub(super) fn set_up(paths: &Paths, allotd: &Path) -> anyhow::Result<Option<String>> {
    let file = unit_file(paths);
    let unit = systemd_unit(allotd);
    let old = read_text(&file)?;
    let rewritten = old.as_deref() != Some(unit.as_str());
    if rewritten {
        write_atomic(&file, &unit)?;
        systemctl(&["daemon-reload"])?;
    }
    let was_enabled = is_enabled();
    systemctl(&["enable", "--now", SYSTEMD_UNIT])?;
    Ok(if old.is_none() {
        Some("registered and started".to_owned())
    } else if rewritten {
        Some(found(old.as_deref()).replaced_note(allotd, "updated and started"))
    } else if !was_enabled {
        Some("enabled and started".to_owned())
    } else {
        None
    })
}

/// How to set the unit up by hand: write it where it is missing or differs, then enable it.
pub(super) fn manual_set_up(paths: &Paths, allotd: &Path) -> Vec<String> {
    let file = unit_file(paths);
    let unit = systemd_unit(allotd);
    let mut manual = Vec::new();
    if read_text(&file).ok().flatten().as_deref() != Some(unit.as_str()) {
        manual.push(format!("Write this to {}:", file.display()));
        manual.extend(unit.lines().map(|line| format!("  {line}")));
    }
    manual.push("systemctl --user daemon-reload".to_owned());
    manual.push(format!("systemctl --user enable --now {SYSTEMD_UNIT}"));
    if is_wsl() {
        manual.push(
            "On WSL, turn systemd on first: add [boot] systemd=true to /etc/wsl.conf, then run \
             wsl --shutdown in Windows."
                .to_owned(),
        );
    }
    manual
}

/// Stops and disables the unit and deletes it, when it starts `allotd`; one for another allotd
/// is not ours to remove. Returns what was removed.
///
/// # Errors
///
/// Fails when `systemctl --user` fails or the file cannot be deleted.
pub(super) fn remove(paths: &Paths, allotd: &Path) -> anyhow::Result<Option<String>> {
    let file = unit_file(paths);
    if !found(read_text(&file)?.as_deref()).starts(allotd) {
        return Ok(None);
    }
    systemctl(&["disable", "--now", SYSTEMD_UNIT])?;
    fs::remove_file(&file).with_context(|| format!("deleting {}", file.display()))?;
    // Only forgets the deleted unit; allotd is already stopped.
    let _ = systemctl(&["daemon-reload"]);
    Ok(Some(format!(
        "removed the systemd user unit {}",
        file.display()
    )))
}

/// How to remove the unit by hand.
pub(super) fn manual_remove(paths: &Paths) -> String {
    format!(
        "systemctl --user disable --now {SYSTEMD_UNIT}, then delete {}",
        unit_file(paths).display()
    )
}

fn found(unit: Option<&str>) -> Found {
    match unit {
        None => Found::Missing,
        Some(unit) => systemd_unit_exec(unit).map_or(Found::Unknown, Found::Runs),
    }
}

/// Whether the unit starts at login; `false` when `systemctl` cannot tell.
fn is_enabled() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-enabled", SYSTEMD_UNIT])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "enabled")
}

fn systemctl(args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("running systemctl --user")?;
    if !output.status.success() {
        bail!(
            "systemctl --user {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn is_wsl() -> bool {
    fs::read_to_string(OS_RELEASE)
        .is_ok_and(|release| release.to_ascii_lowercase().contains("microsoft"))
}
