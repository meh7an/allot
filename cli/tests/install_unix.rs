//! Runs the built `allot install` and `allot uninstall` against a temporary `HOME` on Linux and
//! macOS. Stand-ins for `systemctl` and `launchctl` come first on `PATH`: they log their
//! arguments and keep a marker file for the enabled or loaded state, so no test touches the
//! runner's own user manager or launchd.

#![cfg(unix)]

use std::{
    env, fs,
    net::{Ipv4Addr, TcpListener},
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{self, Command, Output, Stdio},
};

const BINARIES: [&str; 3] = ["allot", "allotd", "allot-mcp"];

/// A home directory with a build output to install from, a config that points at a port nothing
/// listens on, a `~/.local/bin`, and the stand-ins for the service managers.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        Self::with_managers(name, true)
    }

    /// `working` false: every `systemctl` and `launchctl` call fails, as without a user manager
    /// or a GUI login.
    fn with_managers(name: &str, working: bool) -> Self {
        let root = env::temp_dir().join(format!("allot-install-unix-{}-{name}", process::id()));
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
        fs::create_dir_all(root.join(".local").join("bin")).unwrap();

        // Only allot has to run; the other two are never started by these tests.
        let build = root.join("build");
        fs::create_dir_all(&build).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_allot"), build.join("allot")).unwrap();
        fs::write(build.join("allotd"), "allotd").unwrap();
        fs::write(build.join("allot-mcp"), "allot-mcp").unwrap();

        let home = Self(root);
        home.write_managers(working);
        home
    }

    fn write_managers(&self, working: bool) {
        let fakes = self.fakes();
        let state = self.0.join("state");
        fs::create_dir_all(&fakes).unwrap();
        fs::create_dir_all(&state).unwrap();
        let log = self.log();
        let (log, state) = (log.display(), state.display());
        let fail = if working {
            ""
        } else {
            "echo 'not available here' >&2; exit 1\n"
        };
        let systemctl = format!(
            "#!/bin/sh\necho \"systemctl $*\" >> '{log}'\n{fail}\
             case \"$2\" in\n\
             is-enabled) if [ -e '{state}/enabled' ]; then echo enabled; exit 0; fi; \
             echo disabled; exit 1;;\n\
             enable) touch '{state}/enabled';;\n\
             disable) rm -f '{state}/enabled';;\n\
             esac\nexit 0\n"
        );
        let launchctl = format!(
            "#!/bin/sh\necho \"launchctl $*\" >> '{log}'\n{fail}\
             case \"$1\" in\n\
             print) if [ -e '{state}/loaded' ]; then exit 0; fi; exit 113;;\n\
             bootstrap) touch '{state}/loaded';;\n\
             bootout) rm -f '{state}/loaded';;\n\
             esac\nexit 0\n"
        );
        for (name, script) in [("systemctl", systemctl), ("launchctl", launchctl)] {
            let path = fakes.join(name);
            fs::write(&path, script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn fakes(&self) -> PathBuf {
        self.0.join("fakes")
    }

    fn log(&self) -> PathBuf {
        self.0.join("calls.log")
    }

    /// The service manager calls so far, one per line.
    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.log())
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
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

    fn link(&self) -> PathBuf {
        self.0.join(".local").join("bin").join("allot")
    }

    fn claude(&self, name: &str) -> PathBuf {
        self.0.join(".claude").join(name)
    }

    /// The autostart file this OS uses.
    fn autostart(&self) -> PathBuf {
        if cfg!(target_os = "macos") {
            self.0.join("Library/LaunchAgents/tech.mehran.allotd.plist")
        } else {
            self.0.join(".config/systemd/user/allotd.service")
        }
    }

    /// Runs the built `allot` in this home. `claude` points nowhere, so no test registers the
    /// MCP server with the real Claude Code.
    fn allot(&self, args: &[&str]) -> Output {
        let path = format!(
            "{}:{}",
            self.fakes().display(),
            env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_allot"))
            .args(args)
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join(".config"))
            .env("ALLOT_CLAUDE", self.0.join("no-claude"))
            .env("PATH", path)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn install_with(&self, extra: &[&str]) -> String {
        let build = self.build();
        let mut args = vec!["install", "--from", build.to_str().unwrap()];
        args.extend(extra);
        let output = self.allot(&args);
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    fn install(&self) -> String {
        self.install_with(&[])
    }

    fn uninstall(&self) -> String {
        let output = self.allot(&["uninstall"]);
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

fn bin_names(bin: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(bin)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn test_install_copies_binaries_links_allot_and_registers_hooks() {
    let home = Home::new("fresh");
    let stdout = home.install();

    for name in BINARIES {
        let installed = home.bin().join(name);
        assert_eq!(
            fs::read(&installed).unwrap(),
            fs::read(home.build().join(name)).unwrap(),
            "{name}"
        );
        let mode = fs::metadata(&installed).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755, "{name}");
    }
    assert_eq!(bin_names(&home.bin()), ["allot", "allot-mcp", "allotd"]);
    assert_eq!(
        fs::read_link(home.link()).unwrap(),
        home.bin().join("allot")
    );
    assert!(stdout.contains("linked "), "{stdout}");

    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.0.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(
        config["mcpServers"]["allot"]["command"],
        home.bin().join("allot-mcp").to_str().unwrap()
    );
    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.claude("settings.json")).unwrap()).unwrap();
    assert_eq!(
        settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        home.bin().join("allot").to_str().unwrap()
    );
    assert_eq!(
        fs::read_to_string(home.claude("CLAUDE.md")).unwrap(),
        "@allot.md\n"
    );
    assert!(
        home.claude("skills/resource-guard/SKILL.md").is_file(),
        "{stdout}"
    );
    // The autostart started allotd, so nothing says it is not running.
    assert!(!stdout.contains("allotd is not running"), "{stdout}");
}

#[test]
fn test_install_sets_up_autostart_once() {
    let home = Home::new("autostart");
    let stdout = home.install();
    let allotd = home.bin().join("allotd");
    let file = home.autostart();
    let text = fs::read_to_string(&file).unwrap();
    let calls = home.calls();
    assert!(
        stdout.contains(&format!(
            "4. [done] allotd at login, {} {} (registered and started)",
            if cfg!(target_os = "macos") {
                "LaunchAgent"
            } else {
                "systemd user unit"
            },
            file.display()
        )),
        "{stdout}"
    );
    if cfg!(target_os = "macos") {
        assert!(
            text.contains(&format!("<string>{}</string>", allotd.display())),
            "{text}"
        );
        assert!(
            calls.contains(&format!(
                "launchctl bootstrap gui/{} {}",
                uid(),
                file.display()
            )),
            "{calls:?}"
        );
    } else {
        assert!(
            text.contains(&format!("ExecStart=\"{}\"", allotd.display())),
            "{text}"
        );
        assert!(text.contains("WantedBy=default.target"), "{text}");
        assert!(
            calls.contains(&"systemctl --user daemon-reload".to_owned()),
            "{calls:?}"
        );
        assert!(
            calls.contains(&"systemctl --user enable --now allotd.service".to_owned()),
            "{calls:?}"
        );
    }

    let before = calls.len();
    let stdout = home.install();
    assert!(
        stdout.contains("4. [done] allotd at login") && !stdout.contains("started)"),
        "{stdout}"
    );
    let again = &home.calls()[before..];
    assert!(
        !again
            .iter()
            .any(|call| call.contains("daemon-reload") || call.contains("bootstrap")),
        "{again:?}"
    );
}

#[test]
fn test_install_replaces_a_unit_or_agent_for_another_allotd() {
    let home = Home::new("other-allotd");
    let file = home.autostart();
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let old = if cfg!(target_os = "macos") {
        allot_shared::autostart::launch_agent(Path::new("/dev/allotd"))
    } else {
        allot_shared::autostart::systemd_unit(Path::new("/dev/allotd"))
    };
    fs::write(&file, old).unwrap();

    let stdout = home.install();

    assert!(
        stdout.contains("(updated and started; it started /dev/allotd before)"),
        "{stdout}"
    );
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains(home.bin().join("allotd").to_str().unwrap())
    );
}

#[test]
fn test_install_without_a_service_manager_prints_the_steps() {
    let home = Home::with_managers("no-manager", false);
    let stdout = home.install();

    assert!(stdout.contains("4. [todo] allotd at login"), "{stdout}");
    if cfg!(target_os = "macos") {
        assert!(
            stdout.contains(&format!("launchctl bootstrap gui/{} \"", uid())),
            "{stdout}"
        );
    } else {
        assert!(
            stdout.contains("systemctl --user enable --now allotd.service"),
            "{stdout}"
        );
    }
    // Nothing set up allotd's start, so the first session starts it.
    assert!(
        stdout.contains("allotd is not running; the first session starts it"),
        "{stdout}"
    );
}

#[test]
fn test_install_no_autostart_touches_no_service_manager() {
    let home = Home::new("no-autostart");
    let stdout = home.install_with(&["--no-autostart"]);

    assert!(!home.autostart().exists());
    assert!(home.calls().is_empty(), "{:?}", home.calls());
    assert!(!stdout.contains("allotd at login"), "{stdout}");
    assert!(
        stdout.contains("allotd is not running; the first session starts it"),
        "{stdout}"
    );
}

#[test]
fn test_install_leaves_another_allot_on_path_alone() {
    let home = Home::new("foreign-link");
    fs::write(home.link(), "someone else's").unwrap();

    let stdout = home.install();

    assert!(
        stdout.contains("left ") && stdout.contains(" alone"),
        "{stdout}"
    );
    assert_eq!(fs::read_to_string(home.link()).unwrap(), "someone else's");
    home.uninstall();
    assert!(home.link().is_file(), "uninstall keeps what is not ours");
}

#[test]
fn test_install_without_local_bin_says_how_to_add_path() {
    let home = Home::new("no-local-bin");
    fs::remove_dir_all(home.0.join(".local")).unwrap();

    let stdout = home.install();

    assert!(
        stdout.contains(&format!(
            "add {} to PATH to run allot from anywhere",
            home.bin().display()
        )),
        "{stdout}"
    );
}

#[test]
fn test_install_replaces_a_running_allot() {
    let home = Home::new("running");
    home.install();
    let mut running = Command::new(home.bin().join("allot"))
        .args(["hook", "pre-tool"])
        .env("HOME", &home.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    fs::write(home.build().join("allot-mcp"), "a newer allot-mcp").unwrap();

    home.install();

    assert_eq!(
        fs::read_to_string(home.bin().join("allot-mcp")).unwrap(),
        "a newer allot-mcp"
    );
    assert_eq!(bin_names(&home.bin()), ["allot", "allot-mcp", "allotd"]);
    drop(running.stdin.take());
    running.wait().unwrap();
}

#[test]
fn test_uninstall_removes_autostart_binaries_link_and_registrations() {
    let home = Home::new("uninstall");
    home.install();
    let before = home.calls().len();

    let stdout = home.uninstall();

    assert!(!home.autostart().exists(), "{stdout}");
    let calls = &home.calls()[before..];
    if cfg!(target_os = "macos") {
        assert!(
            calls.contains(&format!(
                "launchctl bootout gui/{}/tech.mehran.allotd",
                uid()
            )),
            "{calls:?}"
        );
        assert!(stdout.contains("removed the LaunchAgent"), "{stdout}");
    } else {
        assert!(
            calls.contains(&"systemctl --user disable --now allotd.service".to_owned()),
            "{calls:?}"
        );
        assert!(stdout.contains("removed the systemd user unit"), "{stdout}");
    }
    assert!(!home.bin().exists());
    assert!(fs::symlink_metadata(home.link()).is_err(), "{stdout}");
    assert!(!home.claude("skills/resource-guard/SKILL.md").exists());
    let settings = fs::read_to_string(home.claude("settings.json")).unwrap();
    assert!(!settings.contains("allot"), "{settings}");
    assert!(home.app().join("config.toml").is_file(), "data is kept");
}

#[test]
fn test_uninstall_purge_removes_the_data_dir_with_its_logs() {
    let home = Home::new("purge");
    home.install();
    let logs = home.app().join("logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(logs.join("allotd.log"), "a log").unwrap();
    fs::write(home.app().join("token"), "secret").unwrap();

    let output = home.allot(&["uninstall", "--purge"]);

    assert!(output.status.success(), "{output:?}");
    assert!(!home.app().exists());
}

#[test]
fn test_uninstall_keeps_a_unit_or_agent_for_another_allotd() {
    let home = Home::new("keep-other");
    let file = home.autostart();
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let other = if cfg!(target_os = "macos") {
        allot_shared::autostart::launch_agent(Path::new("/dev/allotd"))
    } else {
        allot_shared::autostart::systemd_unit(Path::new("/dev/allotd"))
    };
    fs::write(&file, &other).unwrap();
    // A link of the same name to another build is not ours either.
    symlink("/dev/allot", home.link()).unwrap();

    home.uninstall();

    assert_eq!(fs::read_to_string(&file).unwrap(), other);
    assert!(home.calls().is_empty(), "{:?}", home.calls());
    assert_eq!(fs::read_link(home.link()).unwrap(), Path::new("/dev/allot"));
}

#[test]
fn test_install_help_names_unix_paths() {
    let home = Home::new("help");
    let output = home.allot(&["install", "--help"]);
    let help = String::from_utf8(output.stdout).unwrap();

    for expected in [
        "~/.allot/bin",
        "target/release",
        "--no-autostart",
        "LaunchAgent",
    ] {
        assert!(help.contains(expected), "{expected}: {help}");
    }
    for unexpected in [
        "%USERPROFILE%",
        ".exe",
        r"target\release",
        "--tray",
        "--no-logon-task",
    ] {
        assert!(!help.contains(unexpected), "{unexpected}: {help}");
    }
}

#[test]
fn test_install_from_the_bin_dir_is_refused_with_a_unix_example() {
    let home = Home::new("from-bin");
    home.install();
    let bin = home.bin();

    let output = home.allot(&["install", "--from", bin.to_str().unwrap()]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("such as target/release/allot install"),
        "{stderr}"
    );
}
