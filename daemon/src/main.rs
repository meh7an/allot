use clap::Parser;

/// Resource coordinator daemon: samples system usage and arbitrates heavy-job leases.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {}

fn main() -> anyhow::Result<()> {
    Args::parse();
    Ok(())
}
