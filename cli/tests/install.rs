//! Runs the built `allot install` and `allot uninstall` against a temporary `USERPROFILE`, including
//! over an `allot.exe` that is running. A copy of `allot.exe` stands in for the tray app: it exits
//! with a usage error on the tray app's switches, which the install reports as warnings.

// Installing is Windows-only for now.
#![cfg(windows)]

use std::{
    env, fs,
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    process::{self, Child, Command, Output, Stdio},
};

const SKILL: &str = include_str!("../../skill/resource-guard/SKILL.md");
const RULE: &str = include_str!("../../skill/claude-md-rule.md");
const BINARIES: [&str; 3] = ["allot.exe", "allotd.exe", "allot-mcp.exe"];

/// A home directory with a build output to install from and a config that points at a port
/// nothing listens on, so no test reaches a real daemon.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let root = env::temp_dir().join(format!("allot-install-{}-{name}", process::id()));
        let _ = fs::remove_dir_all(&root);
        let app = root.join(".allot");
        fs::create_dir_all(&app).unwrap();
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        fs::write(
            app.join("config.toml"),
            format!("[daemon]\nport = {port}\n"),
        )
        .unwrap();

        // Only allot.exe has to run; the other two are never started by these tests.
        let build = root.join("build");
        fs::create_dir_all(&build).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_allot"), build.join("allot.exe")).unwrap();
        fs::write(build.join("allotd.exe"), "allotd").unwrap();
        fs::write(build.join("allot-mcp.exe"), "allot-mcp").unwrap();
        Self(root)
    }

    fn app(&self) -> PathBuf {
        self.0.join(".allot")
    }

    fn bin(&self) -> PathBuf {
        self.app().join("bin")
    }

    fn build(&self) -> PathBuf {
        self.0.join("build")
    }

    fn skill(&self) -> PathBuf {
        self.0.join(r".claude\skills\resource-guard\SKILL.md")
    }

    fn claude(&self, name: &str) -> PathBuf {
        self.0.join(".claude").join(name)
    }

    fn tray(&self) -> PathBuf {
        self.bin().join("tray")
    }

    /// Adds a tray build to the build output, as a release has.
    fn add_tray_build(&self) {
        let tray = self.build().join("tray");
        fs::create_dir_all(tray.join("en-US")).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_allot"), tray.join("allot-tray.exe")).unwrap();
        fs::write(tray.join("en-US").join("Microsoft.ui.xaml.dll.mui"), "mui").unwrap();
        fs::write(tray.join("allot-tray.pri"), "pri").unwrap();
    }

    /// Runs the built `allot` in this home. `claude` points nowhere, so no test registers the MCP
    /// server with the real Claude Code.
    fn allot(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_allot"))
            .args(args)
            .env("USERPROFILE", &self.0)
            .env("ALLOT_CLAUDE", self.0.join("no-claude.exe"))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// Installs from the build output. The logon task is left out: it is one per user, so a
    /// test would replace the real one.
    fn install(&self) -> String {
        let build = self.build();
        let output = self.allot(&[
            "install",
            "--from",
            build.to_str().unwrap(),
            "--no-logon-task",
        ]);
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    fn legacy(&self) -> PathBuf {
        self.0.join(".resources-helper")
    }

    /// Lays out an install from before the rename, as the v0.2.0 setup left it, in place of
    /// the new folder: its data, binaries, tray app, and Claude Code registrations. `rh-mcp.exe`
    /// is a copy of `allot.exe`, so a test can keep it running.
    fn add_legacy_install(&self) {
        let old = self.legacy();
        let bin = old.join("bin");
        fs::create_dir_all(bin.join("tray")).unwrap();
        fs::create_dir_all(old.join("logs")).unwrap();
        fs::rename(self.app().join("config.toml"), old.join("config.toml")).unwrap();
        fs::remove_dir_all(self.app()).unwrap();
        for (name, contents) in [
            ("token", "old token"),
            ("rh.db", "old db"),
            ("tray.json", "{}"),
            (r"logs\rhd.log", "daemon log"),
            (r"logs\rhd.1.log", "older daemon log"),
            (r"logs\tray.log", "tray log"),
            (r"bin\rh.exe", "rh"),
            (r"bin\rhd.exe", "rhd"),
        ] {
            fs::write(old.join(name), contents).unwrap();
        }
        fs::copy(env!("CARGO_BIN_EXE_allot"), bin.join("rh-mcp.exe")).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_allot"), bin.join(r"tray\rh-tray.exe")).unwrap();

        let claude_json = serde_json::json!({
            "mcpServers": {
                "other": { "command": "other.exe" },
                "resources-helper": { "type": "stdio", "command": bin.join("rh-mcp.exe") },
            },
        });
        fs::write(self.0.join(".claude.json"), claude_json.to_string()).unwrap();
        let claude = self.0.join(".claude");
        fs::create_dir_all(self.skill().parent().unwrap()).unwrap();
        fs::write(self.skill(), "the old skill").unwrap();
        let hook = bin.join("rh.exe").to_string_lossy().replace('\\', "/");
        let entry = |args: [&str; 2]| {
            serde_json::json!({
                "matcher": "Bash|PowerShell",
                "hooks": [{ "type": "command", "command": hook, "args": args }],
            })
        };
        let rtk = serde_json::json!({ "matcher": "Bash", "hooks": [{ "command": "rtk hook" }] });
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [rtk, entry(["hook", "pre-tool"])],
                "PostToolUse": [entry(["hook", "post-tool"])],
                "PostToolUseFailure": [entry(["hook", "post-tool"])],
            },
        });
        fs::write(
            claude.join("settings.json"),
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();
        fs::write(
            claude.join("CLAUDE.md"),
            "# Mine\n\n@resources-helper.md\n\n@RTK.md\n",
        )
        .unwrap();
        fs::write(claude.join("resources-helper.md"), "the old rule").unwrap();
    }

    /// Starts the installed `allot.exe` as a hook that waits for its stdin, so the file stays in
    /// use until [`finish`] closes it.
    fn run_installed_allot(&self) -> Child {
        self.run_held(&self.bin().join("allot.exe"))
    }

    /// Starts `exe`, a copy of `allot.exe`, the same way.
    fn run_held(&self, exe: &Path) -> Child {
        Command::new(exe)
            .args(["hook", "pre-tool"])
            .env("USERPROFILE", &self.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn finish(mut child: Child) {
    drop(child.stdin.take());
    child.wait().unwrap();
}

fn old_copies(bin: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(bin) else {
        return Vec::new();
    };
    entries
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".old-"))
        .collect()
}

