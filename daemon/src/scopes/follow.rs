//! Following a command the PreToolUse hook reported into its lease's scope.

use std::time::Duration;

use allot_core::binding::REBIND_WINDOW_MS;
use allot_shared::types::{Holder, JobProfile, LeaseId, LeaseIsolation};

use crate::{
    scopes::{ScopeError, host::ProcInfo, is_protected_name},
    state::AppState,
};

/// Starts the scope of lease `id`, granted at `granted_at_ms`, if it has none and moves the
/// command `holder` is about to start into it, in the background. Called before the bind request answers, so the holder's
/// children are noted before the hook exits and Claude Code starts the command's shell. Does
/// nothing when scopes are off in the config or unavailable on this machine.
pub(crate) fn follow_command(
    state: &AppState,
    id: LeaseId,
    granted_at_ms: i64,
    holder: &Holder,
    profile: JobProfile,
) {
    let config = state.settings().config.linux.clone();
    if !config.scopes {
        return;
    }
    let Some(mut watch) = state.scopes.watch(id, holder.pid) else {
        return;
    };
    let state = state.clone();
    tokio::spawn(async move {
        let scoped = async {
            let isolation = state
                .scopes
                .ensure(id, granted_at_ms, &profile, &config)
                .await?;
            record(&state, id, isolation).await;
            Ok::<(), ScopeError>(())
        };
        let window = Duration::from_millis(u64::try_from(REBIND_WINDOW_MS).unwrap_or_default());
        let (scoped, shell) = tokio::join!(scoped, state.scopes.find_shell(&mut watch, window));
        if let Err(error) = scoped {
            tracing::warn!(
                lease = id.0,
                "no scope, following the process tree: {error}"
            );
            return;
        }
        let Some(shell) = shell else {
            tracing::info!(lease = id.0, "no command shell appeared for the scope");
            return;
        };
        let holders = state.holder_pids();
        let protected =
            |pid: u32, info: &ProcInfo| holders.contains(&pid) || is_protected_name(&info.comm);
        match state.scopes.settle(id, shell, &protected).await {
            Ok(moved) => tracing::info!(lease = id.0, pid = shell, moved, "command scoped"),
            Err(error) => {
                tracing::warn!(
                    lease = id.0,
                    pid = shell,
                    "scoping the command failed: {error}"
                );
            }
        }
    });
}

/// Shows the lease as isolated in its cgroup, unless it ended meanwhile.
async fn record(state: &AppState, id: LeaseId, isolation: LeaseIsolation) {
    let mut ledger = state.ledger.lock().await;
    // A lease that ended meanwhile is left alone; the next sample releases its scope.
    let _ = ledger.set_isolation(id, isolation);
}
