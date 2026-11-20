//! Manual check: replace a config file while an editor has it open.
//!
//! Usage: `cargo run -p allot-shared --example held_write -- <copy of config.toml>`. Open the
//! copy in an editor first, run this, then look at what the editor shows and what is on disk.
//! It flips `admission.headroom_pct` between 20 and 25 through the real config writer, with
//! its retries while the file is held, and reports how the write went. Never point it at the
//! `config.toml` allotd uses.

use std::{env, path::PathBuf, process::ExitCode, time::Instant};

use allot_shared::config_write::{self, Change};

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: held_write <path to a copy of config.toml>");
        return ExitCode::FAILURE;
    };
    let snapshot = match config_write::read(&path) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("cannot read {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let current = config_write::edit(&snapshot.text, &[])
        .map(|edited| edited.config.admission.headroom_pct)
        .unwrap_or_default();
    let next = if current == 25.0 { 20.0 } else { 25.0 };
    let change = Change::SetNumber {
        key: "admission.headroom_pct".to_owned(),
        value: next,
    };

    let started = Instant::now();
    match config_write::apply(&path, snapshot.revision, &[change]) {
        Ok(applied) => {
            println!(
                "headroom_pct set to {next} in {:?}, revision {}",
                started.elapsed(),
                applied.revision
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("nothing written after {:?}: {error}", started.elapsed());
            ExitCode::FAILURE
        }
    }
}
