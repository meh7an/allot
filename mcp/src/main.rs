use clap::Parser;

/// Stdio MCP server exposing system snapshots and heavy-job leases to one Claude session.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {}

fn main() -> anyhow::Result<()> {
    Args::parse();
    Ok(())
}