#[test]
fn test_install_copies_binaries_and_skill_and_registers_hooks_and_rule() {
    let home = Home::new("fresh");
    let stdout = home.install();

    for name in BINARIES {
        let installed = fs::read(home.bin().join(name)).unwrap();
        assert_eq!(
            installed,
            fs::read(home.build().join(name)).unwrap(),
            "{name}"
        );
    }
    assert_eq!(fs::read_to_string(home.skill()).unwrap(), SKILL);
    assert!(!home.app().join("claude-md-rule.md").exists());
    assert!(stdout.contains("allotd is not running"), "{stdout}");
    assert!(stdout.contains("no tray app in"), "{stdout}");
    assert!(!home.tray().exists());

    // claude is not found, so the entry is written to ~/.claude.json directly.
    assert!(
        stdout.contains("1. [done] MCP server, user scope (claude was not found, so it was"),
        "{stdout}"
    );
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.0.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(
        config["mcpServers"]["allot"]["command"],
        home.bin().join("allot-mcp.exe").to_str().unwrap()
    );

    assert!(
        stdout.contains("2. [done] Hooks in")
            && stdout.contains("(added PreToolUse, PostToolUse, PostToolUseFailure)"),
        "{stdout}"
    );
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.claude("settings.json")).unwrap()).unwrap();
    let hook = home
        .bin()
        .join("allot.exe")
        .to_string_lossy()
        .replace('\\', "/");
    for (event, arg) in [
        ("PreToolUse", "pre-tool"),
        ("PostToolUse", "post-tool"),
        ("PostToolUseFailure", "post-tool"),
    ] {
        let entry = &settings["hooks"][event][0]["hooks"][0];
        assert_eq!(entry["command"], hook.as_str(), "{event}");
        assert_eq!(entry["args"], serde_json::json!(["hook", arg]), "{event}");
    }

    assert!(
        stdout.contains("3. [done] CLAUDE.md rule") && stdout.contains("added the import"),
        "{stdout}"
    );
    assert_eq!(fs::read_to_string(home.claude("allot.md")).unwrap(), RULE);
    assert_eq!(
        fs::read_to_string(home.claude("CLAUDE.md")).unwrap(),
        "@allot.md\n"
    );
    assert!(!stdout.contains("scheduled task"), "{stdout}");
    assert!(stdout.contains("restart them"), "{stdout}");

    let stdout = home.install();
    assert!(stdout.contains("2. [done] Hooks in"), "{stdout}");
    assert!(!stdout.contains("(added"), "{stdout}");
    assert!(!stdout.contains("restart them"), "{stdout}");
}

