//! Manual check: replace a config file while an editor has it open.
//!
//! Usage: `cargo run --example held_write -- <copy of config.toml>`. Open the copy in an
//! editor first, run this, then look at what the editor shows and what is on disk. It flips
//! `admission.headroom_pct` between 20 and 25 and reports how the write went.

use std::{env, path::PathBuf, process::ExitCode, time::Duration};

use config_writeback::{
    edit::{self, Layout},
    file::{self, Retry, WriteError},
};
use toml_edit::DocumentMut;

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: held_write <path to a copy of config.toml>");
        return ExitCode::FAILURE;
    };
    let (text, revision) = match file::read(&path) {
        Ok(read) => read,
        Err(error) => {
            eprintln!("cannot read {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let mut doc: DocumentMut = match text.parse() {
        Ok(doc) => doc,
        Err(error) => {
            eprintln!("not TOML: {error}");
            return ExitCode::FAILURE;
        }
    };
    let current = doc
        .get("admission")
        .and_then(|admission| admission.get("headroom_pct"))
        .and_then(|value| value.as_float());
    let next = if current == Some(25.0) { 20.0 } else { 25.0 };
    edit::set(&mut doc, &["admission", "headroom_pct"], next);

    let retry = Retry {
        attempts: 40,
        delay: Duration::from_millis(25),
    };
    match file::write(&path, &Layout::of(&text).render(&doc), revision, retry) {
        Ok(written) => {
            println!(
                "headroom_pct set to {next}: {} attempt(s), race window {:?}",
                written.attempts, written.race_window
            );
            ExitCode::SUCCESS
        }
        Err(WriteError::Conflict { .. }) => {
            eprintln!("the file changed while it was being edited; nothing written");
            ExitCode::FAILURE
        }
        Err(WriteError::Io(error)) => {
            eprintln!("write failed after {} attempts: {error}", retry.attempts);
            ExitCode::FAILURE
        }
    }
}
