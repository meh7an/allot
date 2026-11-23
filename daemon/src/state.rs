//! State shared by the HTTP handlers and the sampling task.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex, PoisonError, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use allot_core::{
    classifier::Classifier,
    history::Lowering,
    ledger::{Ledger, LedgerSettings, ProcessLiveness, Transition},
    process::ProcessTable,
};
use allot_shared::{
    config::Config,
    types::{Holder, JobProfile, Lease, LeaseId, SessionKey, Snapshot},
};
use diesel::{Connection, sqlite::SqliteConnection};
use tokio::sync::{Mutex as AsyncMutex, watch};

use crate::{
    db::{DbPool, repo},
    scopes::Scopes,
    updates::{UpdateOptions, Updates},
};

/// Handle the handlers receive through Axum's `State`.
pub type AppState = Arc<Shared>;

/// The most recent sample.
#[derive(Debug)]
pub struct Latest {
    /// What clients see.
    pub snapshot: Snapshot,
    /// Every running process, for liveness checks and command binding.
    pub processes: ProcessTable,
    /// Total CPU loads of the recent samples, oldest first, for the flyout's sparkline.
    pub cpu_history_pct: Vec<f32>,
}

/// The configuration in effect and what is built from it; replaced as a whole on reload.
#[derive(Debug)]
pub struct Settings {
    /// The loaded configuration.
    pub config: Config,
    /// The config's heavy-command rules, which fingerprint jobs for history.
    pub classifier: Classifier,
}

impl Settings {
    fn new(config: Config) -> Self {
        let classifier = Classifier::new(&config.classifier);
        Self { config, classifier }
    }
}

/// What a lease's request asked for before history changed it, which the release needs to say
/// what the next run of the same request would reserve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Requested {
    /// The resolved class, preset or custom profile.
    pub declared: JobProfile,
    /// How far history may lower it.
    pub lowering: Option<Lowering>,
}

/// The shortest time between two reads of the token file after wrong tokens.
const TOKEN_REREAD: Duration = Duration::from_secs(1);

/// Everything the daemon shares between tasks.
#[derive(Debug)]
pub struct Shared {
    /// The bearer token every authenticated request must carry, see [`Shared::token`].
    token: RwLock<String>,
    token_file: Option<PathBuf>,
    /// When the token file was last read again, which happens at most every
    /// [`TOKEN_REREAD`].
    token_read_at: Mutex<Option<Instant>>,
    /// Database connections.
    pub pool: DbPool,
    /// The ledger. It stays locked while its transitions are written, so the database sees
    /// them in the order the ledger made them.
    pub ledger: AsyncMutex<Ledger>,
    /// Bumped after every batch of lease transitions and every new lease, to wake waiting
    /// acquire requests and tell the panels on D-Bus.
    pub changes: watch::Sender<u64>,
    /// Set by `POST /shutdown`; the server stops gracefully when it turns true.
    pub shutdown: watch::Sender<bool>,
    /// When the daemon started.
    pub started: Instant,
    /// The cgroup scopes of Linux leases.
    pub scopes: Scopes,
    /// The config file the settings routes read and write, or `None` when the daemon keeps
    /// the config it started with.
    pub config_path: Option<PathBuf>,
    /// The port the daemon listens on until it restarts.
    pub listening_port: u16,
    /// Where `POST /shutdown` writes the stop record, or `None` when it cannot.
    pub stop_file: Option<PathBuf>,
    /// Held while the settings routes write the config file, so two writes from this daemon
    /// never race between the revision check and the rename.
    pub config_writes: AsyncMutex<()>,
    /// What allotd knows about newer releases and the update it started.
    pub updates: Updates,
    settings: RwLock<Arc<Settings>>,
    config_error: Mutex<Option<String>>,
    latest: RwLock<Option<Arc<Latest>>>,
    sessions: Mutex<HashMap<SessionKey, Holder>>,
    /// The request of each lease acquired since the daemon started, until its release. Kept
    /// in memory only: a lease restored across a restart has none.
    requests: Mutex<HashMap<LeaseId, Requested>>,
    /// Whether this run already recorded a first session, which spares a write per acquire.
    first_session_noted: AtomicBool,
}

