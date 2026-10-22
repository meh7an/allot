use clap::{Parser, Subcommand};

mod probe;
mod sample;

/// Inspect machine usage and leases, control the daemon, and run the Claude Code hook.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Record this process's ancestry, environment, and optionally stdin to a file, or print
    /// live machine samples with `--sample`.
    Probe(probe::ProbeArgs),
}

fn main() -> anyhow::Result<()> {
    match Args::parse().command {
        Command::Probe(args) => probe::run(&args),
    }
}
