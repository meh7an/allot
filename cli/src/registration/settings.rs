//! Adds and removes allot's hook entries in `~/.claude/settings.json`. The file is edited as an
//! ordered tree, so its key order and every entry allot does not own come back unchanged.

use std::path::Path;

use anyhow::Context;

use super::{
    HOOKS, HookSpec, is_allot_hook,
    json::{Json, parse, text},
};

/// A hook entry that runs `command` with `args`, keys in the order of Claude Code's examples.
/// Exec form (`args`, no shell), so the hook's parent is the session's `claude.exe`.
fn hook_entry(command: &str, args: [&str; 2]) -> Json {
    Json::object(vec![
        ("matcher", Json::String("Bash|PowerShell".to_owned())),
        (
            "hooks",
            Json::Array(vec![Json::object(vec![
                ("type", Json::String("command".to_owned())),
                ("command", Json::String(command.to_owned())),
                (
                    "args",
                    Json::Array(args.map(|arg| Json::String(arg.to_owned())).to_vec()),
                ),
                ("timeout", Json::Number(10.into())),
            ])]),
        ),
    ])
}

/// [`hook_entry`] as pretty JSON, for printing.
pub(super) fn hook_entry_text(command: &str, args: [&str; 2]) -> String {
    serde_json::to_string_pretty(&hook_entry(command, args)).unwrap_or_default()
}

/// `settings` (the file's text, empty when it does not exist) with an entry for each of
/// `missing` appended to its event's array.
///
/// # Errors
///
/// Fails when the text is not JSON, or `hooks` or an event is not of the expected type.
pub(super) fn add_hooks(
    settings: &str,
    missing: &[HookSpec],
    command: &str,
) -> anyhow::Result<String> {
    let mut tree = parse(settings)?;
    let hooks = tree
        .entry("hooks", Json::Object(Vec::new()))
        .context("the file is not a JSON object")?;
    for &(event, args) in missing {
        hooks
            .entry(event, Json::Array(Vec::new()))
            .and_then(Json::array_mut)
            .with_context(|| format!("\"hooks\" or \"hooks.{event}\" has an unexpected type"))?
            .push(hook_entry(command, args));
    }
    Ok(text(&tree, settings))
}

/// `settings` without the hooks that run `exe` with allot's arguments, dropping entries and
/// events left empty by that; `None` when it has none.
///
/// # Errors
///
/// Fails when the text is not JSON.
pub(super) fn remove_hooks(settings: &str, exe: &Path) -> anyhow::Result<Option<String>> {
    let mut tree = parse(settings)?;
    let Some(hooks) = tree.field_mut("hooks").and_then(Json::object_mut) else {
        return Ok(None);
    };
    let mut changed = false;
    for (event, args) in HOOKS {
        let Some(index) = hooks.iter().position(|(name, _)| name == event) else {
            continue;
        };
        let Some(entries) = hooks[index].1.array_mut() else {
            continue;
        };
        let mut emptied_by_allot = false;
        entries.retain_mut(|entry| {
            let Some(list) = entry.field_mut("hooks").and_then(Json::array_mut) else {
                return true;
            };
            let before = list.len();
            list.retain(|hook| !runs_allot(hook, exe, args));
            if list.len() == before {
                return true;
            }
            changed = true;
            emptied_by_allot |= list.is_empty();
            !list.is_empty()
        });
        if emptied_by_allot && entries.is_empty() {
            hooks.remove(index);
        }
    }
    Ok(changed.then(|| text(&tree, settings)))
}

fn runs_allot(hook: &Json, exe: &Path, args: [&str; 2]) -> bool {
    serde_json::to_value(hook).is_ok_and(|hook| is_allot_hook(&hook, exe, args))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[cfg(windows)]
    const COMMAND: &str = "C:/Users/someone/.allot/bin/allot.exe";
    #[cfg(not(windows))]
    const COMMAND: &str = "/home/someone/.allot/bin/allot";
    /// The command as a hand-edited file may spell it, JSON-escaped: Windows ignores case and
    /// separators.
    #[cfg(windows)]
    const SPELLED: &str = r"c:\\users\\someone\\.allot\\bin\\allot.exe";
    #[cfg(not(windows))]
    const SPELLED: &str = COMMAND;

    fn exe() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"C:\Users\someone\.allot\bin\allot.exe")
        } else {
            PathBuf::from(COMMAND)
        }
    }

    #[test]
    fn test_add_hooks_keeps_key_order_and_other_hooks() {
        let settings = r#"{
  "model": "opus",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "rtk hook"
          }
        ]
      }
    ]
  },
  "env": {
    "Z": "1",
    "A": "2"
  }
}
"#;
        let updated = add_hooks(settings, &HOOKS, COMMAND).unwrap();

        let model = updated.find("\"model\"").unwrap();
        let hooks = updated.find("\"hooks\"").unwrap();
        let env = updated.find("\"env\"").unwrap();
        assert!(model < hooks && hooks < env, "{updated}");
        assert!(updated.find("\"Z\"") < updated.find("\"A\""), "{updated}");
        assert!(updated.ends_with("}\n"), "{updated}");

        let tree: serde_json::Value = serde_json::from_str(&updated).unwrap();
        let pre_tool = tree["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool.len(), 2);
        assert_eq!(pre_tool[0]["hooks"][0]["command"], "rtk hook");
        for (event, args) in HOOKS {
            assert!(
                crate::registered::hook_registered(&tree, event, args, &exe()),
                "{event}"
            );
        }
    }

    #[test]
    fn test_add_hooks_to_a_missing_file_and_back_out_again() {
        let added = add_hooks("", &HOOKS, COMMAND).unwrap();
        let entry = added.find("\"matcher\"").unwrap();
        assert!(entry < added.find("\"timeout\"").unwrap(), "{added}");

        let removed = remove_hooks(&added, &exe()).unwrap().unwrap();
        assert_eq!(removed, "{\n  \"hooks\": {}\n}\n");
        assert_eq!(remove_hooks(&removed, &exe()).unwrap(), None);
    }

    #[test]
    fn test_remove_hooks_leaves_other_hooks_in_a_shared_entry_and_crlf() {
        let settings = "{\r\n  \"hooks\": {\r\n    \"PreToolUse\": [\r\n      {\r\n        \
            \"matcher\": \"Bash|PowerShell\",\r\n        \"hooks\": [\r\n          \
            { \"command\": \"rtk hook\" },\r\n          \
            { \"command\": \"SPELLED\", \
            \"args\": [\"hook\", \"pre-tool\"] }\r\n        ]\r\n      }\r\n    ]\r\n  }\r\n}"
            .replace("SPELLED", SPELLED);
        let settings = settings.as_str();

        let removed = remove_hooks(settings, &exe()).unwrap().unwrap();

        let tree: serde_json::Value = serde_json::from_str(&removed).unwrap();
        assert_eq!(
            tree["hooks"]["PreToolUse"],
            serde_json::json!([{
                "matcher": "Bash|PowerShell",
                "hooks": [{ "command": "rtk hook" }],
            }])
        );
        assert!(
            removed.contains("\r\n") && !removed.ends_with('\n'),
            "{removed:?}"
        );
    }

    #[test]
    fn test_add_hooks_refuses_what_it_cannot_edit() {
        for settings in [
            "not json",
            "[]",
            r#"{ "hooks": [] }"#,
            r#"{ "hooks": { "PreToolUse": {} } }"#,
        ] {
            assert!(add_hooks(settings, &HOOKS, COMMAND).is_err(), "{settings}");
        }
    }
}