/// Where the daemon's configuration comes from and how it is served.
#[derive(Debug, Clone)]
pub struct Service {
    /// The config file, or `None` to keep the config the daemon started with.
    pub config_path: Option<PathBuf>,
    /// The port the listener is bound to.
    pub listening_port: u16,
    /// The stop record's file.
    pub stop_file: Option<PathBuf>,
    /// The token's file, read again when a request presents another token.
    pub token_file: Option<PathBuf>,
    /// How allotd looks for newer releases and starts the update.
    pub updates: UpdateOptions,
}

impl Shared {
    /// Fresh state with an empty ledger and no sample yet.
    pub fn new(
        config: Config,
        token: String,
        pool: DbPool,
        scopes: Scopes,
        service: Service,
    ) -> Self {
        let ledger = Ledger::new(LedgerSettings::from_config(&config));
        Self {
            token: RwLock::new(token),
            token_file: service.token_file,
            token_read_at: Mutex::new(None),
            pool,
            ledger: AsyncMutex::new(ledger),
            changes: watch::Sender::new(0),
            shutdown: watch::Sender::new(false),
            started: Instant::now(),
            scopes,
            config_path: service.config_path,
            listening_port: service.listening_port,
            stop_file: service.stop_file,
            config_writes: AsyncMutex::new(()),
            updates: Updates::new(service.updates),
            settings: RwLock::new(Arc::new(Settings::new(config))),
            config_error: Mutex::new(None),
            latest: RwLock::new(None),
            sessions: Mutex::new(HashMap::new()),
            requests: Mutex::new(HashMap::new()),
            first_session_noted: AtomicBool::new(false),
        }
    }

    /// The bearer token every authenticated request must carry.
    pub fn token(&self) -> String {
        self.token
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Reads the token file again after a request presented another token, and returns
    /// whether the token changed. The file stays the truth: a token file replaced while
    /// allotd runs, as by reinstalling, holds the token from then on, so panels that read the
    /// new file get in and `POST /shutdown` still works. At most once a second, so wrong
    /// tokens cannot keep allotd reading the disk.
    pub fn reread_token(&self) -> bool {
        let Some(path) = &self.token_file else {
            return false;
        };
        {
            let mut read_at = self
                .token_read_at
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if read_at.is_some_and(|at| at.elapsed() < TOKEN_REREAD) {
                return false;
            }
            *read_at = Some(Instant::now());
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return false;
        };
        let fresh = text.trim();
        let mut token = self.token.write().unwrap_or_else(PoisonError::into_inner);
        if fresh.is_empty() || *token == fresh {
            return false;
        }
        tracing::info!("the token file changed; allotd takes the new token");
        fresh.clone_into(&mut token);
        true
    }

    /// The configuration in effect. Hold the returned value for one request, so it sees one
    /// consistent config even when a reload lands meanwhile.
    pub fn settings(&self) -> Arc<Settings> {
        self.settings
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Puts `config`, read from `source`, into effect when it differs from the config in
    /// effect, and clears a reported config error either way. A changed `daemon.port` is only
    /// logged, since the listener stays bound until a restart.
    pub async fn reload_config(&self, config: Config, source: &str) {
        let previous = self.settings();
        if config == previous.config {
            self.set_config_error(None);
            return;
        }
        let (old_port, new_port) = (previous.config.daemon.port, config.daemon.port);
        if new_port != old_port {
            tracing::warn!(
                "daemon.port changed from {old_port} to {new_port}; allotd keeps listening on \
                 {} until it restarts, while clients already use {new_port}",
                self.listening_port
            );
        }
        self.apply_config(config).await;
        tracing::info!("config reloaded from {source}");
    }

    /// Puts `config` into effect for later requests and the ledger, and clears a reported
    /// config error. Leases keep the profiles they reserved.
    async fn apply_config(&self, config: Config) {
        let mut ledger = self.ledger.lock().await;
        ledger.set_settings(LedgerSettings::from_config(&config));
        *self
            .settings
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Arc::new(Settings::new(config));
        drop(ledger);
        self.set_config_error(None);
    }

    /// Why the last config change was rejected, if it was.
    pub fn config_error(&self) -> Option<String> {
        self.config_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Records or clears the reason the config file was rejected.
    pub fn set_config_error(&self, error: Option<String>) {
        *self
            .config_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = error;
    }

    /// The most recent sample, if one has arrived.
    pub fn latest(&self) -> Option<Arc<Latest>> {
        self.latest
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Replaces the most recent sample.
    pub fn set_latest(&self, latest: Latest) {
        *self.latest.write().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(latest));
    }

    /// The heartbeat TTL in effect.
    pub fn heartbeat_ttl(&self) -> Duration {
        Duration::from_secs(self.settings().config.leases.heartbeat_ttl_secs)
    }

    /// Registers a session that contacted the daemon, and records when the first session of
    /// this install did, which ends the tray's first run. A failed write is logged and tried
    /// again on the next contact.
    pub async fn connect(&self, holder: Holder) {
        self.register(holder);
        if self.first_session_noted.swap(true, Ordering::Relaxed) {
            return;
        }
        let noted = self
            .with_db(|conn| repo::note_first_session(conn, now_ms()))
            .await;
        if let Err(error) = noted {
            self.first_session_noted.store(false, Ordering::Relaxed);
            tracing::error!("recording the first session failed: {error:#}");
        }
    }

    /// Adds or refreshes a session.
    pub fn register(&self, holder: Holder) {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(holder.session.clone(), holder);
    }

    /// The PIDs of every registered session's holder.
    pub fn holder_pids(&self) -> HashSet<u32> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .map(|holder| holder.pid)
            .collect()
    }

    /// Forgets sessions whose holder is gone and returns the rest.
    pub fn live_sessions(&self, processes: &ProcessTable) -> Vec<Holder> {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        sessions.retain(|_, holder| processes.is_alive(holder));
        sessions.values().cloned().collect()
    }

    /// Keeps the request of lease `id`, and forgets those of leases no longer in `tracked`,
    /// which ended without a release.
    pub fn remember_request(&self, id: LeaseId, requested: Requested, tracked: &[Lease]) {
        let mut requests = self.requests.lock().unwrap_or_else(PoisonError::into_inner);
        requests.retain(|kept, _| tracked.iter().any(|lease| lease.id == *kept));
        requests.insert(id, requested);
    }

    /// Takes the request of lease `id`, if this daemon run still has it.
    pub fn take_request(&self, id: LeaseId) -> Option<Requested> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id)
    }

