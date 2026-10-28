//! Runs the built allotd under a temporary home and stops it with SIGTERM, the way `kill`,
//! systemd and launchd stop a service.

#![cfg(unix)]

use std::{
    fs,
    net::{Ipv4Addr, TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use tempfile::TempDir;

const READY_TIMEOUT: Duration = Duration::from_secs(30);
const EXIT_TIMEOUT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(50);

/// The allotd this test started; dropping it kills it if the test did not stop it.
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
        let root = home.join(".allot");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("config.toml"),
            format!("[daemon]\nport = {port}\n"),
        )
        .unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_allotd"))
            .env("HOME", home)
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

    fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .args([name, &self.0.id().to_string()])
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + EXIT_TIMEOUT;
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "allotd did not exit in time");
            thread::sleep(POLL);
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

#[test]
fn test_sigterm_shuts_allotd_down_gracefully() {
    let home = TempDir::new().unwrap();
    let mut daemon = Daemon::start(home.path(), free_port());

    daemon.signal("-TERM");
    let status = daemon.wait();

    assert!(status.success(), "{status}");
    let log = fs::read_to_string(home.path().join(".allot/logs/allotd.log")).unwrap();
    assert!(log.contains("shutdown requested"), "{log}");
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn test_allotd_keeps_its_data_dir_and_token_to_the_user() {
    let home = TempDir::new().unwrap();
    let mut daemon = Daemon::start(home.path(), free_port());
    daemon.signal("-TERM");
    daemon.wait();

    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let root = home.path().join(".allot");
    assert_eq!(mode(&root), 0o700);
    assert_eq!(mode(&root.join("token")), 0o600);
}
