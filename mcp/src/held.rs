//! Leases this server acquired: kept alive with heartbeats and released when it exits.

use std::{
    collections::BTreeSet,
    sync::{Mutex, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use allot_shared::types::{LeaseId, LeaseState, SessionKey};
use tokio::sync::Notify;

use crate::daemon::DaemonApi;

/// The shortest wait between heartbeats, however little time a lease has left.
const MIN_BEAT: Duration = Duration::from_secs(1);
/// The wait when no held lease answered: none is held, or allotd is unreachable.
const UNKNOWN_BEAT: Duration = Duration::from_secs(5);

/// The lease IDs this process holds.
#[derive(Debug, Default)]
pub struct HeldLeases {
    ids: Mutex<BTreeSet<LeaseId>>,
    added: Notify,
}

impl HeldLeases {
    /// Starts heartbeating `id`.
    pub fn insert(&self, id: LeaseId) {
        self.lock().insert(id);
        self.added.notify_one();
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
        self.ids.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Heartbeats every held lease once and returns how long to wait before the next round.
/// Leases that ended, or that the daemon no longer knows, are dropped from the set.
pub async fn heartbeat_once<C: DaemonApi>(
    client: &C,
    session: &SessionKey,
    held: &HeldLeases,
) -> Duration {
    let mut expiries = Vec::new();
    for id in held.ids() {
        match client.heartbeat(id, session).await {
            Ok(response) if is_ended(response.state) || response.end_cause.is_some() => {
                if let Some(cause) = response.end_cause.filter(|cause| cause.is_notice()) {
                    eprintln!(
                        "allot-mcp: lease {} was {} by the user",
                        id.0,
                        cause.as_str()
                    );
                }
                held.remove(id);
            }
            Ok(response) => expiries.push(response.expires_at_ms),
            Err(error) if matches!(error.status(), Some(403 | 404)) => held.remove(id),
            Err(error) => eprintln!("allot-mcp: heartbeat for lease {} failed: {error}", id.0),
        }
    }
    next_beat(&expiries, now_ms())
}

/// Heartbeats every held lease, again after the wait each round returns and at once when a
/// lease is added, forever.
pub async fn heartbeat_loop<C: DaemonApi>(client: &C, session: &SessionKey, held: &HeldLeases) {
    loop {
        let wait = heartbeat_once(client, session, held).await;
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            () = held.added.notified() => {}
        }
    }
}

/// A third of the shortest time any lease has left at `now_ms`, so two heartbeats can be lost
/// before it expires. Taken from each answer rather than the config, because the lease
/// timeout can change while the session runs.
fn next_beat(expiries: &[i64], now_ms: i64) -> Duration {
    let Some(soonest) = expiries.iter().min() else {
        return UNKNOWN_BEAT;
    };
    let left_ms = u64::try_from(soonest.saturating_sub(now_ms)).unwrap_or_default();
    Duration::from_millis(left_ms / 3).max(MIN_BEAT)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// Releases every held lease; used when the session closes this server.
pub async fn release_all<C: DaemonApi>(client: &C, session: &SessionKey, held: &HeldLeases) {
    for id in held.ids() {
        if let Err(error) = client.release(id, session).await {
            eprintln!("allot-mcp: releasing lease {} failed: {error}", id.0);
        }
        held.remove(id);
    }
}

fn is_ended(state: LeaseState) -> bool {
    matches!(state, LeaseState::Released | LeaseState::Expired)
}
#[cfg(test)]
mod tests {
    use allot_shared::types::EndCause;

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

    #[test]
    fn test_next_beat_follows_the_soonest_expiry() {
        // A lease timeout lowered to 15 s shows in the next answer, not in the config read at
        // startup.
        assert_eq!(
            next_beat(&[100_000 + 60_000, 100_000 + 15_000], 100_000),
            Duration::from_secs(5)
        );
        assert_eq!(next_beat(&[100_000 + 1_500], 100_000), MIN_BEAT);
        assert_eq!(next_beat(&[90_000], 100_000), MIN_BEAT);
        assert_eq!(next_beat(&[], 100_000), UNKNOWN_BEAT);
    }

    #[tokio::test]
    async fn test_heartbeat_once_waits_briefly_when_nothing_answers() {
        let held = HeldLeases::default();
        held.insert(LeaseId(1));
        let wait = heartbeat_once(&MockDaemon::unreachable(), &me(), &held).await;
        assert_eq!(wait, UNKNOWN_BEAT);
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
