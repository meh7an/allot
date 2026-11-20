//! The in-process allotd the API tests drive: the real router and ledger over a temp
//! database and config folder, with a fake sampler whose machine state each test controls.
//! Nothing here touches the user's data folder or a running allotd.

// Each test binary uses its own share of the harness.
#![allow(dead_code)]

use std::{
    fs,
    future::Future,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use allot_core::sampler::{RawProcess, RawSample, Sampler};
use allot_shared::{
    api::HealthResponse,
    config::Config,
    types::{Holder, MemStats, ResourceUsage, SessionKey},
};
use allotd::{DaemonOptions, Maintenance, Scopes, db::open_pool};
use diesel::{Connection, sqlite::SqliteConnection};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde_json::Value;
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

pub const TOKEN: &str = "test-token";
pub const ALICE_PID: u32 = 4_242;
pub const BOB_PID: u32 = 5_353;

pub struct FakeSampler(Arc<Mutex<RawSample>>);

impl Sampler for FakeSampler {
    /// The current machine state, stamped now: the quiet window and the benchmark meter
    /// count each sample once, by its timestamp.
    fn sample(&mut self) -> allot_shared::Result<RawSample> {
        let mut sample = self.0.lock().unwrap().clone();
        sample.taken_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        Ok(sample)
    }
}

pub fn process(pid: u32, start_time: u64) -> RawProcess {
    RawProcess {
        pid,
        parent_pid: None,
        name: "claude.exe".into(),
        exe: None,
        start_time,
        usage: ResourceUsage::default(),
    }
}

/// A 16-core, 32 GB machine at rest with two Claude sessions running.
pub fn idle_machine() -> RawSample {
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
            reclaimable_mb: 0,
        },
        processes: vec![process(ALICE_PID, 100), process(BOB_PID, 200)],
        gpus: vec![],
        drives: vec![],
    }
}

pub fn holder(session: &str, pid: u32, pid_start_time: u64) -> Holder {
    Holder {
        session: SessionKey(session.into()),
        pid,
        pid_start_time,
    }
}

pub fn alice() -> Holder {
    holder("alice", ALICE_PID, 100)
}

pub fn bob() -> Holder {
    holder("bob", BOB_PID, 200)
}

pub async fn send(request: RequestBuilder) -> Response {
    request.send().await.unwrap()
}

/// Polls `check` every 10 ms for up to 3 s.
pub async fn eventually<F, Fut>(what: &str, mut check: F)
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

pub struct Daemon {
    pub base: String,
    pub client: reqwest::Client,
    pub machine: Arc<Mutex<RawSample>>,
    pub stop: Option<oneshot::Sender<()>>,
    pub server: Option<JoinHandle<anyhow::Result<()>>>,
    pub dir: TempDir,
}

impl Daemon {
    pub async fn start() -> Self {
        Self::start_with(Config::default(), |_| {}).await
    }

    /// Starts a daemon on `config` after `prepare` has run against its fresh database.
    pub async fn start_with(config: Config, prepare: impl FnOnce(&mut SqliteConnection)) -> Self {
        let dir = TempDir::new().unwrap();
        prepare(&mut open_pool(&database(&dir)).unwrap().get().unwrap());
        Self::serve_in(dir, config, Arc::new(Mutex::new(idle_machine()))).await
    }

    /// Stops the daemon gracefully and starts a new one on the same database, config file,
    /// and machine.
    pub async fn restart(mut self) -> Self {
        self.stop_server().await;
        let Self { machine, dir, .. } = self;
        Self::serve_in(dir, Config::default(), machine).await
    }

    /// Starts a daemon that checks its config file only once an hour, so whatever changes
    /// the config in effect in the meantime is not the poll.
    pub async fn start_unpolled() -> Self {
        let machine = Arc::new(Mutex::new(idle_machine()));
        let dir = TempDir::new().unwrap();
        Self::serve(dir, Config::default(), machine, Duration::from_secs(3_600)).await
    }

