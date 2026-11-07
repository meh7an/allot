//! Repository tasks, run as `cargo xtask <task>`. Only the standard library, so a task costs
//! the workspace build next to nothing.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

mod version;

const USAGE: &str = "usage:
  cargo xtask version <x.y.z>   set the version everywhere and promote the changelog
  cargo xtask version --check   check that every place agrees";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let root = repo_root();
    let result = match args.as_slice() {
        ["version", "--check"] => version::check(&root),
        ["version", new] => version::set(&root, new),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}
