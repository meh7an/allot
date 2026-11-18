//! Windows: replacing `config.toml` while another process holds it open.
//!
//! The `hold` binary opens the file with a given share mode, like an editor or a scanner,
//! and keeps it open until its stdin closes. Each test ends only the holders it started.
#![cfg(windows)]

use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use config_writeback::file::{self, Retry, WriteError};
use tempfile::TempDir;

const SHARE_READ: u32 = 1;
const SHARE_WRITE: u32 = 2;
const SHARE_DELETE: u32 = 4;

const START: &str = "[admission]\nheadroom_pct = 20.0\n";
const NEXT: &str = "[admission]\nheadroom_pct = 30.0\n";

/// A spawned `hold` process, closed and reaped on drop.
struct Holder(Child);

impl Holder {
    fn open(path: &Path, share: u32) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_hold"))
            .arg(path)
            .arg(share.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert_eq!(line.trim(), "ready", "hold did not open the file");
        Self(child)
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        drop(self.0.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.0.try_wait() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn describe(result: &Result<file::Written, WriteError>) -> String {
    match result {
        Ok(written) => format!("replaced in {:?}", written.race_window),
        Err(WriteError::Conflict { .. }) => "conflict".into(),
        Err(WriteError::Io(error)) => format!("os error {:?}", error.raw_os_error()),
    }
}

/// One write per share mode. Only a holder that allows delete lets the rename through; it
/// keeps reading the old file. Every other holder blocks it, and one that shares nothing
/// even blocks reading the file.
#[test]
fn test_replace_by_share_mode() {
    let cases = [
        ("none", 0, Some(32)),
        ("read", SHARE_READ, Some(5)),
        ("read+write", SHARE_READ | SHARE_WRITE, Some(5)),
        (
            "read+write+delete",
            SHARE_READ | SHARE_WRITE | SHARE_DELETE,
            None,
        ),
    ];
    let mut report = Vec::new();
    for (name, share, expected_error) in cases {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, START).unwrap();
        let revision = file::read(&path).unwrap().1;

        let holder = Holder::open(&path, share);
        let started = Instant::now();
        let result = file::write(&path, NEXT, revision, Retry::NONE);
        let took = started.elapsed();
        drop(holder);

        report.push(format!(
            "{name:>18}: {} (call took {took:?})",
            describe(&result)
        ));
        match (expected_error, &result) {
            (None, Ok(_)) => assert_eq!(fs::read_to_string(&path).unwrap(), NEXT),
            (Some(code), Err(WriteError::Io(error))) => {
                assert_eq!(error.raw_os_error(), Some(code), "{name}");
                assert_eq!(fs::read_to_string(&path).unwrap(), START, "{name}");
            }
            _ => panic!("{name}: unexpected {}", describe(&result)),
        }
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(names.len(), 1, "{name}: temporary file left behind");
    }
    println!("{}", report.join("\n"));
}

/// A holder that lets go within the retry budget only delays the write.
#[test]
fn test_retry_outlasts_short_hold() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, START).unwrap();
    let revision = file::read(&path).unwrap().1;

    let holder = Holder::open(&path, SHARE_READ | SHARE_WRITE);
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        drop(holder);
    });
    let retry = Retry {
        attempts: 40,
        delay: Duration::from_millis(25),
    };
    let started = Instant::now();
    let written = file::write(&path, NEXT, revision, retry).unwrap();
    println!(
        "replaced after {} attempts, {:?}",
        written.attempts,
        started.elapsed()
    );
    release.join().unwrap();
    assert!(written.attempts > 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), NEXT);
}
