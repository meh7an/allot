//! Runs the built `allot update` against a temporary home, with the built `allot` installed as
//! its `~/.allot/bin/allot`, and a release server on a loopback port through `ALLOT_UPDATE_BASE`.
//! Nothing here reaches GitHub or the real `~/.allot`, and every run ends before an install
//! step: what is installed per OS is covered by the unit tests and the end-to-end CI job.

use std::{
    collections::HashMap,
    env, fs,
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::PathBuf,
    process::{self, Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use allot_shared::{
    paths::Paths,
    platform::{CLI_EXE, HOME_VAR},
    update::{OutcomeState, UpdateOutcome},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A home directory with the built allot installed in it.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let root = env::temp_dir().join(format!("allot-update-{}-{name}", process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = Self(root);
        fs::create_dir_all(home.paths().bin_dir()).unwrap();
        fs::copy(
            env!("CARGO_BIN_EXE_allot"),
            home.paths().bin_dir().join(CLI_EXE),
        )
        .unwrap();
        home
    }

    /// A home with no install at all.
    fn empty(name: &str) -> Self {
        let home = Self::new(name);
        fs::remove_file(home.paths().bin_dir().join(CLI_EXE)).unwrap();
        home
    }

    fn paths(&self) -> Paths {
        Paths::under(&self.0)
    }

    /// Runs the built `allot update` in this home, reading releases from `base`.
    fn update(&self, args: &[&str], base: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_allot"))
            .arg("update")
            .args(args)
            .env(HOME_VAR, &self.0)
            .env("ALLOT_UPDATE_BASE", base)
            .env_remove("ALLOT_NPM_LAUNCHER")
            .env_remove("ALLOT_ALLOW_DOWNGRADE")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A release server that redirects `releases/latest` to a tag and answers 404 to the rest.
struct Server {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    fn latest(tag: &str) -> Self {
        Self::start(Some(HashMap::from([(
            "/releases/latest".to_owned(),
            format!("/releases/tag/{tag}"),
        )])))
    }

    /// Accepts connections and hangs up without an answer. A closed port would do, but a port
    /// someone just released can be taken by another test before this one asks.
    fn hanging_up() -> Self {
        Self::start(None)
    }

    fn start(routes: Option<HashMap<String, String>>) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = Arc::clone(&stop);
            thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let (Ok(stream), Some(routes)) = (stream, &routes) {
                        serve(stream, routes);
                    }
                }
            })
        };
        Self {
            port,
            stop,
            thread: Some(thread),
        }
    }

    fn base(&self) -> String {
        format!("http://127.0.0.1:{}/releases", self.port)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accept loop so it sees the flag.
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(mut stream: TcpStream, routes: &HashMap<String, String>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut request = [0_u8; 2048];
    let Ok(read) = stream.read(&mut request) else {
        return;
    };
    let text = String::from_utf8_lossy(&request[..read]);
    let path = text.split_whitespace().nth(1).unwrap_or_default();
    let reply = match routes.get(path) {
        Some(to) => format!(
            "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
        None => {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
        }
    };
    let _ = stream.write_all(reply.as_bytes());
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn test_check_json_reports_a_newer_release_as_an_update_status() {
    let home = Home::new("check-newer");
    let server = Server::latest("v99.0.0");

    let output = home.update(&["--check", "--json"], &server.base());

    assert!(output.status.success(), "{output:?}");
    let status: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(status["state"], "available");
    assert_eq!(status["installed"], VERSION);
    assert_eq!(status["latest"], "99.0.0");
    assert_eq!(
        status["release_url"],
        format!("{}/tag/v99.0.0", server.base())
    );
    assert!(status["checked_at_ms"].as_i64().unwrap() > 0);
}

#[test]
fn test_check_says_how_to_install_a_newer_release() {
    let home = Home::new("check-text");
    let server = Server::latest("v99.0.0");

    let output = home.update(&["--check"], &server.base());

    assert!(output.status.success(), "{output:?}");
    let text = stdout(&output);
    assert!(text.contains("99.0.0 is available"), "{text}");
    assert!(text.contains("run `allot update`"), "{text}");
}

#[test]
fn test_check_reports_the_installed_release_as_up_to_date() {
    let home = Home::new("check-current");
    let server = Server::latest(&format!("v{VERSION}"));

    let json = home.update(&["--check", "--json"], &server.base());
    let status: serde_json::Value = serde_json::from_str(&stdout(&json)).unwrap();
    assert_eq!(status["state"], "up_to_date");
    assert_eq!(status["latest"], VERSION);

    let text = home.update(&["--check"], &server.base());
    assert!(text.status.success(), "{text:?}");
    assert!(stdout(&text).contains("is up to date"), "{}", stdout(&text));
}

#[test]
fn test_update_with_nothing_newer_installs_nothing_and_writes_no_record() {
    let home = Home::new("nothing-newer");
    let server = Server::latest(&format!("v{VERSION}"));

    let output = home.update(&[], &server.base());

    assert!(output.status.success(), "{output:?}");
    assert!(
        stdout(&output).contains("is up to date"),
        "{}",
        stdout(&output)
    );
    assert!(!home.paths().update_outcome_file().exists());
    assert!(!home.paths().updates_dir().exists());
}

#[test]
fn test_check_fails_when_the_release_server_hangs_up() {
    let home = Home::new("check-closed");
    let server = Server::hanging_up();

    let output = home.update(&["--check"], &server.base());

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("could not reach"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn test_update_without_an_install_says_there_is_nothing_to_update() {
    let home = Home::empty("no-install");

    let output = home.update(&["--check"], "http://127.0.0.1:1/releases");

    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(message.contains("nothing to update"), "{message}");
    assert!(message.contains("allot install"), "{message}");
}

#[test]
fn test_update_to_the_installed_version_by_pin_changes_nothing() {
    let home = Home::new("pin-same");

    let output = home.update(&["--version", VERSION], "http://127.0.0.1:1/releases");

    assert!(output.status.success(), "{output:?}");
    assert!(
        stdout(&output).contains("already installed"),
        "{}",
        stdout(&output)
    );
    assert!(!home.paths().update_outcome_file().exists());
}

#[test]
fn test_update_to_an_older_pin_is_refused_unless_the_downgrade_is_allowed() {
    let home = Home::new("pin-older");
    let server = Server::latest("v99.0.0");

    let refused = home.update(&["--version", "0.1.0"], &server.base());

    assert!(!refused.status.success());
    let message = stderr(&refused);
    assert!(message.contains("--allow-downgrade"), "{message}");
    assert!(!home.paths().update_outcome_file().exists());

    // Allowed, it goes on to fetch a release the server does not have.
    let allowed = home.update(&["--version", "0.1.0", "--allow-downgrade"], &server.base());
    assert!(!allowed.status.success());
    let message = stderr(&allowed);
    assert!(!message.contains("--allow-downgrade"), "{message}");
    assert!(message.contains("allot-0.1.0"), "{message}");
}

#[test]
fn test_a_failed_download_is_recorded_for_allotd_and_fails_the_command() {
    let home = Home::new("download-fails");
    let server = Server::latest("v99.0.0");

    let output = home.update(&[], &server.base());

    assert!(!output.status.success(), "{output:?}");
    let message = stderr(&output);
    assert!(message.contains("allot-99.0.0"), "{message}");
    let outcome = UpdateOutcome::read(&home.paths().update_outcome_file()).unwrap();
    assert_eq!(outcome.state, OutcomeState::Failed);
    assert_eq!(outcome.version, "99.0.0");
    assert!(outcome.reason.unwrap().contains("allot-99.0.0"));
    assert!(outcome.finished_at_ms.unwrap() >= outcome.started_at_ms);
}

#[test]
fn test_a_second_update_while_one_runs_is_refused() {
    let home = Home::new("locked");
    let server = Server::latest("v99.0.0");
    let updates = home.paths().updates_dir();
    fs::create_dir_all(&updates).unwrap();
    let held = fs::File::create(updates.join("update.lock")).unwrap();
    held.lock().unwrap();

    let output = home.update(&[], &server.base());

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("another `allot update` is running"),
        "{}",
        stderr(&output)
    );
}
