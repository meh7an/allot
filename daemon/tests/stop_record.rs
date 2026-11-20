//! Runs the built allotd under a temporary home: a panel's Stop service leaves the stop record
//! behind, and a start, the way a panel's Start or a sign-in launcher does it, ends the stop.

use std::{
    fs,
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use allot_shared::{
    api::ShutdownRequest,
    paths::Paths,
    platform::HOME_VAR,
    stop::{StopRecord, StoppedBy},
};
use tempfile::TempDir;

const READY_TIMEOUT: Duration = Duration::from_secs(30);
const EXIT_TIMEOUT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(50);

/// The allotd this test started; dropping it kills it if the test did not stop it. Only this
/// child is ever killed.
struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

impl Daemon {
    /// Starts allotd with `home` as its home directory, listening on `port`.
    fn start(home: &Path, port: u16) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_allotd"))
            .env(HOME_VAR, home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut daemon = Self(child);
        let deadline = Instant::now() + READY_TIMEOUT;
        while TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err() {
            assert!(
                daemon.0.try_wait().unwrap().is_none(),
                "allotd exited before it listened"
            );
            assert!(Instant::now() < deadline, "allotd did not listen in time");
            thread::sleep(POLL);
        }
        daemon
    }

    /// Asks allotd to shut down through its API, with `request` as the body when set, then
    /// waits for it to exit.
    async fn shut_down(mut self, paths: &Paths, port: u16, request: Option<ShutdownRequest>) {
        let token = fs::read_to_string(paths.token_file()).unwrap();
        let mut post = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/shutdown"))
            .bearer_auth(token.trim());
        if let Some(request) = request {
            post = post.json(&request);
        }
        let response = post.send().await.unwrap();
        assert!(response.status().is_success(), "{}", response.status());
        let deadline = Instant::now() + EXIT_TIMEOUT;
        while self.0.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "allotd did not exit in time");
            tokio::time::sleep(POLL).await;
        }
    }
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn test_allotd_start_clears_the_stop_record() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let port = free_port();
    fs::create_dir(paths.root()).unwrap();
    fs::write(paths.config_file(), format!("[daemon]\nport = {port}\n")).unwrap();
    StopRecord::now(StoppedBy::Panel)
        .write(&paths.stop_file())
        .unwrap();

    let daemon = Daemon::start(home.path(), port);

    assert!(!paths.stop_file().exists());
    daemon.shut_down(&paths, port, None).await;
    let log = fs::read_to_string(paths.log_dir().join("allotd.log")).unwrap();
    assert!(log.contains("stop record cleared"), "{log}");
}

#[tokio::test]
async fn test_allotd_stopped_from_a_panel_stays_stopped_until_started() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let port = free_port();
    fs::create_dir(paths.root()).unwrap();
    fs::write(paths.config_file(), format!("[daemon]\nport = {port}\n")).unwrap();

    let stop = ShutdownRequest {
        by: StoppedBy::Panel,
    };
    Daemon::start(home.path(), port)
        .shut_down(&paths, port, Some(stop))
        .await;

    let record = StopRecord::read(&paths.stop_file()).expect("the stop leaves its record");
    assert_eq!(record.by, StoppedBy::Panel);
    Daemon::start(home.path(), port)
        .shut_down(&paths, port, None)
        .await;
    assert!(!paths.stop_file().exists());
}
