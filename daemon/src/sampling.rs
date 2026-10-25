//! The sampling thread and the task that turns its samples into snapshots and ledger ticks.

use std::{
    collections::VecDeque,
    thread,
    time::{Duration, Instant},
};

use anyhow::Context;
use rh_core::{
    binding::{self, BindOutcome},
    idle::IdleWatch,
    ledger::Ledger,
    process::ProcessTable,
    sampler::{RawSample, Sampler, SnapshotBuilder},
};
use rh_shared::types::{
    BoundProcess, EndCause, JobRun, LeaseBinding, LeaseId, LeaseState, SessionKey,
};
use tokio::sync::mpsc;

use crate::{
    db::repo,
    routes,
    state::{AppState, Latest, now_ms},
};

/// Once-a-second CPU loads the flyout's sparkline shows.
const CPU_HISTORY: usize = 24;

/// Creates the sampler on the sampling thread. The Windows sampler holds PDH handles that
/// cannot move between threads, so it must be built where it is used.
pub type SamplerFactory = Box<dyn FnOnce() -> rh_shared::Result<Box<dyn Sampler>> + Send>;

type SampleResult = rh_shared::Result<RawSample>;

/// Starts the sampling thread. It samples every `interval` until the receiver is dropped.
///
/// # Errors
///
/// Fails when the OS refuses to start the thread.
pub fn spawn_sampler(
    factory: SamplerFactory,
    interval: Duration,
) -> anyhow::Result<mpsc::Receiver<SampleResult>> {
    let (sender, receiver) = mpsc::channel(4);
    thread::Builder::new()
        .name("rh-sampler".into())
        .spawn(move || {
            let mut sampler = match factory() {
                Ok(sampler) => sampler,
                Err(error) => {
                    // If the receiver is gone the daemon is shutting down anyway.
                    let _ = sender.blocking_send(Err(error));
                    return;
                }
            };
            let mut next = Instant::now();
            loop {
                next += interval;
                thread::sleep(next.saturating_duration_since(Instant::now()));
                if sender.blocking_send(sampler.sample()).is_err() {
                    return;
                }
            }
        })
        .context("starting the sampling thread")?;
    Ok(receiver)
}

/// Folds samples into snapshots, ticks the ledger, finds the processes of bound commands, and
/// ends leases their holder forgot after each one, until the sampling thread stops.
pub async fn run(state: AppState, mut samples: mpsc::Receiver<SampleResult>) {
    let mut builder = SnapshotBuilder::new();
    let mut cpu_history = VecDeque::with_capacity(CPU_HISTORY);
    let mut idle = IdleWatch::new();
    while let Some(sample) = samples.recv().await {
        match sample {
            Ok(raw) => ingest(&state, &mut builder, &mut cpu_history, &mut idle, raw).await,
            Err(error) => tracing::warn!("sampling failed: {error}"),
        }
    }
    tracing::warn!("sampling stopped");
}

async fn ingest(
    state: &AppState,
    builder: &mut SnapshotBuilder,
    cpu_history: &mut VecDeque<f32>,
    idle: &mut IdleWatch,
    raw: RawSample,
) {
    let processes = ProcessTable::new(&raw.processes);
    let sessions = state.live_sessions(&processes);
    let snapshot = builder.build(raw, &sessions);
    if cpu_history.len() == CPU_HISTORY {
        cpu_history.pop_front();
    }
    cpu_history.push_back(snapshot.cpu.usage_pct.avg_1s);
    state.set_latest(Latest {
        snapshot,
        processes,
        cpu_history_pct: cpu_history.iter().copied().collect(),
    });
    let mut ledger = state.ledger.lock().await;
    if let Err(error) = state.tick(&mut ledger).await {
        tracing::error!("persisting lease transitions failed: {error:#}");
    }
    if let Err(error) = rebind(state, &mut ledger).await {
        tracing::error!("persisting command bindings failed: {error:#}");
    }
    let runs = match release_idle(state, &mut ledger, idle).await {
        Ok(runs) => runs,
        Err(error) => {
            tracing::error!("ending idle leases failed: {error:#}");
            Vec::new()
        }
    };
    drop(ledger);
    for run in runs {
        routes::store_job_run(state, run).await;
    }
}

/// Ends the active leases whose command has ended and whose holder's session stayed idle for
/// `leases.idle_release_secs`, as expired, and returns their history rows to store. A row ends
/// when the session went idle, so the forgotten minutes do not count as run time.
async fn release_idle(
    state: &AppState,
    ledger: &mut Ledger,
    idle: &mut IdleWatch,
) -> anyhow::Result<Vec<JobRun>> {
    let Some(latest) = state.latest() else {
        return Ok(Vec::new());
    };
    let after_secs = state.settings().config.leases.idle_release_secs;
    let due = idle.observe(
        ledger.leases(),
        &latest.snapshot,
        &latest.processes,
        Duration::from_secs(after_secs),
    );
    if due.is_empty() {
        return Ok(Vec::new());
    }
    let now = now_ms();
    let mut runs = Vec::new();
    for (id, idle_since_ms) in due {
        let released = ledger.end(id, EndCause::Expired, now)?;
        let reason = format!("no command running and the session idle for {after_secs}s");
        tracing::info!(lease = id.0, %reason, "lease ended for its holder");
        state
            .with_db(move |conn| repo::end_lease(conn, id, EndCause::Expired, now, &reason))
            .await?;
        if let Some(run) = &released.run {
            runs.push(routes::job_run(
                state,
                &released.lease,
                run,
                EndCause::Expired,
                idle_since_ms,
            ));
        }
    }
    state.notify_changes();
    // The freed room may let queued leases start right away.
    state.tick(ledger).await?;
    Ok(runs)
}

/// Stores the root process of every active lease's bound command that the latest sample shows
/// for the first time, or anew after the earlier root exited.
async fn rebind(state: &AppState, ledger: &mut Ledger) -> anyhow::Result<()> {
    let Some(latest) = state.latest() else {
        return Ok(());
    };
    let now = now_ms();
    let found: Vec<(LeaseId, SessionKey, BoundProcess, LeaseBinding)> = ledger
        .leases()
        .iter()
        .filter(|lease| lease.state == LeaseState::Active)
        .filter_map(|lease| {
            let current = lease.binding.as_ref()?;
            match binding::resolve(current, &lease.holder, &latest.processes, now) {
                BindOutcome::Bound(root) if current.root != Some(root) => Some((
                    lease.id,
                    lease.holder.session.clone(),
                    root,
                    LeaseBinding {
                        root: Some(root),
                        ..current.clone()
                    },
                )),
                _ => None,
            }
        })
        .collect();
    for (id, session, root, found_binding) in found {
        ledger.bind(id, &session, found_binding.clone())?;
        tracing::info!(lease = id.0, pid = root.pid, "command process found");
        state
            .with_db(move |conn| repo::bind_lease(conn, id, &found_binding))
            .await?;
    }
    Ok(())
}
