//! The daemon calls the tools make, behind a trait so tests can use a mock.

use std::future::Future;

use rh_client::{Client, Result};
use rh_shared::{
    api::{
        AcquireRequest, AcquireResponse, CanFitRequest, HeartbeatResponse, LeaseQuery,
        ReleaseResponse, Verdict,
    },
    types::{Holder, Lease, LeaseId, SessionKey, Snapshot},
    ui::Notice,
};

/// The subset of [`Client`] the MCP server uses.
pub trait DaemonApi: Send + Sync + 'static {
    /// See [`Client::snapshot`].
    fn snapshot(&self) -> impl Future<Output = Result<Snapshot>> + Send;
    /// See [`Client::can_fit`].
    fn can_fit(&self, request: &CanFitRequest) -> impl Future<Output = Result<Verdict>> + Send;
    /// See [`Client::acquire`].
    fn acquire(
        &self,
        request: &AcquireRequest,
    ) -> impl Future<Output = Result<AcquireResponse>> + Send;
    /// See [`Client::wait`].
    fn wait(
        &self,
        id: LeaseId,
        session: &SessionKey,
        wait_timeout_secs: u64,
    ) -> impl Future<Output = Result<AcquireResponse>> + Send;
    /// See [`Client::heartbeat`].
    fn heartbeat(
        &self,
        id: LeaseId,
        session: &SessionKey,
    ) -> impl Future<Output = Result<HeartbeatResponse>> + Send;
    /// See [`Client::release`].
    fn release(
        &self,
        id: LeaseId,
        session: &SessionKey,
    ) -> impl Future<Output = Result<ReleaseResponse>> + Send;
    /// See [`Client::leases`].
    fn leases(&self, query: &LeaseQuery) -> impl Future<Output = Result<Vec<Lease>>> + Send;
    /// See [`Client::register_session`].
    fn register_session(&self, holder: &Holder) -> impl Future<Output = Result<()>> + Send;
    /// See [`Client::notices`].
    fn notices(&self, session: &SessionKey) -> impl Future<Output = Result<Vec<Notice>>> + Send;
    /// See [`Client::mark_delivered`].
    fn mark_delivered(
        &self,
        id: LeaseId,
        session: &SessionKey,
    ) -> impl Future<Output = Result<bool>> + Send;
}

/// Registers `holder` with the daemon. Failing only costs per-session usage in snapshots,
/// so it is logged rather than fatal.
pub async fn register<C: DaemonApi>(client: &C, holder: &Holder) {
    if let Err(error) = client.register_session(holder).await {
        eprintln!("rh-mcp: registering the session failed: {error}");
    }
}

impl DaemonApi for Client {
    fn snapshot(&self) -> impl Future<Output = Result<Snapshot>> + Send {
        Client::snapshot(self)
    }

    fn can_fit(&self, request: &CanFitRequest) -> impl Future<Output = Result<Verdict>> + Send {
        Client::can_fit(self, request)
    }

    fn acquire(
        &self,
        request: &AcquireRequest,
    ) -> impl Future<Output = Result<AcquireResponse>> + Send {
        Client::acquire(self, request)
    }

    fn wait(
        &self,
        id: LeaseId,
        session: &SessionKey,
        wait_timeout_secs: u64,
    ) -> impl Future<Output = Result<AcquireResponse>> + Send {
        Client::wait(self, id, session, wait_timeout_secs)
    }

    fn heartbeat(
        &self,
        id: LeaseId,
        session: &SessionKey,
    ) -> impl Future<Output = Result<HeartbeatResponse>> + Send {
        Client::heartbeat(self, id, session)
    }

    fn release(
        &self,
        id: LeaseId,
        session: &SessionKey,
    ) -> impl Future<Output = Result<ReleaseResponse>> + Send {
        Client::release(self, id, session)
    }

    fn leases(&self, query: &LeaseQuery) -> impl Future<Output = Result<Vec<Lease>>> + Send {
        Client::leases(self, query)
    }

    fn register_session(&self, holder: &Holder) -> impl Future<Output = Result<()>> + Send {
        Client::register_session(self, holder)
    }

    fn notices(&self, session: &SessionKey) -> impl Future<Output = Result<Vec<Notice>>> + Send {
        Client::notices(self, session)
    }

    fn mark_delivered(
        &self,
        id: LeaseId,
        session: &SessionKey,
    ) -> impl Future<Output = Result<bool>> + Send {
        Client::mark_delivered(self, id, session)
    }
}
