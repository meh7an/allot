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
    startup::clear_stop_record(&paths.stop_file());

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
    // Before the bind: once the port answers, `kill` must find the handler, not the default
    // action that ends allotd without its graceful shutdown.
    let shutdown = shutdown_signal();
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
        stop_file: Some(paths.stop_file()),
    };
    allotd::serve(listener, options, shutdown).await
}

/// Installs the stop handlers when called, not when first polled, and resolves once one fires.
fn shutdown_signal() -> impl Future<Output = ()> + Send + 'static {
    let interrupt = interrupt();
    let terminate = terminate();
    async move {
        tokio::select! {
            () = interrupt => {}
            () = terminate => {}
        }
        tracing::info!("shutdown requested");
    }
}

#[cfg(unix)]
fn interrupt() -> impl Future<Output = ()> + Send + 'static {
    listen(tokio::signal::unix::SignalKind::interrupt(), "Ctrl+C")
}

/// SIGTERM, which `kill`, systemd and launchd send to stop a service.
#[cfg(unix)]
fn terminate() -> impl Future<Output = ()> + Send + 'static {
    listen(tokio::signal::unix::SignalKind::terminate(), "SIGTERM")
}

#[cfg(unix)]
fn listen(
    kind: tokio::signal::unix::SignalKind,
    name: &'static str,
) -> impl Future<Output = ()> + Send + 'static {
    let registered = tokio::signal::unix::signal(kind);
    async move {
        match registered {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!("listening for {name} failed: {error}");
                std::future::pending::<()>().await;
            }
        }
    }
}

#[cfg(windows)]
fn interrupt() -> impl Future<Output = ()> + Send + 'static {
    let registered = tokio::signal::windows::ctrl_c();
    async move {
        match registered {
            Ok(mut ctrl_c) => {
                ctrl_c.recv().await;
            }
            Err(error) => {
                tracing::error!("listening for Ctrl+C failed: {error}");
                std::future::pending::<()>().await;
            }
        }
    }
}

/// Windows has no SIGTERM; allotd stops through `POST /shutdown` or Ctrl+C.
#[cfg(windows)]
fn terminate() -> impl Future<Output = ()> + Send + 'static {
    std::future::pending()
}