    /// Wakes every request waiting for a lease transition.
    pub fn notify_changes(&self) {
        self.changes.send_modify(|generation| *generation += 1);
    }

    /// Runs blocking database work off the async runtime.
    ///
    /// # Errors
    ///
    /// Fails when no connection is available, `work` fails, or the blocking task panics.
    pub async fn with_db<T, F>(&self, work: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut SqliteConnection) -> anyhow::Result<T> + Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            work(&mut conn)
        })
        .await?
    }

    /// Writes `transitions` to the database, all stamped `at_ms`.
    ///
    /// # Errors
    ///
    /// Fails when a write fails; earlier transitions in the batch stay written.
    pub async fn persist(&self, transitions: &[Transition], at_ms: i64) -> anyhow::Result<()> {
        if transitions.is_empty() {
            return Ok(());
        }
        for transition in transitions {
            tracing::info!(
                lease = transition.id.0,
                to = transition.to.as_str(),
                reason = %transition.reason,
                "lease transition"
            );
        }
        let transitions = transitions.to_vec();
        self.with_db(move |conn| {
            for transition in &transitions {
                conn.transaction(|conn| {
                    if let Some(cores) = transition.granted_cores {
                        repo::shrink_lease(conn, transition.id, cores)?;
                    }
                    repo::transition_lease(
                        conn,
                        transition.id,
                        transition.to,
                        at_ms,
                        &transition.reason,
                    )
                })?;
            }
            Ok(())
        })
        .await
    }

    /// Runs one ledger tick against the latest sample and persists what changed. Does
    /// nothing before the first sample.
    ///
    /// # Errors
    ///
    /// Fails when the transitions cannot be written.
    pub async fn tick(&self, ledger: &mut Ledger) -> anyhow::Result<()> {
        let Some(latest) = self.latest() else {
            return Ok(());
        };
        let now = now_ms();
        let transitions = ledger.tick(now, &latest.snapshot, &latest.processes);
        self.persist(&transitions, now).await?;
        if !transitions.is_empty() {
            self.notify_changes();
        }
        Ok(())
    }
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