#[test]
fn test_install_marks_registered_steps_done() {
    let home = Home::new("registered");
    let bin = home.bin();
    let mcp = bin.join("allot-mcp.exe");
    let claude_json = serde_json::json!({
        "mcpServers": { "allot": { "type": "stdio", "command": mcp, "args": [] } },
    });
    fs::write(home.0.join(".claude.json"), claude_json.to_string()).unwrap();
    let claude = home.0.join(".claude");
    fs::create_dir_all(&claude).unwrap();
    let hook = bin.join("allot.exe").to_string_lossy().replace('\\', "/");
    let entry = |args: [&str; 2]| {
        serde_json::json!([{
            "matcher": "Bash|PowerShell",
            "hooks": [{ "type": "command", "command": hook, "args": args }],
        }])
    };
    let settings = serde_json::json!({
        "hooks": {
            "PreToolUse": entry(["hook", "pre-tool"]),
            "PostToolUse": entry(["hook", "post-tool"]),
            "PostToolUseFailure": entry(["hook", "post-tool"]),
        },
    });
    let settings = serde_json::to_string_pretty(&settings).unwrap();
    fs::write(claude.join("settings.json"), &settings).unwrap();
    // The rule pasted by hand under a numbered heading, as an older install asked.
    let rule = RULE.replace(
        "## Shared Machine Resources",
        "## 11 — Shared Machine Resources",
    );
    fs::write(claude.join("CLAUDE.md"), format!("# Rules\n\n{rule}")).unwrap();

    let stdout = home.install();

    assert!(
        stdout.contains("1. [done] MCP server, user scope\n"),
        "{stdout}"
    );
    assert!(!stdout.contains("claude mcp add"), "{stdout}");
    assert!(stdout.contains("2. [done] Hooks in"), "{stdout}");
    assert_eq!(
        fs::read_to_string(claude.join("settings.json")).unwrap(),
        settings
    );
    assert!(
        stdout.contains("3. [done] CLAUDE.md rule")
            && stdout.contains("replaced the pasted rule in CLAUDE.md with an import"),
        "{stdout}"
    );
    assert_eq!(
        fs::read_to_string(claude.join("CLAUDE.md")).unwrap(),
        "# Rules\n\n@allot.md\n"
    );
    assert_eq!(
        fs::read_to_string(claude.join("CLAUDE.md.allot-backup")).unwrap(),
        format!("# Rules\n\n{rule}")
    );
}

