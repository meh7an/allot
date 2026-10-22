use clap::Parser;

/// Inspect machine usage and leases, control the daemon, and run the Claude Code hook.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {}

fn main() -> anyhow::Result<()> {
    Args::parse();
    Ok(())
}
