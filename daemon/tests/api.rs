//! End-to-end tests: the real router and ledger over a temp database, with a fake sampler
//! whose machine state each test controls.

use std::{
    future::Future,
    process,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use diesel::sqlite::SqliteConnection;
use reqwest::{RequestBuilder, Response, StatusCode};
use rh_core::sampler::{RawProcess, RawSample, Sampler};
use rh_shared::{
    api::{
        AcquireRequest, AcquireResponse, CanFitRequest, HealthResponse, HeartbeatResponse,
        LeasesResponse, ProfileSpec, RegisterSessionRequest, ReleaseResponse, SessionsResponse,
        Verdict,
    },
    config::Config,
    types::{
        Holder, JobClass, JobProfile, Lease, LeaseId, LeaseKind, LeaseState, MemStats,
        ResourceUsage, SessionKey, Snapshot,
    },
};
use rhd::{
    DaemonOptions,
    db::{
        open_pool,
        repo::{self, LeaseDraft},
    },
};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

const TOKEN: &str = "test-token";
const ALICE_PID: u32 = 4_242;
const BOB_PID: u32 = 5_353;

struct FakeSampler(Arc<Mutex<RawSample>>);

impl Sampler for FakeSampler {
    fn sample(&mut self) -> rh_shared::Result<RawSample> {
        Ok(self.0.lock().unwrap().clone())
    }
}

fn process(pid: u32, start_time: u64) -> RawProcess {
    RawProcess {
        pid,
        parent_pid: None,
        name: "claude.exe".into(),
        start_time,
        usage: ResourceUsage::default(),
    }
}

/// A 16-core, 32 GB machine at rest with two Claude sessions running.
fn idle_machine() -> RawSample {
    RawSample {
        taken_at_ms: 0,
        cpu_total_pct: 0.0,
        per_core_pct: vec![0.0; 16],
        memory: MemStats {
            total_mb: 32_768,
            used_mb: 4_000,
            available_mb: 28_768,
            commit_used_mb: 6_000,
            commit_limit_mb: 65_536,
        },
        processes: vec![process(ALICE_PID, 100), process(BOB_PID, 200)],
        gpus: vec![],
    }
}

fn holder(session: &str, pid: u32, pid_start_time: u64) -> Holder {
    Holder {
        session: SessionKey(session.into()),
        pid,
        pid_start_time,
    }
}

fn alice() -> Holder {
    holder("alice", ALICE_PID, 100)
}

fn bob() -> Holder {
    holder("bob", BOB_PID, 200)
}

async fn send(request: RequestBuilder) -> Response {
    request.send().await.unwrap()
}

/// Polls `check` every 10 ms for up to 3 s.
async fn eventually<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

struct Daemon {
    base: String,
    client: reqwest::Client,
    machine: Arc<Mutex<RawSample>>,
    stop: Option<oneshot::Sender<()>>,
    server: Option<JoinHandle<anyhow::Result<()>>>,
    _dir: TempDir,
}

impl Daemon {
    async fn start() -> Self {
        Self::start_with(|_| {}).await
    }

    /// Starts a daemon after `prepare` has run against its fresh database.
    async fn start_with(prepare: impl FnOnce(&mut SqliteConnection)) -> Self {
        let dir = TempDir::new().unwrap();
        let pool = open_pool(&dir.path().join("rh.db")).unwrap();
        prepare(&mut pool.get().unwrap());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let machine = Arc::new(Mutex::new(idle_machine()));
        let sampled = machine.clone();
        let options = DaemonOptions {
            config: Config::default(),
            token: TOKEN.into(),
            pool,
            sampler: Box::new(move || Ok(Box::new(FakeSampler(sampled)) as Box<dyn Sampler>)),
            sample_interval: Duration::from_millis(20),
        };
        let (stop, stopped) = oneshot::channel::<()>();
        let server = tokio::spawn(rhd::serve(listener, options, async {
            let _ = stopped.await;
        }));
        let daemon = Self {
            base,
            client: reqwest::Client::new(),
            machine,
            stop: Some(stop),
            server: Some(server),
            _dir: dir,
        };
        eventually("the first snapshot", || async {
            daemon.get("/snapshot").await.status() == StatusCode::OK
        })
        .await;
        daemon
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    async fn get(&self, path: &str) -> Response {
        let request = self.client.get(self.url(path)).bearer_auth(TOKEN);
        request.send().await.unwrap()
    }

    fn post(&self, path: &str) -> RequestBuilder {
        self.client.post(self.url(path)).bearer_auth(TOKEN)
    }

    async fn delete(&self, path: &str) -> Response {
        let request = self.client.delete(self.url(path)).bearer_auth(TOKEN);
        request.send().await.unwrap()
    }

    async fn acquire(&self, holder: Holder, profile: ProfileSpec, wait_secs: u64) -> Response {
        let request = AcquireRequest {
            holder,
            profile,
            kind: LeaseKind::Normal,
            label: "cargo build".into(),
            wait_timeout_secs: wait_secs,
        };
        send(self.post("/leases").json(&request)).await
    }

    async fn acquire_build(&self, holder: Holder, wait_secs: u64) -> AcquireResponse {
        let profile = ProfileSpec::Class(JobClass::CpuHeavy);
        let response = self.acquire(holder, profile, wait_secs).await;
        assert_eq!(response.status(), StatusCode::OK);
        response.json().await.unwrap()
    }

    async fn heartbeat(&self, id: LeaseId, session: &str) -> Response {
        let path = format!("/leases/{}/heartbeat?session={session}", id.0);
        send(self.post(&path)).await
    }

    async fn heartbeat_state(&self, id: LeaseId, session: &str) -> LeaseState {
        let response = self.heartbeat(id, session).await;
        assert_eq!(response.status(), StatusCode::OK);
        response.json::<HeartbeatResponse>().await.unwrap().state
    }

    async fn release(&self, id: LeaseId, session: &str) -> Response {
        let path = format!("/leases/{}?session={session}", id.0);
        self.delete(&path).await
    }

    async fn leases(&self, query: &str) -> Vec<Lease> {
        let response = self.get(&format!("/leases{query}")).await;
        response.json::<LeasesResponse>().await.unwrap().leases
    }

    async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            stop.send(()).unwrap();
        }
        if let Some(server) = self.server.take() {
            server.await.unwrap().unwrap();
        }
    }
}