#[test]
fn test_install_mirrors_the_tray_build_and_reports_the_stand_in_exe() {
    let home = Home::new("tray");
    home.add_tray_build();
    let stray = home.tray().join("fr-FR").join("old.mui");
    fs::create_dir_all(stray.parent().unwrap()).unwrap();
    fs::write(&stray, "old").unwrap();
    fs::write(home.tray().join("allot-tray.exe"), "an older build").unwrap();

    let stdout = home.install();

    assert!(stdout.contains("installed the tray app into"), "{stdout}");
    assert_eq!(
        fs::read(home.tray().join("allot-tray.exe")).unwrap(),
        fs::read(env!("CARGO_BIN_EXE_allot")).unwrap()
    );
    assert!(
        home.tray()
            .join(r"en-US\Microsoft.ui.xaml.dll.mui")
            .is_file()
    );
    assert!(home.tray().join("allot-tray.pri").is_file());
    assert!(!home.tray().join("fr-FR").exists());
    // A tray app was installed before, so Launch at logon is left as the user set it.
    assert!(!stdout.contains("Launch at logon"), "{stdout}");
    // The stand-in exits at once, as a tray app does when another copy runs.
    assert!(stdout.contains("exited at once"), "{stdout}");

    let output = home.allot(&[
        "install",
        "--from",
        home.build().to_str().unwrap(),
        "--tray",
        home.build().to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("has no allot-tray.exe"));
}

#[test]
fn test_first_tray_install_asks_the_tray_app_to_turn_on_launch_at_logon() {
    let home = Home::new("tray-first");
    home.add_tray_build();

    let stdout = home.install();

    // The stand-in does not know the switch, which shows the install asked for it.
    assert!(
        stdout.contains("warning: Launch at logon was not turned on")
            && stdout.contains("--launch-at-logon on exited with"),
        "{stdout}"
    );
}

#[test]
fn test_reinstall_over_a_tray_app_that_does_not_quit_renames_its_files_aside() {
    let home = Home::new("tray-running");
    home.add_tray_build();
    home.install();
    let running = home.run_held(&home.tray().join("allot-tray.exe"));

    let stdout = home.install();

    assert!(stdout.contains("the tray app did not quit"), "{stdout}");
    assert!(stdout.contains("still in use"), "{stdout}");
    let old = old_copies(&home.tray());
    assert_eq!(old.len(), 1, "{old:?}");
    assert!(old[0].starts_with("allot-tray.exe.old-"), "{old:?}");

    finish(running);
    home.install();
    assert!(old_copies(&home.tray()).is_empty());
}

#[test]
fn test_reinstall_over_a_running_allot_sets_the_old_copy_aside_until_it_exits() {
    let home = Home::new("running");
    home.install();
    let running = home.run_installed_allot();

    let stdout = home.install();

    assert!(stdout.contains("still in use"), "{stdout}");
    let old = old_copies(&home.bin());
    assert_eq!(old.len(), 1, "{old:?}");
    assert!(old[0].starts_with("allot.old-"), "{old:?}");
    assert!(home.bin().join("allot.exe").is_file());

    finish(running);
    let stdout = home.install();
    assert!(!stdout.contains("still in use"), "{stdout}");
    assert!(old_copies(&home.bin()).is_empty());
}

#[test]
fn test_install_takes_over_a_resources_helper_install() {
    let home = Home::new("legacy");
    home.add_legacy_install();
    let config = fs::read_to_string(home.legacy().join("config.toml")).unwrap();

    let stdout = home.install();

    assert!(
        stdout.contains("taking over the Resources Helper install"),
        "{stdout}"
    );
    for (name, contents) in [
        ("config.toml", config.as_str()),
        ("token", "old token"),
        ("allot.db", "old db"),
        ("tray.json", "{}"),
        (r"logs\allotd.log", "daemon log"),
        (r"logs\allotd.1.log", "older daemon log"),
        (r"logs\tray.log", "tray log"),
    ] {
        assert_eq!(
            fs::read_to_string(home.app().join(name)).unwrap(),
            contents,
            "{name}"
        );
    }
    assert!(!home.legacy().exists(), "{stdout}");
    assert_eq!(fs::read_to_string(home.skill()).unwrap(), SKILL);

    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.0.join(".claude.json")).unwrap()).unwrap();
    let mut servers: Vec<_> = config["mcpServers"].as_object().unwrap().keys().collect();
    servers.sort();
    assert_eq!(servers, ["allot", "other"], "{stdout}");
    assert!(
        stdout.contains("removed the MCP server \"resources-helper\""),
        "{stdout}"
    );

    let settings = fs::read_to_string(home.claude("settings.json")).unwrap();
    assert!(!settings.contains(".resources-helper"), "{settings}");
    let settings: serde_json::Value = serde_json::from_str(&settings).unwrap();
    let pre_tool = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre_tool.len(), 2, "{settings}");
    assert_eq!(pre_tool[0]["hooks"][0]["command"], "rtk hook");
    for event in ["PostToolUse", "PostToolUseFailure"] {
        assert_eq!(
            settings["hooks"][event].as_array().unwrap().len(),
            1,
            "{event}"
        );
    }

    assert!(
        stdout.contains("replaced the Resources Helper import in CLAUDE.md"),
        "{stdout}"
    );
    assert_eq!(
        fs::read_to_string(home.claude("CLAUDE.md")).unwrap(),
        "# Mine\n\n@allot.md\n\n@RTK.md\n"
    );
    assert_eq!(fs::read_to_string(home.claude("allot.md")).unwrap(), RULE);
    assert!(!home.claude("resources-helper.md").exists());

    let stdout = home.install();
    assert!(!stdout.contains("Resources Helper"), "{stdout}");
}

