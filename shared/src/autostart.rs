//! The files that start allotd at login on Linux and macOS: a systemd user unit and a launchd
//! LaunchAgent. `allot install` writes them, and on Linux the client starts allotd through the
//! unit, so both read them the same way. Neither restarts allotd when it exits: the next
//! session that needs it starts it again, as on Windows.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

/// The systemd user unit's name.
pub const SYSTEMD_UNIT: &str = "allotd.service";
/// The LaunchAgent's label, which also names its plist.
pub const LAUNCHD_LABEL: &str = "tech.mehran.allotd";
/// Overrides `~/.config` as the base of systemd's user unit directory.
pub const CONFIG_HOME_VAR: &str = "XDG_CONFIG_HOME";

const EXEC_START: &str = "ExecStart=";
const PROGRAM_ARGUMENTS: &str = "<key>ProgramArguments</key>";

/// The user unit file under `home`, in `config_home` (the value of [`CONFIG_HOME_VAR`]) when it
/// is an absolute path, as the XDG spec requires, and in `~/.config` otherwise.
pub fn systemd_unit_file(home: &Path, config_home: Option<&OsStr>) -> PathBuf {
    let base = config_home
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .map_or_else(|| home.join(".config"), Path::to_owned);
    base.join("systemd").join("user").join(SYSTEMD_UNIT)
}

/// The LaunchAgent's plist under `home`.
pub fn launch_agent_file(home: &Path) -> PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist"))
}

/// The user unit that starts `allotd` when the user manager starts, at login. `Type=exec`
/// makes `systemctl start` fail when the file cannot be run, so the client can fall back.
pub fn systemd_unit(allotd: &Path) -> String {
    let exec = escape_exec(&allotd.to_string_lossy());
    format!(
        "# Written by allot install; allot uninstall removes it.\n\
         [Unit]\n\
         Description=Allot daemon (allotd)\n\
         Documentation=https://github.com/meh7an/allot\n\
         \n\
         [Service]\n\
         Type=exec\n\
         {EXEC_START}\"{exec}\"\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

/// The executable a user unit starts, from its first `ExecStart=` line, or `None` when it has
/// none.
pub fn systemd_unit_exec(unit: &str) -> Option<String> {
    let value = unit
        .lines()
        .find_map(|line| line.trim().strip_prefix(EXEC_START))?
        .trim();
    let raw = match value.strip_prefix('"') {
        Some(quoted) => unquote_exec(quoted)?,
        None => value.split_whitespace().next()?.to_owned(),
    };
    Some(raw.replace("%%", "%").replace("$$", "$"))
}

/// The LaunchAgent that starts `allotd` when it is loaded, which launchd does at login.
pub fn launch_agent(allotd: &Path) -> String {
    let program = xml_escape(&allotd.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LAUNCHD_LABEL}</string>
	{PROGRAM_ARGUMENTS}
	<array>
		<string>{program}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
</dict>
</plist>
"#
    )
}

/// The executable a LaunchAgent's plist starts, from the first of its `ProgramArguments`, or
/// `None` when it has none (a binary plist included).
pub fn launch_agent_program(plist: &str) -> Option<String> {
    let after = &plist[plist.find(PROGRAM_ARGUMENTS)? + PROGRAM_ARGUMENTS.len()..];
    let start = after.find("<string>")? + "<string>".len();
    let end = start + after[start..].find("</string>")?;
    Some(xml_unescape(&after[start..end]))
}

/// `text` with the five XML special characters escaped.
pub fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Undoes [`xml_escape`].
pub fn xml_unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// A path for a double-quoted `ExecStart=` word: C escapes for `\` and `"`, and the `%`
/// specifiers and `$` variables systemd would otherwise expand doubled.
fn escape_exec(path: &str) -> String {
    path.replace('\\', r"\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
        .replace('$', "$$")
}

/// The word up to the closing quote of a double-quoted `ExecStart=` value, with `\\` and `\"`
/// unescaped; `None` when the quote is not closed.
fn unquote_exec(quoted: &str) -> Option<String> {
    let mut word = String::new();
    let mut chars = quoted.chars();
    while let Some(char) = chars.next() {
        match char {
            '"' => return Some(word),
            '\\' => word.push(chars.next()?),
            other => word.push(other),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_systemd_unit_file_uses_an_absolute_config_home_only() {
        let home = Path::new("/home/someone");
        let default = home.join(".config/systemd/user/allotd.service");
        assert_eq!(systemd_unit_file(home, None), default);
        assert_eq!(
            systemd_unit_file(home, Some(OsStr::new("relative/config"))),
            default
        );
        let xdg = std::env::temp_dir().join("xdg");
        assert_eq!(
            systemd_unit_file(home, Some(xdg.as_os_str())),
            xdg.join("systemd/user/allotd.service")
        );
    }

    #[test]
    fn test_launch_agent_file_is_named_after_the_label() {
        assert_eq!(
            launch_agent_file(Path::new("/Users/someone")),
            Path::new("/Users/someone/Library/LaunchAgents/tech.mehran.allotd.plist")
        );
    }

    #[test]
    fn test_systemd_unit_quotes_and_reads_back_an_awkward_path() {
        let allotd = r#"/home/a b/100% "$HOME"\x/.allot/bin/allotd"#;
        let unit = systemd_unit(Path::new(allotd));

        assert!(
            unit.contains(r#"ExecStart="/home/a b/100%% \"$$HOME\"\\x/.allot/bin/allotd""#),
            "{unit}"
        );
        assert!(unit.contains("Type=exec\n"));
        assert!(unit.contains("WantedBy=default.target\n"));
        assert!(!unit.contains("Restart="), "allotd is not restarted");
        assert_eq!(systemd_unit_exec(&unit).as_deref(), Some(allotd));
    }

    #[test]
    fn test_systemd_unit_exec_reads_unquoted_and_rejects_missing_or_unclosed() {
        let unquoted = "[Service]\nExecStart=/opt/allotd --flag\n";
        assert_eq!(systemd_unit_exec(unquoted).as_deref(), Some("/opt/allotd"));
        assert_eq!(systemd_unit_exec("[Service]\nType=exec\n"), None);
        assert_eq!(systemd_unit_exec("ExecStart=\"/opt/allotd\n"), None);
    }

    #[test]
    fn test_launch_agent_escapes_and_reads_back_its_program() {
        let allotd = "/Users/A&B <x>/.allot/bin/allotd";
        let plist = launch_agent(Path::new(allotd));

        assert!(
            plist.contains("<string>/Users/A&amp;B &lt;x&gt;/.allot/bin/allotd</string>"),
            "{plist}"
        );
        assert!(plist.contains("<string>tech.mehran.allotd</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(!plist.contains("KeepAlive"), "allotd is not restarted");
        assert_eq!(launch_agent_program(&plist).as_deref(), Some(allotd));
        assert_eq!(launch_agent_program("bplist00"), None);
    }
}
