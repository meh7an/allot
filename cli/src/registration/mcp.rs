//! Registers the MCP server through Claude Code's own `claude mcp` commands rather than by
//! editing `~/.claude.json`, which running sessions rewrite all the time.

use std::{
    env,
    ffi::{OsStr, OsString},
    io,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, bail};

use super::MCP_NAME;

/// Overrides the `claude` command, for an install that is not on `PATH`.
pub(super) const CLAUDE_VAR: &str = "RH_CLAUDE";

/// `claude mcp add` at user scope for `exe`; with `replace`, the entry of the same name (which
/// runs another exe) is removed first. Returns false when `claude` cannot be found.
///
/// # Errors
///
/// Fails when a command fails.
pub(super) fn add(exe: &Path, replace: bool) -> anyhow::Result<bool> {
    if replace && !remove()? {
        return Ok(false);
    }
    claude(&[
        "mcp".as_ref(),
        "add".as_ref(),
        "--scope".as_ref(),
        "user".as_ref(),
        MCP_NAME.as_ref(),
        "--".as_ref(),
        exe.as_os_str(),
    ])
}

/// `claude mcp remove` at user scope. Returns false when `claude` cannot be found.
///
/// # Errors
///
/// Fails when the command fails.
pub(super) fn remove() -> anyhow::Result<bool> {
    claude(&[
        "mcp".as_ref(),
        "remove".as_ref(),
        "--scope".as_ref(),
        "user".as_ref(),
        MCP_NAME.as_ref(),
    ])
}

/// Runs `claude` with `args`: the native install (`claude.exe`), then the npm one
/// (`claude.cmd`), unless [`CLAUDE_VAR`] names one. Returns false when none is found.
fn claude(args: &[&OsStr]) -> anyhow::Result<bool> {
    let programs: Vec<OsString> = match env::var_os(CLAUDE_VAR) {
        Some(program) => vec![program],
        None => vec!["claude".into(), "claude.cmd".into()],
    };
    let shown = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    for program in programs {
        let output = match Command::new(&program)
            .args(args)
            .stdin(Stdio::null())
            .output()
        {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            result => result.with_context(|| format!("running claude {shown}"))?,
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let reason = if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            };
            bail!("claude {shown} failed: {}", reason.trim());
        }
        return Ok(true);
    }
    Ok(false)
}
