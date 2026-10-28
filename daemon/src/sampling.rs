//! The sampling thread and the task that turns its samples into snapshots and ledger ticks.

use std::{
    collections::VecDeque,
    thread,
    time::{Duration, Instant},
};

use allot_core::{
    binding::{self, BindOutcome},
    idle::{IdleWatch, QUICK_OVERRUN_MS, quick_overruns},
    ledger::Ledger,
    process::ProcessTable,
    sampler::{RawSample, Sampler, SnapshotBuilder},
};
use allot_shared::types::{
    BoundProcess, EndCause, JobRun, LeaseBinding, LeaseId, LeaseState, SessionKey,
};
use anyhow::Context;
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
pub type SamplerFactory = Box<dyn FnOnce() -> allot_shared::Result<Box<dyn Sampler>> + Send>;

type SampleResult = allot_shared::Result<RawSample>;

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
        .name("allot-sampler".into())
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
    ledger.observe_scopes(&state.scopes.usage());
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
    state.scopes.retain(|id| {
        ledger
            .leases()
            .iter()
            .any(|lease| lease.id == id && lease.state == LeaseState::Active)
    });
    drop(ledger);
    for run in runs {
        routes::store_job_run(state, run).await;
    }
}

/// Ends, as expired, the active leases whose command has ended and whose holder's session
/// stayed idle for `leases.idle_release_secs`, and the quick leases still active after
/// [`QUICK_OVERRUN_MS`], and returns their history rows to store. An idle lease's row ends
/// when the session went idle, so the forgotten minutes do not count as run time; an overrun
/// quick lease's row ends now, so history learns the command is not quick.
async fn release_idle(
    state: &AppState,
    ledger: &mut Ledger,
    idle: &mut IdleWatch,
) -> anyhow::Result<Vec<JobRun>> {
    let Some(latest) = state.latest() else {
        return Ok(Vec::new());
    };
    let after_secs = state.settings().config.leases.idle_release_secs;
    let now = now_ms();
    let mut due: Vec<(LeaseId, i64, String)> = idle
        .observe(
            ledger.leases(),
            &latest.snapshot,
            &latest.processes,
            Duration::from_secs(after_secs),
        )
        .into_iter()
        .map(|(id, since_ms)| {
            let reason = format!("no command running and the session idle for {after_secs}s");
            (id, since_ms, reason)
        })
        .collect();
    let overrun_secs = QUICK_OVERRUN_MS / 1_000;
    for id in quick_overruns(ledger.leases(), now) {
        if due.iter().all(|(ended, ..)| *ended != id) {
            let reason = format!("quick lease still active after {overrun_secs}s");
            due.push((id, now, reason));
        }
    }
    if due.is_empty() {
        return Ok(Vec::new());
    }
    let mut runs = Vec::new();
    for (id, run_ended_ms, reason) in due {
        let released = ledger.end(id, EndCause::Expired, now)?;
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
                run_ended_ms,
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
