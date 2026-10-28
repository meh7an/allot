//! Whether Claude Code's own configuration runs this install: the MCP server in
//! `~/.claude.json` and the hooks in `~/.claude/settings.json`. Paths in those files are
//! compared the way the OS compares them.

use std::path::Path;

use allot_shared::platform::{PATH_STYLE, PathStyle};
use anyhow::Context;
use serde_json::Value;

/// The name the MCP server is registered under.
pub const MCP_NAME: &str = "allot";

/// A hook entry `settings.json` needs: the event and the `allot` arguments it runs.
pub type HookSpec = (&'static str, [&'static str; 2]);

/// Every hook entry: the lease check before a call, and the notices after one.
pub const HOOKS: [HookSpec; 3] = [
    ("PreToolUse", ["hook", "pre-tool"]),
    ("PostToolUse", ["hook", "post-tool"]),
    ("PostToolUseFailure", ["hook", "post-tool"]),
];

/// Whether `~/.claude.json` registers the MCP server, at user scope, to run `exe`.
pub fn mcp_registered(config: &Value, exe: &Path) -> bool {
    config["mcpServers"][MCP_NAME]["command"]
        .as_str()
        .is_some_and(|command| same_path(command, exe))
}

/// Whether `settings.json` has an `event` hook that runs `exe` with `args`.
pub fn hook_registered(settings: &Value, event: &str, args: [&str; 2], exe: &Path) -> bool {
    let Some(entries) = settings["hooks"][event].as_array() else {
        return false;
    };
    entries
        .iter()
        .filter_map(|entry| entry["hooks"].as_array())
        .flatten()
        .any(|hook| is_allot_hook(hook, exe, args))
}

/// Whether one hook of an entry runs `exe` with `args`.
pub fn is_allot_hook(hook: &Value, exe: &Path, args: [&str; 2]) -> bool {
    hook["command"]
        .as_str()
        .is_some_and(|command| same_path(command, exe))
        && hook["args"]
            .as_array()
            .is_some_and(|found| found.iter().map(Value::as_str).eq(args.map(Some)))
}

/// The hook entries `settings` (the file's text, `None` when it does not exist) lacks, in
/// [`HOOKS`] order.
///
/// # Errors
///
/// Fails when the text is not JSON.
pub fn missing_hooks(settings: Option<&str>, exe: &Path) -> anyhow::Result<Vec<HookSpec>> {
    let settings: Value = match settings {
        Some(text) if !text.trim().is_empty() => {
            serde_json::from_str(text).context("the file is not valid JSON")?
        }
        _ => Value::Null,
    };
    Ok(HOOKS
        .into_iter()
        .filter(|&(event, args)| !hook_registered(&settings, event, args, exe))
        .collect())
}

/// Whether the path in `text`, quoted or not, names `path` on this OS.
pub fn same_path(text: &str, path: &Path) -> bool {
    same_path_in(PATH_STYLE, text, &path.to_string_lossy())
}

/// Windows ignores case and takes either separator, macOS ignores case, and Linux compares
/// paths as they are spelled.
fn same_path_in(style: PathStyle, text: &str, path: &str) -> bool {
    let normalize = |text: &str| {
        let text = text.trim_matches('"');
        match style {
            PathStyle::Windows => text.replace('/', "\\").to_lowercase(),
            PathStyle::Macos => text.to_lowercase(),
            PathStyle::Linux => text.to_owned(),
        }
    };
    normalize(text) == normalize(path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use allot_shared::platform::{CLI_EXE, MCP_EXE};
    use serde_json::json;

    use super::*;

    fn bin() -> PathBuf {
        let home = if cfg!(windows) {
            r"C:\Users\someone"
        } else {
            "/home/someone"
        };
        PathBuf::from(home).join(".allot").join("bin")
    }

    #[test]
    fn test_same_path_in_windows_ignores_case_and_separator() {
        let path = r"C:\Users\someone\.allot\bin\allot.exe";
        assert!(same_path_in(
            PathStyle::Windows,
            "\"c:/users/SOMEONE/.allot/bin/allot.exe\"",
            path
        ));
        assert!(!same_path_in(
            PathStyle::Windows,
            r"C:\other\allot.exe",
            path
        ));
    }

    #[test]
    fn test_same_path_in_macos_ignores_case_only() {
        let path = "/Users/someone/.allot/bin/allot";
        assert!(same_path_in(
            PathStyle::Macos,
            "/users/SOMEONE/.allot/bin/allot",
            path
        ));
        assert!(!same_path_in(
            PathStyle::Macos,
            r"\Users\someone\.allot\bin\allot",
            path
        ));
    }

    #[test]
    fn test_same_path_in_linux_is_case_sensitive() {
        let path = "/home/someone/.allot/bin/allot";
        assert!(same_path_in(
            PathStyle::Linux,
            "\"/home/someone/.allot/bin/allot\"",
            path
        ));
        assert!(!same_path_in(
            PathStyle::Linux,
            "/home/Someone/.allot/bin/allot",
            path
        ));
        assert!(!same_path_in(
            PathStyle::Linux,
            r"\home\someone\.allot\bin\allot",
            path
        ));
    }

    #[test]
    fn test_mcp_registered_finds_this_install_at_user_scope() {
        let exe = bin().join(MCP_EXE);
        let user = json!({ "mcpServers": { "allot": { "command": exe } } });
        let project_only = json!({
            "projects": { "/repo": { "mcpServers": { "allot": { "command": exe } } } },
        });
        assert!(mcp_registered(&user, &exe));
        assert!(!mcp_registered(&project_only, &exe));
        assert!(!mcp_registered(&user, &bin().join("other")));
    }

    #[test]
    fn test_missing_hooks_lists_the_events_without_this_install() {
        let exe = bin().join(CLI_EXE);
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash|PowerShell",
                    "hooks": [{ "type": "command", "command": exe, "args": ["hook", "pre-tool"] }],
                }],
                "PostToolUse": [{
                    "hooks": [{ "command": "/elsewhere/allot", "args": ["hook", "post-tool"] }],
                }],
            },
        });
        let missing = missing_hooks(Some(&settings.to_string()), &exe).unwrap();
        assert_eq!(missing, HOOKS[1..]);
        assert_eq!(missing_hooks(None, &exe).unwrap(), HOOKS);
        assert!(missing_hooks(Some("{"), &exe).is_err());
    }
}