#[tokio::test]
async fn test_health_is_open_and_every_other_route_needs_the_token() {
    let daemon = Daemon::start().await;
    let health = daemon
        .client
        .get(daemon.url("/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    let health: HealthResponse = health.json().await.unwrap();
    assert_eq!(health.pid, process::id());

    let anonymous = daemon
        .client
        .get(daemon.url("/snapshot"))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let wrong = daemon.client.get(daemon.url("/leases")).bearer_auth("nope");
    assert_eq!(
        wrong.send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    daemon.stop().await;
}

#[tokio::test]
async fn test_snapshot_lists_registered_live_sessions_only() {
    let daemon = Daemon::start().await;
    let snapshot: Snapshot = daemon.get("/snapshot").await.json().await.unwrap();
    assert_eq!(snapshot.cpu.logical_cores, 16);

    for holder in [alice(), holder("ghost", 9_999, 1)] {
        let request = RegisterSessionRequest { holder };
        let response = send(daemon.post("/sessions").json(&request)).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
    eventually("alice in /sessions", || async {
        let sessions: SessionsResponse = daemon.get("/sessions").await.json().await.unwrap();
        let keys: Vec<_> = sessions
            .sessions
            .iter()
            .map(|s| s.holder.session.0.clone())
            .collect();
        keys == ["alice"]
    })
    .await;
    daemon.stop().await;
}

#[tokio::test]
async fn test_acquire_on_idle_machine_is_granted_at_once() {
    let daemon = Daemon::start().await;
    let acquired = daemon.acquire_build(alice(), 0).await;
    assert_eq!(acquired.lease.state, LeaseState::Active);
    assert!(acquired.verdict.fits, "{:?}", acquired.verdict);
    assert_eq!(acquired.lease.class, Some(JobClass::CpuHeavy));

    let active = daemon.leases("?state=active").await;
    assert_eq!(
        active.iter().map(|lease| lease.id).collect::<Vec<_>>(),
        [acquired.lease.id]
    );
    daemon.stop().await;
}

#[tokio::test]
async fn test_second_build_queues_and_starts_after_release() {
    let daemon = Daemon::start().await;
    let first = daemon.acquire_build(alice(), 0).await.lease;
    let second = daemon.acquire_build(bob(), 0).await;
    assert_eq!(second.lease.state, LeaseState::Queued);
    assert!(!second.verdict.fits);
    assert!(
        second
            .verdict
            .reasons
            .iter()
            .any(|reason| reason.starts_with("cores:")),
        "{:?}",
        second.verdict
    );

    let released: ReleaseResponse = daemon
        .release(first.id, "alice")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(released.lease.state, LeaseState::Released);
    assert_eq!(
        daemon.heartbeat_state(second.lease.id, "bob").await,
        LeaseState::Active
    );
    daemon.stop().await;
}

#[tokio::test]
async fn test_acquire_wait_returns_as_soon_as_the_lease_starts() {
    let daemon = Arc::new(Daemon::start().await);
    let first = daemon.acquire_build(alice(), 0).await.lease;

    let waiter = daemon.clone();
    let started = Instant::now();
    let second = tokio::spawn(async move { waiter.acquire_build(bob(), 30).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!second.is_finished());
    daemon.release(first.id, "alice").await;

    let second = second.await.unwrap();
    assert_eq!(second.lease.state, LeaseState::Active);
    assert!(started.elapsed() < Duration::from_secs(5));
    Arc::into_inner(daemon).unwrap().stop().await;
}

#[tokio::test]
async fn test_lease_of_a_dead_holder_expires() {
    let daemon = Daemon::start().await;
    let lease = daemon.acquire_build(bob(), 0).await.lease;
    assert_eq!(lease.state, LeaseState::Active);

    daemon
        .machine
        .lock()
        .unwrap()
        .processes
        .retain(|process| process.pid != BOB_PID);
    eventually("the lease to expire", || async {
        daemon.heartbeat_state(lease.id, "bob").await == LeaseState::Expired
    })
    .await;
    let expired = daemon.leases("?state=expired").await;
    assert_eq!(expired[0].id, lease.id);
    daemon.stop().await;
}

#[tokio::test]
async fn test_heartbeat_and_release_check_the_owner() {
    let daemon = Daemon::start().await;
    let lease = daemon.acquire_build(alice(), 0).await.lease;

    assert_eq!(
        daemon.heartbeat(lease.id, "bob").await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        daemon.release(lease.id, "bob").await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        daemon.heartbeat(LeaseId(999), "alice").await.status(),
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        daemon.release(lease.id, "alice").await.status(),
        StatusCode::OK
    );
    let again: ReleaseResponse = daemon
        .release(lease.id, "alice")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(again.lease.state, LeaseState::Released);
    daemon.stop().await;
}

#[tokio::test]
async fn test_invalid_profile_is_unprocessable() {
    let daemon = Daemon::start().await;
    let profile = ProfileSpec::Custom(JobProfile {
        cores: 1.0,
        ram_mb: 100,
        gpu_pct: 150.0,
        vram_mb: 0,
        est_secs: 10,
    });
    let response = daemon.acquire(alice(), profile, 0).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(response.text().await.unwrap().contains("gpu_pct"));
    daemon.stop().await;
}

#[tokio::test]
async fn test_can_fit_reports_the_shortfall() {
    let daemon = Daemon::start().await;
    let request = CanFitRequest {
        profile: ProfileSpec::Class(JobClass::CpuHeavy),
        kind: LeaseKind::Normal,
    };
    let can_fit = || send(daemon.post("/can-fit").json(&request));
    let verdict: Verdict = can_fit().await.json().await.unwrap();
    assert!(verdict.fits);

    daemon.acquire_build(alice(), 0).await;
    let verdict: Verdict = can_fit().await.json().await.unwrap();
    assert!(!verdict.fits);
    assert!(verdict.wait_estimate_secs.is_some());
    daemon.stop().await;
}

#[tokio::test]
async fn test_leases_left_from_a_previous_run_expire_on_startup() {
    let mut leftover = None;
    let daemon = Daemon::start_with(|conn| {
        let draft = LeaseDraft {
            holder: alice(),
            kind: LeaseKind::Normal,
            class: None,
            profile: JobProfile {
                cores: 1.0,
                ram_mb: 100,
                gpu_pct: 0.0,
                vram_mb: 0,
                est_secs: 10,
            },
            label: "old build".into(),
            requested_at_ms: 1,
            expires_at_ms: 2,
        };
        let lease = repo::insert_lease(conn, &draft).unwrap();
        repo::transition_lease(conn, lease.id, LeaseState::Active, 1, "granted").unwrap();
        leftover = Some(lease.id);
    })
    .await;

    let leases = daemon.leases("").await;
    assert_eq!(leases[0].id, leftover.unwrap());
    assert_eq!(leases[0].state, LeaseState::Expired);
    daemon.stop().await;
}
