use clap::{Parser, Subcommand};

mod daemon;
mod probe;
mod sample;
mod status;

/// Inspect machine usage and leases, control the daemon, and run the Claude Code hook.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show machine usage, sessions, and their leases. Starts rhd when it is not running.
    Status,
    /// List active and queued leases.
    Leases {
        /// Show the most recent leases in any state instead.
        #[arg(long)]
        all: bool,
    },
    /// Start, stop, or check the background daemon.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Record this process's ancestry, environment, and optionally stdin to a file, or print
    /// live machine samples with `--sample`.
    Probe(probe::ProbeArgs),
}

#[derive(Debug, Subcommand)]
enum DaemonAction {
    /// Start rhd in the background unless it is already running.
    Start,
    /// Ask rhd to shut down and wait until it has.
    Stop,
    /// Report whether rhd is running, without starting it.
    Status,
}

fn main() -> anyhow::Result<()> {
    match Args::parse().command {
        Command::Status => status::status(),
        Command::Leases { all } => status::leases(all),
        Command::Daemon { action } => match action {
            DaemonAction::Start => daemon::start(),
            DaemonAction::Stop => daemon::stop(),
            DaemonAction::Status => daemon::status(),
        },
        Command::Probe(args) => probe::run(&args),
    }
}
