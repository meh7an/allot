//! The resources-helper daemon: system sampling, the lease ledger, their persistence, and the
//! HTTP API that serves them.

pub mod db;
mod error;
mod routes;
mod sampling;
pub mod startup;
mod state;

use std::{future::Future, sync::Arc, time::Duration};

use rh_shared::{api::LeaseQuery, config::Config, types::LeaseState};
use tokio::net::TcpListener;

pub use crate::sampling::SamplerFactory;
use crate::{
    db::{DbPool, repo},
    state::{Shared, now_ms},
};

/// What the daemon needs to serve, prepared by `main` or by a test.
pub struct DaemonOptions {
    /// The loaded configuration.
    pub config: Config,
    /// The bearer token clients must send.
    pub token: String,
    /// The opened, migrated database.
    pub pool: DbPool,
    /// Builds the sampler on the sampling thread.
    pub sampler: SamplerFactory,
    /// Time between samples; one second in production, since the rolling windows count
    /// samples.
    pub sample_interval: Duration,
}

/// Serves the API on `listener` until `shutdown` resolves.
///
/// Leases left queued or active by a previous run are expired first, since nothing tracks
/// them any more.
///
/// # Errors
///
/// Fails when the database cannot be read, the sampling thread cannot start, or the server
/// stops with an I/O error.
pub async fn serve(
    listener: TcpListener,
    options: DaemonOptions,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let state = Arc::new(Shared::new(options.config, options.token, options.pool));
    expire_leftovers(&state).await?;
    let samples = sampling::spawn_sampler(options.sampler, options.sample_interval)?;
    let sampling = tokio::spawn(sampling::run(state.clone(), samples));

    tracing::info!("listening on {}", listener.local_addr()?);
    let served = axum::serve(listener, routes::router(state))
        .with_graceful_shutdown(shutdown)
        .await;
    sampling.abort();
    tracing::info!("stopped");
    Ok(served?)
}

async fn expire_leftovers(state: &Shared) -> anyhow::Result<()> {
    let now = now_ms();
    let expired = state
        .with_db(move |conn| {
            let mut expired = 0;
            for leftover_state in [LeaseState::Queued, LeaseState::Active] {
                let query = LeaseQuery {
                    session: None,
                    state: Some(leftover_state),
                };
                for lease in repo::list_leases(conn, &query)? {
                    repo::transition_lease(
                        conn,
                        lease.id,
                        LeaseState::Expired,
                        now,
                        "daemon restarted",
                    )?;
                    expired += 1;
                }
            }
            Ok(expired)
        })
        .await?;
    if expired > 0 {
        tracing::info!("expired {expired} lease(s) left over from the previous run");
    }
    Ok(())
}
