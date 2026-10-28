// No console window when the logon task starts allotd. Output goes to allotd.log, not the console.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{net::Ipv4Addr, time::Duration};

use allot_core::sampler::Sampler;
use allot_sampler::SystemSampler;
use allot_shared::{config::Config, paths::Paths};
use allotd::{DaemonOptions, Maintenance, Scopes, db, startup};
use anyhow::Context;
use clap::Parser;
use tokio::net::TcpListener;

/// Resource coordinator daemon: samples system usage and arbitrates heavy-job leases.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// Run as the idle anchor that keeps a lease's cgroup scope alive; allotd starts these.
    #[cfg(target_os = "linux")]
    #[arg(long = "anchor", value_name = "SCOPE", hide = true)]
    anchor: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    #[cfg(target_os = "linux")]
    if args.anchor.is_some() {
        // It only has to exist: it holds nothing, and allotd ends it with SIGKILL.
        loop {
            std::thread::park();
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = args;
    daemon()
}

#[tokio::main]
async fn daemon() -> anyhow::Result<()> {
    let paths = Paths::from_env()?;
    startup::create_data_dir(paths.root())?;
    let _lock = startup::InstanceLock::acquire(&paths.lock_file())?;
    startup::init_logging(&paths.log_dir())?;

    // A detached allotd has no console, so the log is the only place a failure shows up.
    let result = run(&paths).await;
    if let Err(error) = &result {
        tracing::error!("daemon failed: {error:#}");
    }
    result
}

async fn run(paths: &Paths) -> anyhow::Result<()> {
    let config_file = paths.config_file();
    let config = Config::load(&config_file)?;
    let token = startup::load_or_create_token(&paths.token_file())?;
    let pool = db::open_pool(&paths.database_file())?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.daemon.port))
        .await
        .with_context(|| {
            format!(
                "binding 127.0.0.1:{}; another program may use the port (set daemon.port in {})",
                config.daemon.port,
                config_file.display()
            )
        })?;

    let options = DaemonOptions {
        config,
        token,
        pool,
        sampler: Box::new(|| Ok(Box::new(SystemSampler::new()?) as Box<dyn Sampler>)),
        sample_interval: Duration::from_secs(1),
        maintenance: Maintenance::standard(config_file),
        scopes: Scopes::detect(paths.root()).await,
    };
    allotd::serve(listener, options, shutdown_signal()).await
}

async fn shutdown_signal() {
    tokio::select! {
        () = interrupt() => {}
        () = terminate() => {}
    }
    tracing::info!("shutdown requested");
}

async fn interrupt() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!("listening for Ctrl+C failed: {error}");
        std::future::pending::<()>().await;
    }
}

/// SIGTERM, which `kill`, systemd and launchd send to stop a service.
#[cfg(unix)]
async fn terminate() {
    use tokio::signal::unix::{SignalKind, signal};

    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            terminate.recv().await;
        }
        Err(error) => {
            tracing::error!("listening for SIGTERM failed: {error}");
            std::future::pending::<()>().await;
        }
    }
}

/// Windows has no SIGTERM; allotd stops through `POST /shutdown` or Ctrl+C.
#[cfg(not(unix))]
async fn terminate() {
    std::future::pending::<()>().await;
}