#[test]
fn test_take_over_keeps_what_open_sessions_read_until_they_exit() {
    let home = Home::new("legacy-running");
    home.add_legacy_install();
    let running = home.run_held(&home.legacy().join(r"bin\rh-mcp.exe"));

    let stdout = home.install();

    assert!(stdout.contains("still in use"), "{stdout}");
    let mut left: Vec<_> = fs::read_dir(home.legacy())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["bin", "config.toml", "token"]);
    assert_eq!(
        fs::read_dir(home.legacy().join("bin")).unwrap().count(),
        1,
        "only rh-mcp.exe"
    );

    finish(running);
    let stdout = home.install();
    assert!(!home.legacy().exists(), "{stdout}");
}

#[test]
fn test_install_from_the_bin_dir_or_without_a_binary_is_refused() {
    let home = Home::new("refused");
    home.install();

    let bin = home.bin();
    let output = home.allot(&["install", "--from", bin.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("is the install itself"));

    fs::remove_file(home.build().join("allot-mcp.exe")).unwrap();
    let build = home.build();
    let output = home.allot(&["install", "--from", build.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("has no allot-mcp.exe"));
}

#[test]
fn test_uninstall_keeps_the_data_and_purge_removes_it() {
    let home = Home::new("uninstall");
    home.add_tray_build();
    fs::create_dir_all(home.claude("")).unwrap();
    fs::write(home.claude("CLAUDE.md"), "# Mine\n").unwrap();
    home.install();
    fs::write(home.app().join("allot.db"), "db").unwrap();
    fs::write(home.app().join("token"), "token").unwrap();

    let output = home.allot(&["uninstall"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("removed the tray app"), "{stdout}");
    assert!(!home.bin().exists());
    assert!(!home.skill().exists());
    assert!(!home.skill().parent().unwrap().exists());
    assert_eq!(
        fs::read_to_string(home.claude("settings.json")).unwrap(),
        "{\n  \"hooks\": {}\n}\n"
    );
    assert_eq!(
        fs::read_to_string(home.claude("CLAUDE.md")).unwrap(),
        "# Mine\n"
    );
    assert!(!home.claude("allot.md").exists());
    // claude is not found, so the entry is removed from ~/.claude.json directly.
    assert!(stdout.contains("removed the MCP server from"), "{stdout}");
    assert_eq!(
        fs::read_to_string(home.0.join(".claude.json")).unwrap(),
        "{\n  \"mcpServers\": {}\n}\n"
    );
    for kept in ["allot.db", "token", "config.toml"] {
        assert!(home.app().join(kept).is_file(), "{kept}");
    }

    let output = home.allot(&["uninstall", "--purge"]);
    assert!(output.status.success(), "{output:?}");
    assert!(!home.app().exists());
}

#[test]
fn test_uninstall_sets_a_running_allot_aside() {
    let home = Home::new("uninstall-running");
    home.install();
    let running = home.run_installed_allot();

    let output = home.allot(&["uninstall"]);

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("still in use"), "{stdout}");
    assert!(!home.bin().join("allot.exe").exists());
    assert!(!home.bin().join("allotd.exe").exists());
    assert_eq!(old_copies(&home.bin()).len(), 1);
    finish(running);
}
