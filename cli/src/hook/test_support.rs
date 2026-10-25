//! A fake daemon and sample leases and notices for the hook tests.

use std::cell::RefCell;

use allot_client::ClientError;
use allot_shared::{
    api::BindRequest,
    types::{EndCause, Holder, JobProfile, Lease, LeaseId, LeaseKind, LeaseState, SessionKey},
    ui::Notice,
};

use super::HookDaemon;

pub const FIXTURE_SESSION: &str = "00000000-0000-4000-8000-000000000001";

/// Answers from fixed leases and notices and records what the hooks sent.
#[derive(Debug, Default)]
pub struct FakeDaemon {
    pub leases: Vec<Lease>,
    pub notices: Vec<Notice>,
    /// Notices another channel marks delivered between the list and the mark.
    pub taken_elsewhere: Vec<LeaseId>,
    /// Every call fails as if allotd were not running.
    pub down: bool,
    pub fail_bind: bool,
    pub(in crate::hook) asked: RefCell<Vec<String>>,
    pub(in crate::hook) binds: RefCell<Vec<(LeaseId, BindRequest)>>,
    pub(in crate::hook) delivered: RefCell<Vec<LeaseId>>,
}

impl FakeDaemon {
    /// The sessions whose leases were asked for.
    pub fn asked(&self) -> Vec<String> {
        self.asked.borrow().clone()
    }

    pub fn binds(&self) -> Vec<(LeaseId, BindRequest)> {
        self.binds.borrow().clone()
    }

    /// The notices marked delivered, in order.
    pub fn delivered(&self) -> Vec<LeaseId> {
        self.delivered.borrow().clone()
    }

    fn up(&self) -> allot_client::Result<()> {
        if self.down { Err(down()) } else { Ok(()) }
    }
}

impl HookDaemon for FakeDaemon {
    fn leases(&self, session: &SessionKey) -> allot_client::Result<Vec<Lease>> {
        self.up()?;
        self.asked.borrow_mut().push(session.0.clone());
        Ok(self.leases.clone())
    }

    fn bind(
        &self,
        id: LeaseId,
        _session: &SessionKey,
        request: &BindRequest,
    ) -> allot_client::Result<()> {
        self.up()?;
        if self.fail_bind {
            return Err(ClientError::Http {
                status: 409,
                message: "lease 1 is not active".into(),
            });
        }
        self.binds.borrow_mut().push((id, request.clone()));
        Ok(())
    }

    fn notices(&self, _session: &SessionKey) -> allot_client::Result<Vec<Notice>> {
        self.up()?;
        let delivered = self.delivered.borrow();
        Ok(self
            .notices
            .iter()
            .filter(|notice| !delivered.contains(&notice.lease_id))
            .cloned()
            .collect())
    }

    fn mark_delivered(&self, id: LeaseId, _session: &SessionKey) -> allot_client::Result<bool> {
        self.up()?;
        if self.taken_elsewhere.contains(&id) || self.delivered.borrow().contains(&id) {
            return Ok(false);
        }
        self.delivered.borrow_mut().push(id);
        Ok(true)
    }
}

impl HookDaemon for &FakeDaemon {
    fn leases(&self, session: &SessionKey) -> allot_client::Result<Vec<Lease>> {
        (*self).leases(session)
    }

    fn bind(
        &self,
        id: LeaseId,
        session: &SessionKey,
        request: &BindRequest,
    ) -> allot_client::Result<()> {
        (*self).bind(id, session, request)
    }

    fn notices(&self, session: &SessionKey) -> allot_client::Result<Vec<Notice>> {
        (*self).notices(session)
    }

    fn mark_delivered(&self, id: LeaseId, session: &SessionKey) -> allot_client::Result<bool> {
        (*self).mark_delivered(id, session)
    }
}

fn down() -> ClientError {
    ClientError::Unreachable {
        url: "http://127.0.0.1:47823".into(),
        reason: "connection refused".into(),
    }
}

/// A connect that fails as if allotd were not running.
pub fn unreachable() -> allot_client::Result<FakeDaemon> {
    Err(down())
}

/// A connect that must not happen.
pub fn never() -> allot_client::Result<FakeDaemon> {
    panic!("this payload must not reach the daemon");
}

pub fn lease(id: i64, state: LeaseState, profile: JobProfile) -> Lease {
    Lease {
        id: LeaseId(id),
        holder: Holder {
            session: SessionKey(FIXTURE_SESSION.into()),
            pid: 4_242,
            pid_start_time: 1,
        },
        kind: LeaseKind::Normal,
        state,
        class: None,
        profile,
        label: format!("job {id}"),
        cwd: String::new(),
        requested_at_ms: 0,
        granted_at_ms: None,
        ended_at_ms: None,
        expires_at_ms: 60_000,
        end_cause: None,
        binding: None,
    }
}

pub fn cpu() -> JobProfile {
    JobProfile {
        cores: 8.0,
        ram_mb: 4_096,
        gpu_pct: 0.0,
        vram_mb: 0,
        est_secs: 300,
    }
}

pub fn gpu() -> JobProfile {
    JobProfile {
        cores: 0.0,
        ram_mb: 2_048,
        gpu_pct: 0.0,
        vram_mb: 6_144,
        est_secs: 600,
    }
}

/// A notice for `cargo build` in the fixture session.
pub fn notice(id: i64, cause: EndCause, tool_use_id: Option<&str>) -> Notice {
    Notice {
        lease_id: LeaseId(id),
        session: SessionKey(FIXTURE_SESSION.into()),
        cause,
        command: "cargo build".into(),
        tool_use_id: tool_use_id.map(str::to_owned),
        ended_at_ms: 1_793_000_090_000,
        delivered_at_ms: None,
    }
}