    pub async fn serve_in(dir: TempDir, config: Config, machine: Arc<Mutex<RawSample>>) -> Self {
        Self::serve(dir, config, machine, Duration::from_millis(20)).await
    }

    async fn serve(
        dir: TempDir,
        config: Config,
        machine: Arc<Mutex<RawSample>>,
        config_poll: Duration,
    ) -> Self {
        let pool = open_pool(&database(&dir)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let sampled = machine.clone();
        let options = DaemonOptions {
            config,
            token: TOKEN.into(),
            pool,
            sampler: Box::new(move || Ok(Box::new(FakeSampler(sampled)) as Box<dyn Sampler>)),
            sample_interval: Duration::from_millis(20),
            maintenance: Maintenance {
                config_path: Some(dir.path().join("config.toml")),
                config_poll,
                prune_every: Duration::from_secs(3_600),
            },
            scopes: Scopes::disabled(),
        };
        let (stop, stopped) = oneshot::channel::<()>();
        let server = tokio::spawn(allotd::serve(listener, options, async {
            let _ = stopped.await;
        }));
        let daemon = Self {
            base,
            client: reqwest::Client::new(),
            machine,
            stop: Some(stop),
            server: Some(server),
            dir,
        };
        eventually("the first snapshot", || async {
            daemon.get("/snapshot").await.status() == StatusCode::OK
        })
        .await;
        daemon
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// The port the daemon listens on, picked by the system.
    pub fn port(&self) -> u16 {
        self.base.rsplit(':').next().unwrap().parse().unwrap()
    }

    pub fn patch(&self, path: &str) -> RequestBuilder {
        self.client.patch(self.url(path)).bearer_auth(TOKEN)
    }

    /// The daemon's config file.
    pub fn config_file(&self) -> PathBuf {
        self.dir.path().join("config.toml")
    }

    pub async fn get(&self, path: &str) -> Response {
        let request = self.client.get(self.url(path)).bearer_auth(TOKEN);
        request.send().await.unwrap()
    }

    /// Waits until `count` samples taken after `after_ms` have reached the daemon, so a test
    /// does not depend on how quickly a loaded CI runner schedules the sampling loop.
    pub async fn wait_for_samples_after(&self, after_ms: i64, count: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut seen = Vec::new();
        while seen.len() < count {
            let snapshot: Value = self.get("/snapshot").await.json().await.unwrap();
            let taken = snapshot["taken_at_ms"].as_i64().unwrap();
            if taken > after_ms && !seen.contains(&taken) {
                seen.push(taken);
            }
            assert!(
                std::time::Instant::now() < deadline,
                "only {} sample(s) after {after_ms} within 10 s",
                seen.len()
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    pub fn post(&self, path: &str) -> RequestBuilder {
        self.client.post(self.url(path)).bearer_auth(TOKEN)
    }

    pub async fn delete(&self, path: &str) -> Response {
        let request = self.client.delete(self.url(path)).bearer_auth(TOKEN);
        request.send().await.unwrap()
    }
    /// A connection to the daemon's database, for checks the API does not expose.
    pub fn db(&self) -> SqliteConnection {
        SqliteConnection::establish(&database(&self.dir).to_string_lossy()).unwrap()
    }

    pub fn write_config(&self, text: &str) {
        fs::write(self.dir.path().join("config.toml"), text).unwrap();
    }

    pub async fn health(&self) -> HealthResponse {
        send(self.client.get(self.url("/health")))
            .await
            .json()
            .await
            .unwrap()
    }

    pub async fn stop(mut self) {
        self.stop_server().await;
    }

    pub async fn stop_server(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.send(()).unwrap();
        }
        if let Some(server) = self.server.take() {
            server.await.unwrap().unwrap();
        }
    }
}

pub fn database(dir: &TempDir) -> PathBuf {
    dir.path().join("allot.db")
}
