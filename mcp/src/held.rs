//! Leases this server acquired: kept alive with heartbeats and released when it exits.

use std::{
    collections::BTreeSet,
    sync::{Mutex, PoisonError},
    time::Duration,
};

use rh_shared::types::{LeaseId, LeaseState, SessionKey};

use crate::daemon::DaemonApi;

/// The lease IDs this process holds.
#[derive(Debug, Default)]
pub struct HeldLeases(Mutex<BTreeSet<LeaseId>>);

impl HeldLeases {
    /// Starts heartbeating `id`.
    pub fn insert(&self, id: LeaseId) {
        self.lock().insert(id);
    }

    /// Stops heartbeating `id`.
    pub fn remove(&self, id: LeaseId) {
        self.lock().remove(&id);
    }

    /// The held IDs, in order.
    pub fn ids(&self) -> Vec<LeaseId> {
        self.lock().iter().copied().collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeSet<LeaseId>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Heartbeats every held lease once. Leases that ended, or that the daemon no longer knows,
/// are dropped from the set.
pub async fn heartbeat_once<C: DaemonApi>(client: &C, session: &SessionKey, held: &HeldLeases) {
    for id in held.ids() {
        match client.heartbeat(id, session).await {
            Ok(response) if is_ended(response.state) || response.end_cause.is_some() => {
                if let Some(cause) = response.end_cause.filter(|cause| cause.is_notice()) {
                    eprintln!("rh-mcp: lease {} was {} by the user", id.0, cause.as_str());
                }
                held.remove(id);
            }
            Ok(_) => {}
            Err(error) if matches!(error.status(), Some(403 | 404)) => held.remove(id),
            Err(error) => eprintln!("rh-mcp: heartbeat for lease {} failed: {error}", id.0),
        }
    }
}

/// Heartbeats every held lease each `every`, forever.
pub async fn heartbeat_loop<C: DaemonApi>(
    client: &C,
    session: &SessionKey,
    held: &HeldLeases,
    every: Duration,
) {
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        heartbeat_once(client, session, held).await;
    }
}

/// Releases every held lease; used when the session closes this server.
pub async fn release_all<C: DaemonApi>(client: &C, session: &SessionKey, held: &HeldLeases) {
    for id in held.ids() {
        if let Err(error) = client.release(id, session).await {
            eprintln!("rh-mcp: releasing lease {} failed: {error}", id.0);
        }
        held.remove(id);
    }
}

fn is_ended(state: LeaseState) -> bool {
    matches!(state, LeaseState::Released | LeaseState::Expired)
}

#[cfg(test)]
mod tests {
    use rh_shared::types::EndCause;

    use super::*;
    use crate::test_support::{MockDaemon, holder, lease};

    fn me() -> SessionKey {
        holder("me").session
    }

    #[tokio::test]
    async fn test_heartbeat_once_keeps_live_leases_and_drops_ended_or_unknown() {
        let daemon = MockDaemon::default();
        for (id, state) in [
            (1, LeaseState::Active),
            (2, LeaseState::Queued),
            (3, LeaseState::Expired),
        ] {
            daemon.add_lease(lease(id, holder("me"), state));
        }
        let held = HeldLeases::default();
        for id in [1, 2, 3, 4] {
            held.insert(LeaseId(id));
        }

        heartbeat_once(&daemon, &me(), &held).await;
        assert_eq!(held.ids(), [LeaseId(1), LeaseId(2)]);
        assert_eq!(daemon.heartbeats().len(), 4);
    }

    #[tokio::test]
    async fn test_heartbeat_once_drops_leases_the_user_stopped() {
        let daemon = MockDaemon::default();
        daemon.add_lease(lease(1, holder("me"), LeaseState::Active));
        daemon.add_lease(lease(2, holder("me"), LeaseState::Active));
        daemon.stop(LeaseId(1), EndCause::Killed);
        let held = HeldLeases::default();
        held.insert(LeaseId(1));
        held.insert(LeaseId(2));

        heartbeat_once(&daemon, &me(), &held).await;
        assert_eq!(held.ids(), [LeaseId(2)]);
    }

    #[tokio::test]
    async fn test_heartbeat_once_keeps_leases_when_the_daemon_is_down() {
        let held = HeldLeases::default();
        held.insert(LeaseId(1));
        heartbeat_once(&MockDaemon::unreachable(), &me(), &held).await;
        assert_eq!(held.ids(), [LeaseId(1)]);
    }

    #[tokio::test]
    async fn test_release_all_releases_and_forgets_every_lease() {
        let daemon = MockDaemon::default();
        daemon.add_lease(lease(1, holder("me"), LeaseState::Active));
        daemon.add_lease(lease(2, holder("me"), LeaseState::Queued));
        let held = HeldLeases::default();
        held.insert(LeaseId(1));
        held.insert(LeaseId(2));

        release_all(&daemon, &me(), &held).await;
        assert_eq!(daemon.releases(), [LeaseId(1), LeaseId(2)]);
        assert!(held.ids().is_empty());
    }
}
