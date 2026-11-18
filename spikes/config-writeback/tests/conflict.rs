//! Atomic writes and edits saved by hand between allotd's read and its write.

use std::{fs, path::Path};

use allot_shared::config::Config;
use config_writeback::{
    edit,
    file::{self, Retry, WriteError},
};
use tempfile::TempDir;
use toml_edit::DocumentMut;

const START: &str = "# mine\n[admission]\nheadroom_pct = 20.0  # gaming\n";

fn set_headroom(text: &str, pct: f64) -> String {
    let mut doc: DocumentMut = text.parse().unwrap();
    edit::set(&mut doc, &["admission", "headroom_pct"], pct);
    doc.to_string()
}

fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != "config.toml")
        .collect()
}

#[test]
fn test_write_unchanged_file_replaces_it_and_cleans_up() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, START).unwrap();

    let (text, revision) = file::read(&path).unwrap();
    let written = file::write(&path, &set_headroom(&text, 30.0), revision, Retry::NONE).unwrap();

    let (now, now_revision) = file::read(&path).unwrap();
    assert_eq!(now, "# mine\n[admission]\nheadroom_pct = 30.0  # gaming\n");
    assert_eq!(written.revision, now_revision);
    assert_eq!(written.attempts, 1);
    assert!(leftovers(dir.path()).is_empty());
    println!("race window: {:?}", written.race_window);
}

#[test]
fn test_write_after_hand_edit_conflicts_and_leaves_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, START).unwrap();

    let (text, revision) = file::read(&path).unwrap();
    let ours = set_headroom(&text, 30.0);
    let hand = format!("{START}\n[estimates]\nlower = false\n");
    fs::write(&path, &hand).unwrap();

    let result = file::write(&path, &ours, revision, Retry::NONE);
    assert!(matches!(result, Err(WriteError::Conflict { .. })));
    assert_eq!(fs::read_to_string(&path).unwrap(), hand);
    assert!(leftovers(dir.path()).is_empty());

    // A single-key change can be applied again on top of the hand edit.
    let (text, revision) = file::read(&path).unwrap();
    file::write(&path, &set_headroom(&text, 30.0), revision, Retry::NONE).unwrap();
    let config = Config::load(&path).unwrap();
    assert_eq!(config.admission.headroom_pct, 30.0);
    assert!(!config.estimates.lower);
}

#[test]
fn test_write_missing_file_creates_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let (text, revision) = file::read(&path).unwrap();
    assert_eq!(text, "");
    file::write(&path, &set_headroom(&text, 25.0), revision, Retry::NONE).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[admission]\nheadroom_pct = 25.0\n"
    );
}
