//! Runs the built `allot` against a home whose allotd is not running: `allot daemon stop`
//! records the stop, and the commands that would start allotd leave it stopped and say why.

use std::{
    env, fs,
    io::Write,
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::PathBuf,
    process::{self, Command, Output},
};

use allot_shared::{
    paths::Paths,
    platform::HOME_VAR,
    stop::{StopRecord, StoppedBy},
};
use serde_json::Value;

/// A home whose config points at a free port, where nothing runs.
struct Home {
    dir: PathBuf,
    port: u16,
}

impl Home {
    fn new(name: &str) -> Self {
        let dir = env::temp_dir().join(format!("allot-stop-{}-{name}", process::id()));
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let paths = Paths::under(&dir);
        fs::create_dir_all(paths.root()).unwrap();
        fs::write(paths.config_file(), format!("[daemon]\nport = {port}\n")).unwrap();
        Self { dir, port }
    }

    fn paths(&self) -> Paths {
        Paths::under(&self.dir)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_allot"))
            .args(args)
            .env(HOME_VAR, &self.dir)
            .env("NO_COLOR", "1")
            .output()
            .unwrap()
    }
}

impl Drop for Home {
    /// Should a command have started allotd after all, it runs under this home on this port:
    /// shut that one down, with this home's token, so no test leaves it behind.
    fn drop(&mut self) {
        if let Ok(token) = fs::read_to_string(self.paths().token_file())
            && let Ok(mut stream) = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port))
        {
            let _ = write!(
                stream,
                "POST /shutdown HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\n\
                 Content-Length: 0\r\nConnection: close\r\n\r\n",
                token.trim()
            );
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn test_daemon_stop_keeps_allotd_stopped_for_later_commands() {
    let home = Home::new("cli");

    let stop = home.run(&["daemon", "stop"]);
    assert!(stop.status.success(), "{stop:?}");
    assert!(
        stdout(&stop).starts_with("allotd is not running; it stays stopped until"),
        "{stop:?}"
    );
    let record = StopRecord::read(&home.paths().stop_file()).unwrap();
    assert_eq!(record.by, StoppedBy::Cli);

    let status = home.run(&["status"]);
    assert!(status.status.success(), "{status:?}");
    assert!(
        stdout(&status).starts_with("allotd  STOPPED: you stopped it with `allot daemon stop`"),
        "{status:?}"
    );

    let leases = home.run(&["leases"]);
    assert!(!leases.status.success(), "{leases:?}");
    assert!(
        stderr(&leases).contains("allotd is stopped: the user stopped it"),
        "{leases:?}"
    );

    let daemon_status = home.run(&["daemon", "status"]);
    assert!(
        stdout(&daemon_status).starts_with("allotd is stopped: you stopped it"),
        "{daemon_status:?}"
    );
    let json: Value =
        serde_json::from_slice(&home.run(&["daemon", "status", "--json"]).stdout).unwrap();
    assert_eq!(json["running"], false);
    assert_eq!(json["stopped"]["by"], "cli");

    let doctor: Value = serde_json::from_slice(&home.run(&["doctor", "--json"]).stdout).unwrap();
    let check = doctor
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "allotd")
        .unwrap();
    assert_eq!(check["outcome"], "warn");
    assert!(
        check["detail"]
            .as_str()
            .unwrap()
            .starts_with("stopped: you stopped it"),
        "{check}"
    );

    assert!(
        TcpStream::connect((Ipv4Addr::LOCALHOST, home.port)).is_err(),
        "a command started allotd"
    );
    assert!(home.paths().stop_file().exists());
}
