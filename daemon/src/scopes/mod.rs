//! Lease scopes on Linux: each lease's command runs in a transient systemd scope of its own,
//! which gives the command's exact CPU time and memory and a kill that ends every process in
//! it at once.
//!
//! A scope is garbage-collected the moment its last process exits, so allotd starts an idle
//! anchor in it when a lease reports its first command. The anchor keeps the scope, and the
//! usage of processes that already exited, until the lease ends; then only the anchor ends,
//! and processes still running (a job started in the background) keep running. The scope is
//! `Delegate=yes`, so the user manager moves processes of the same user into it: allotd
//! watches the holder for the shell Claude Code starts for the command, then moves the
//! shell's tree until a pass finds nothing left outside. Children forked after a move start
//! in the scope.
//!
//! Without cgroup v2, a session bus, or a systemd user manager answering on it, leases follow
//! their process tree instead, as on macOS.

// Off Linux nothing builds a backend; the fakes still test it on every OS.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod bus;
#[cfg(test)]
mod fake;
mod follow;
mod host;
#[cfg(target_os = "linux")]
mod systemd;

use std::{
    collections::{HashMap, HashSet},
    io,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use allot_core::meter::ScopeUsage;
use allot_shared::{
    config::LinuxConfig,
    platform::{ALLOT_BINARIES, SESSION_ROOTS},
    types::{EnforcedLimits, IsolationKind, JobProfile, LeaseId, LeaseIsolation},
};

pub(crate) use self::follow::follow_command;
use self::{
    bus::{BusError, ScopeCaps, SystemdBus},
    host::{Host, ProcInfo, find_anchor},
};

/// The flag that makes allotd run as a scope's idle anchor, followed by the scope name.
pub const ANCHOR_FLAG: &str = "--anchor";

const BYTES_PER_MB: u64 = 1024 * 1024;

/// How often the watcher looks for the command's shell. The shell starts 25 to 190 ms after
/// the PreToolUse hook exits, and memory it touches before the move stays charged to its old
/// cgroup, so the move has to follow within milliseconds.
const WATCH_INTERVAL: Duration = Duration::from_millis(1);

/// How long a new scope's anchor may take to show up in it; about 30 ms is usual.
const START_TIMEOUT: Duration = Duration::from_secs(2);

/// Passes of the move loop before it gives up on a tree that keeps forking outside.
const MAX_PASSES: u32 = 20;

/// Why a lease got no scope, or its command did not fully move into it.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ScopeError {
    /// The user manager refused or the bus failed.
    #[error(transparent)]
    Bus(#[from] BusError),
    /// The anchor process did not start.
    #[error("starting the anchor failed: {0}")]
    Anchor(#[source] io::Error),
    /// The scope never showed its anchor.
    #[error("{0} did not start within {timeout:?}", timeout = START_TIMEOUT)]
    NotStarted(String),
    /// Scopes are off on this machine, or the lease has none.
    #[error("lease {} has no scope", .0.0)]
    NoScope(LeaseId),
    /// The tree kept starting processes outside the scope.
    #[error("processes still appeared outside {0} after {passes} passes", passes = MAX_PASSES)]
    Unsettled(String),
}

/// Lease scopes, or nothing where the machine cannot run them.
#[derive(Debug)]
pub struct Scopes {
    backend: Option<Backend>,
}

/// The command a lease reported, which the watcher looks for among the holder's children.
#[derive(Debug)]
pub(crate) struct Watch {
    id: LeaseId,
    holder: u32,
    generation: u64,
    /// Children that cannot be the command: they ran before it was reported, or are hooks.
    baseline: HashSet<u32>,
}

#[derive(Debug)]
struct Backend {
    bus: Arc<dyn SystemdBus>,
    host: Arc<dyn Host>,
    /// Names this install's scopes apart from those of another data dir's allotd.
    tag: String,
    tracked: Mutex<Tracked>,
    /// One scope starts at a time, so two reports for one lease cannot start two.
    starting: tokio::sync::Mutex<()>,
}

#[derive(Debug, Default)]
struct Tracked {
    scopes: HashMap<LeaseId, Scope>,
    /// The newest watch per lease; an older one stops looking.
    watches: HashMap<LeaseId, u64>,
    next_watch: u64,
}

#[derive(Debug)]
struct Scope {
    name: String,
    cgroup: String,
    limits: EnforcedLimits,
    /// The previous CPU reading; `None` for a scope found again after a restart.
    last: Option<Reading>,
}

#[derive(Debug, Clone, Copy)]
struct Reading {
    at: Instant,
    cpu_usage_usec: u64,
}

impl Scopes {
    /// No scopes: every lease follows its process tree.
    pub fn disabled() -> Self {
        Self { backend: None }
    }

    /// Scopes through the systemd user manager when this machine has one on cgroup v2, with
    /// names tagged by `data_dir`; otherwise [`Scopes::disabled`]. Logs which it is.
    pub async fn detect(data_dir: &Path) -> Self {
        #[cfg(target_os = "linux")]
        {
            let program = match std::env::current_exe() {
                Ok(program) => program,
                Err(error) => {
                    tracing::warn!("lease scopes off: allotd cannot find itself: {error}");
                    return Self::disabled();
                }
            };
            Self::connect(data_dir, program, vec![ANCHOR_FLAG.to_owned()]).await
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = data_dir;
            Self::disabled()
        }
    }

    /// Like [`Scopes::detect`], with anchors started as `program args.. <scope>`.
    #[cfg(target_os = "linux")]
    async fn connect(data_dir: &Path, program: std::path::PathBuf, args: Vec<String>) -> Self {
        if !allot_sampler::cgroup::is_cgroup2(Path::new("/")) {
            tracing::info!("lease scopes off: no cgroup v2 hierarchy; leases follow their tree");
            return Self::disabled();
        }
        let bus = match systemd::UserManager::connect().await {
            Ok(bus) => bus,
            Err(error) => {
                tracing::info!("lease scopes off: {error}; leases follow their tree");
                return Self::disabled();
            }
        };
        let tag = install_tag(data_dir);
        // Doubles as the check that a systemd user manager answers on the bus.
        if let Err(error) = bus.list_units(&scope_pattern(&tag)).await {
            tracing::info!("lease scopes off: {error}; leases follow their tree");
            return Self::disabled();
        }
        tracing::info!(tag, "lease scopes on, through the systemd user manager");
        let host = host::LinuxHost::new(program, args);
        Self::with(Arc::new(bus), Arc::new(host), tag)
    }

    fn with(bus: Arc<dyn SystemdBus>, host: Arc<dyn Host>, tag: String) -> Self {
        Self {
            backend: Some(Backend {
                bus,
                host,
                tag,
                tracked: Mutex::new(Tracked::default()),
                starting: tokio::sync::Mutex::new(()),
            }),
        }
    }

    /// Starts looking for the command lease `id` just reported: notes the children `holder`
    /// already has, and makes any earlier watch of the lease stop. `None` without scopes.
    pub(crate) fn watch(&self, id: LeaseId, holder: u32) -> Option<Watch> {
        let backend = self.backend.as_ref()?;
        let baseline = backend.host.children(holder).into_iter().collect();
        let mut tracked = backend.lock();
        tracked.next_watch += 1;
        let generation = tracked.next_watch;
        tracked.watches.insert(id, generation);
        Some(Watch {
            id,
            holder,
            generation,
            baseline,
        })
    }

    /// The scope of lease `id`, granted at `granted_at_ms`, started with its anchor and the
    /// caps `config` turns on for `profile` unless the lease already has one.
    ///
    /// # Errors
    ///
    /// Fails without scopes, or when the anchor, the user manager, or the scope fails.
    pub(crate) async fn ensure(
        &self,
        id: LeaseId,
        granted_at_ms: i64,
        profile: &JobProfile,
        config: &LinuxConfig,
    ) -> Result<LeaseIsolation, ScopeError> {
        let backend = self.backend.as_ref().ok_or(ScopeError::NoScope(id))?;
        let _starting = backend.starting.lock().await;
        if let Some(scope) = backend.lock().scopes.get(&id) {
            return Ok(scope.isolation());
        }
        let name = scope_name(id, granted_at_ms, &backend.tag);
        let (caps, limits) = caps_for(profile, config);
        let host = &*backend.host;
        let anchor = host.spawn_anchor(&name).map_err(ScopeError::Anchor)?;
        if let Err(error) = backend.bus.start_scope(&name, anchor, caps).await {
            let _ = host.kill_process(anchor);
            return Err(error.into());
        }
        let Some(cgroup) = wait_for_cgroup(host, anchor, &name).await else {
            let _ = host.kill_process(anchor);
            return Err(ScopeError::NotStarted(name));
        };
        tracing::info!(lease = id.0, scope = name, "lease scope started");
        let scope = Scope {
            name,
            cgroup,
            limits,
            last: Some(Reading {
                at: Instant::now(),
                cpu_usage_usec: 0,
            }),
        };
        let isolation = scope.isolation();
        backend.lock().scopes.insert(id, scope);
        Ok(isolation)
    }

    /// The command's shell: the first new child of the holder that leads its own session and
    /// is neither an allot binary (a hook) nor still a fork of the holder. `None` when none
    /// starts `within` the time given, or a newer report replaced `watch`.
    pub(crate) async fn find_shell(&self, watch: &mut Watch, within: Duration) -> Option<u32> {
        let backend = self.backend.as_ref()?;
        let deadline = Instant::now() + within;
        loop {
            if !backend.is_current(watch) {
                return None;
            }
            if let Some(shell) = backend.new_session_leader(watch) {
                return Some(shell);
            }
            if Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(WATCH_INTERVAL).await;
        }
    }

    /// Moves the tree under `root` into lease `id`'s scope, pass after pass until one finds
    /// nothing outside, and returns how many processes moved. `protected` processes and
    /// everything below them stay where they are.
    ///
    /// # Errors
    ///
    /// Fails when the lease has no scope, the user manager refuses a process that still
    /// runs, or the tree keeps forking outside for [`MAX_PASSES`] passes.
    pub(crate) async fn settle(
        &self,
        id: LeaseId,
        root: u32,
        protected: &(dyn Fn(u32, &ProcInfo) -> bool + Sync),
    ) -> Result<usize, ScopeError> {
        let backend = self.backend.as_ref().ok_or(ScopeError::NoScope(id))?;
        let (name, cgroup) = {
            let tracked = backend.lock();
            let scope = tracked.scopes.get(&id).ok_or(ScopeError::NoScope(id))?;
            (scope.name.clone(), scope.cgroup.clone())
        };
        let host = &*backend.host;
        let mut moved = 0;
        for _ in 0..MAX_PASSES {
            let outside: Vec<u32> = tree(host, root, protected)
                .into_iter()
                .filter(|pid| host.cgroup_of(*pid).is_some_and(|other| other != cgroup))
                .collect();
            if outside.is_empty() {
                return Ok(moved);
            }
            if backend.bus.attach(&name, &outside).await.is_ok() {
                moved += outside.len();
                continue;
            }
            // One process that exited meanwhile fails the whole call; move the rest alone.
            for pid in outside {
                match backend.bus.attach(&name, &[pid]).await {
                    Ok(()) => moved += 1,
                    Err(error) if host.process(pid).is_some() => return Err(error.into()),
                    Err(_) => {}
                }
            }
        }
        Err(ScopeError::Unsettled(name))
    }

    /// CPU and RAM of every scope since the previous call, for the leases' run meters.
    pub(crate) fn usage(&self) -> Vec<(LeaseId, ScopeUsage)> {
        self.usage_at(Instant::now())
    }

    fn usage_at(&self, now: Instant) -> Vec<(LeaseId, ScopeUsage)> {
        let Some(backend) = self.backend.as_ref() else {
            return Vec::new();
        };
        let mut tracked = backend.lock();
        tracked
            .scopes
            .iter_mut()
            .filter_map(|(id, scope)| {
                let reading = backend.host.usage(&scope.cgroup).ok()?;
                let cores = scope.last.map_or(0.0, |last| {
                    let elapsed = now.saturating_duration_since(last.at).as_secs_f64();
                    let used = reading.cpu_usage_usec.saturating_sub(last.cpu_usage_usec);
                    if elapsed > 0.0 {
                        (used as f64 / 1e6 / elapsed) as f32
                    } else {
                        0.0
                    }
                });
                scope.last = Some(Reading {
                    at: now,
                    cpu_usage_usec: reading.cpu_usage_usec,
                });
                let usage = ScopeUsage {
                    cores,
                    ram_mb: reading.memory_bytes / BYTES_PER_MB,
                };
                Some((*id, usage))
            })
            .collect()
    }

    /// Ends every process in lease `id`'s scope at once and forgets the scope. Returns the
    /// processes it ended, without the anchor, by PID and name; `None` when the lease has no
    /// scope, or a protected process (a nested Claude session, an allot binary) started in
    /// it after the move, which the tree kill then spares.
    pub(crate) async fn kill(&self, id: LeaseId) -> Option<Vec<(u32, String)>> {
        let backend = self.backend.as_ref()?;
        let scope = {
            let mut tracked = backend.lock();
            tracked.watches.remove(&id);
            tracked.scopes.remove(&id)?
        };
        let host = &*backend.host;
        let pids = host.procs(&scope.cgroup);
        let anchor = find_anchor(host, &pids, &scope.name);
        let members: Vec<(u32, String)> = pids
            .iter()
            .filter(|pid| Some(**pid) != anchor)
            .filter_map(|pid| host.process(*pid).map(|info| (*pid, info.comm)))
            .collect();
        if members.iter().any(|(_, comm)| is_protected_name(comm)) {
            tracing::warn!(
                lease = id.0,
                scope = scope.name,
                "a protected process runs in the scope; only the process tree is killed"
            );
            end_anchor(host, anchor);
            return None;
        }
        if let Err(error) = host.kill_cgroup(&scope.cgroup) {
            tracing::warn!(
                scope = scope.name,
                "cgroup.kill failed ({error}); asking systemd"
            );
            if let Err(error) = backend.bus.kill(&scope.name).await {
                tracing::warn!(scope = scope.name, "killing the scope failed: {error}");
                end_anchor(host, anchor);
                return None;
            }
        }
        Some(members)
    }

    /// Ends the anchors of the scopes of leases `keep` rejects, and their watches. Processes
    /// still running in such a scope keep running; systemd removes it once they exit.
    pub(crate) fn retain(&self, keep: impl Fn(LeaseId) -> bool) {
        let Some(backend) = self.backend.as_ref() else {
            return;
        };
        let ended: Vec<(LeaseId, Scope)> = {
            let mut tracked = backend.lock();
            tracked.watches.retain(|id, _| keep(*id));
            let ids: Vec<LeaseId> = tracked
                .scopes
                .keys()
                .copied()
                .filter(|id| !keep(*id))
                .collect();
            ids.into_iter()
                .filter_map(|id| tracked.scopes.remove(&id).map(|scope| (id, scope)))
                .collect()
        };
        let host = &*backend.host;
        for (id, scope) in ended {
            let pids = host.procs(&scope.cgroup);
            end_anchor(host, find_anchor(host, &pids, &scope.name));
            tracing::info!(lease = id.0, scope = scope.name, "lease scope released");
        }
    }

    /// Takes back the scopes a previous run left for the leases in `active`, by ID and grant
    /// time in milliseconds, and ends the anchors of this install's other scopes, whose lease
    /// ended while allotd was down.
    pub(crate) async fn adopt(&self, active: &[(LeaseId, i64)]) -> Vec<(LeaseId, LeaseIsolation)> {
        let Some(backend) = self.backend.as_ref() else {
            return Vec::new();
        };
        let units = match backend.bus.list_units(&scope_pattern(&backend.tag)).await {
            Ok(units) => units,
            Err(error) => {
                tracing::warn!("listing scopes left from the last run failed: {error}");
                return Vec::new();
            }
        };
        let host = &*backend.host;
        let mut adopted = Vec::new();
        for unit in units {
            let Some((id, granted_secs)) = parse_scope_name(&unit, &backend.tag) else {
                continue;
            };
            let Ok(Some(cgroup)) = backend.bus.control_group(&unit).await else {
                continue;
            };
            let is_active = active.iter().any(|(active_id, granted_at_ms)| {
                *active_id == id && granted_at_ms.div_euclid(1_000) == granted_secs
            });
            if !is_active {
                let pids = host.procs(&cgroup);
                end_anchor(host, find_anchor(host, &pids, &unit));
                tracing::info!(scope = unit, "released a scope whose lease ended");
                continue;
            }
            let scope = Scope {
                name: unit,
                limits: host.limits(&cgroup),
                cgroup,
                last: None,
            };
            adopted.push((id, scope.isolation()));
            backend.lock().scopes.insert(id, scope);
        }
        adopted
    }
}

impl Backend {
    fn lock(&self) -> MutexGuard<'_, Tracked> {
        self.tracked.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn is_current(&self, watch: &Watch) -> bool {
        self.lock().watches.get(&watch.id) == Some(&watch.generation)
    }

    fn new_session_leader(&self, watch: &mut Watch) -> Option<u32> {
        for pid in self.host.children(watch.holder) {
            if watch.baseline.contains(&pid) {
                continue;
            }
            let Some(info) = self.host.process(pid) else {
                watch.baseline.insert(pid);
                continue;
            };
            // A fresh fork still runs as the holder until it calls setsid and execs.
            if info.session != pid || is_session_root(&info.comm) {
                continue;
            }
            if is_allot_binary(&info.comm) {
                watch.baseline.insert(pid);
                continue;
            }
            return Some(pid);
        }
        None
    }
}

impl Scope {
    fn isolation(&self) -> LeaseIsolation {
        LeaseIsolation {
            kind: IsolationKind::Cgroup,
            scope: Some(self.name.clone()),
            limits: self.limits,
        }
    }
}

/// Whether a process named `comm` must never be moved into a scope or killed with one.
pub(crate) fn is_protected_name(comm: &str) -> bool {
    is_allot_binary(comm) || is_session_root(comm)
}

fn is_allot_binary(comm: &str) -> bool {
    ALLOT_BINARIES.contains(&comm)
}

fn is_session_root(comm: &str) -> bool {
    SESSION_ROOTS.contains(&comm)
}

/// The caps `config` turns on for `profile`: CPU time at its cores and memory throttling at
/// its RAM. A profile without cores or RAM is not capped in that resource.
fn caps_for(profile: &JobProfile, config: &LinuxConfig) -> (ScopeCaps, EnforcedLimits) {
    let cores = (config.enforce_cpu && profile.cores > 0.0).then_some(profile.cores);
    let ram_mb = (config.enforce_memory && profile.ram_mb > 0).then_some(profile.ram_mb);
    let caps = ScopeCaps {
        cpu_usec_per_sec: cores.map(|cores| (f64::from(cores) * 1e6).round() as u64),
        memory_high_bytes: ram_mb.map(|mb| mb.saturating_mul(BYTES_PER_MB)),
    };
    let limits = EnforcedLimits {
        cpu_cores: cores,
        memory_high_mb: ram_mb,
        memory_max_mb: None,
    };
    (caps, limits)
}

/// The cgroup of the scope `name` once its anchor shows up in it.
async fn wait_for_cgroup(host: &dyn Host, anchor: u32, name: &str) -> Option<String> {
    let suffix = format!("/{name}");
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        let cgroup = host.cgroup_of(anchor)?;
        if cgroup.ends_with(&suffix) {
            return Some(cgroup);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(WATCH_INTERVAL).await;
    }
}

/// `root` and its descendants, without `protected` processes and what runs below them.
fn tree(host: &dyn Host, root: u32, protected: &dyn Fn(u32, &ProcInfo) -> bool) -> Vec<u32> {
    let mut members = Vec::new();
    let mut pending = vec![root];
    while let Some(pid) = pending.pop() {
        let Some(info) = host.process(pid) else {
            continue;
        };
        if protected(pid, &info) {
            continue;
        }
        members.push(pid);
        pending.extend(host.children(pid));
    }
    members
}

fn end_anchor(host: &dyn Host, anchor: Option<u32>) {
    if let Some(anchor) = anchor
        && let Err(error) = host.kill_process(anchor)
    {
        tracing::warn!(pid = anchor, "ending a scope's anchor failed: {error}");
    }
}

/// `allot-<lease id>-<grant time in seconds>-<tag>.scope`. The grant time keeps names apart
/// when lease IDs start over (a deleted database) while an old scope still runs a job.
fn scope_name(id: LeaseId, granted_at_ms: i64, tag: &str) -> String {
    let granted_secs = granted_at_ms.div_euclid(1_000);
    format!("allot-{}-{granted_secs}-{tag}.scope", id.0)
}

fn scope_pattern(tag: &str) -> String {
    format!("allot-*-{tag}.scope")
}

/// The lease ID and grant time in seconds that [`scope_name`] put into `name`.
fn parse_scope_name(name: &str, tag: &str) -> Option<(LeaseId, i64)> {
    let (id, granted_secs) = name
        .strip_prefix("allot-")?
        .strip_suffix(".scope")?
        .strip_suffix(tag)?
        .strip_suffix('-')?
        .split_once('-')?;
    Some((LeaseId(id.parse().ok()?), granted_secs.parse().ok()?))
}

/// Eight hex digits of the FNV-1a hash of `data_dir`, stable across Rust releases unlike the
/// standard hasher, so a restarted or updated allotd finds its scopes by name.
fn install_tag(data_dir: &Path) -> String {
    let hash = data_dir
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0x811c_9dc5_u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
        });
    format!("{hash:08x}")
}

#[cfg(test)]
mod tests {
    use allot_shared::platform::{CLI_EXE, DAEMON_EXE, MCP_EXE};

    use super::*;
    use crate::scopes::{
        fake::{APP_SLICE, Call, Fake, SESSION_SCOPE},
        host::CgroupReading,
    };

    const TAG: &str = "0a1b2c3d";
    /// When the test leases were granted, in milliseconds and in the scope names' seconds.
    const GRANTED_MS: i64 = 1_793_193_211_500;
    const GRANTED: i64 = 1_793_193_211;
    const CLAUDE: &str = SESSION_ROOTS[0];
    /// The Claude session's root process.
    const HOLDER: u32 = 100;

    fn scopes(fake: &Fake) -> Scopes {
        Scopes::with(Arc::new(fake.clone()), Arc::new(fake.clone()), TAG.into())
    }

    fn profile(cores: f32, ram_mb: u64) -> JobProfile {
        JobProfile {
            cores,
            ram_mb,
            gpu_pct: 0.0,
            vram_mb: 0,
            est_secs: 60,
        }
    }

    fn config(enforce_cpu: bool, enforce_memory: bool) -> LinuxConfig {
        LinuxConfig {
            scopes: true,
            enforce_cpu,
            enforce_memory,
        }
    }

    /// A Claude session with its MCP server, which shares its session.
    fn session() -> Fake {
        let fake = Fake::new();
        fake.add(HOLDER, 1, CLAUDE, 90);
        fake.add(101, HOLDER, MCP_EXE, 90);
        fake
    }

    async fn start(
        scopes: &Scopes,
        id: i64,
        profile: JobProfile,
        config: LinuxConfig,
    ) -> Result<LeaseIsolation, ScopeError> {
        scopes
            .ensure(LeaseId(id), GRANTED_MS, &profile, &config)
            .await
    }

    fn name(id: i64) -> String {
        format!("allot-{id}-{GRANTED}-{TAG}.scope")
    }

    fn cgroup(id: i64) -> String {
        format!("{APP_SLICE}/{}", name(id))
    }

    fn by_name(_: u32, info: &ProcInfo) -> bool {
        is_protected_name(&info.comm)
    }

    fn anchor_of(fake: &Fake, id: i64) -> u32 {
        let scope = name(id);
        fake.calls()
            .into_iter()
            .find_map(|call| match call {
                Call::Start {
                    scope: started,
                    anchor,
                    ..
                } if started == scope => Some(anchor),
                _ => None,
            })
            .unwrap()
    }

    /// Lease 1's scope holding the shell 200 and its child 201.
    async fn scoped_build(fake: &Fake, scopes: &Scopes) {
        fake.add(200, HOLDER, "zsh", 200);
        fake.add(201, 200, "cargo", 200);
        start(scopes, 1, profile(4.0, 0), config(false, false))
            .await
            .unwrap();
        scopes.settle(LeaseId(1), 200, &by_name).await.unwrap();
    }

    #[tokio::test]
    async fn test_ensure_starts_one_scope_with_the_enabled_caps() {
        let fake = session();
        let scopes = scopes(&fake);
        let isolation = start(&scopes, 7, profile(4.0, 2_048), config(true, true))
            .await
            .unwrap();
        assert_eq!(isolation.kind, IsolationKind::Cgroup);
        assert_eq!(
            isolation.scope.as_deref(),
            Some("allot-7-1793193211-0a1b2c3d.scope")
        );
        let limits = EnforcedLimits {
            cpu_cores: Some(4.0),
            memory_high_mb: Some(2_048),
            memory_max_mb: None,
        };
        assert_eq!(isolation.limits, limits);
        let again = start(&scopes, 7, profile(1.0, 0), config(false, false))
            .await
            .unwrap();
        assert_eq!(again, isolation);

        let calls = fake.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        let caps = ScopeCaps {
            cpu_usec_per_sec: Some(4_000_000),
            memory_high_bytes: Some(2_048 * BYTES_PER_MB),
        };
        assert!(matches!(&calls[0], Call::Start { caps: started, .. } if *started == caps));
        assert_eq!(fake.cgroup(anchor_of(&fake, 7)), Some(cgroup(7)));
    }

    #[test]
    fn test_caps_are_off_by_default_and_for_an_empty_profile() {
        let (caps, limits) = caps_for(&profile(4.0, 2_048), &config(false, false));
        assert_eq!(caps, ScopeCaps::default());
        assert_eq!(limits, EnforcedLimits::default());
        let (caps, limits) = caps_for(&profile(0.0, 0), &config(true, true));
        assert_eq!(caps, ScopeCaps::default());
        assert_eq!(limits, EnforcedLimits::default());
        let (caps, _) = caps_for(&profile(0.5, 0), &config(true, true));
        assert_eq!(caps.cpu_usec_per_sec, Some(500_000));
    }

    #[tokio::test]
    async fn test_ensure_ends_the_anchor_when_the_manager_refuses() {
        let fake = session();
        fake.machine().fail_start = true;
        let error = start(&scopes(&fake), 7, profile(4.0, 0), config(false, false))
            .await
            .unwrap_err();
        assert!(matches!(error, ScopeError::Bus(_)), "{error}");
        assert!(!fake.alive(anchor_of(&fake, 7)));
    }

    #[tokio::test]
    async fn test_find_shell_skips_older_children_hooks_and_unfinished_forks() {
        let fake = session();
        fake.add(102, HOLDER, "zsh", 102);
        let scopes = scopes(&fake);
        let mut watch = scopes.watch(LeaseId(1), HOLDER).unwrap();
        fake.add(103, HOLDER, CLI_EXE, 103);
        fake.add(104, HOLDER, CLAUDE, 90);
        assert_eq!(scopes.find_shell(&mut watch, Duration::ZERO).await, None);
        fake.add(105, HOLDER, "zsh", 105);
        assert_eq!(
            scopes.find_shell(&mut watch, Duration::ZERO).await,
            Some(105)
        );
    }

    #[tokio::test]
    async fn test_find_shell_waits_for_the_shell_to_start() {
        let fake = session();
        let scopes = scopes(&fake);
        let mut watch = scopes.watch(LeaseId(1), HOLDER).unwrap();
        let later = fake.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            later.add(200, HOLDER, "bwrap", 200);
        });
        let shell = scopes.find_shell(&mut watch, Duration::from_secs(5)).await;
        assert_eq!(shell, Some(200));
    }

    #[tokio::test]
    async fn test_find_shell_stops_when_a_newer_report_replaces_the_watch() {
        let fake = session();
        let scopes = scopes(&fake);
        let mut older = scopes.watch(LeaseId(1), HOLDER).unwrap();
        let mut newer = scopes.watch(LeaseId(1), HOLDER).unwrap();
        fake.add(200, HOLDER, "zsh", 200);
        assert_eq!(scopes.find_shell(&mut older, Duration::ZERO).await, None);
        assert_eq!(
            scopes.find_shell(&mut newer, Duration::ZERO).await,
            Some(200)
        );
    }

    #[tokio::test]
    async fn test_settle_moves_the_whole_tree_and_late_forks() {
        let fake = session();
        let scopes = scopes(&fake);
        start(&scopes, 1, profile(4.0, 0), config(false, false))
            .await
            .unwrap();
        fake.add(200, HOLDER, "zsh", 200);
        fake.add(201, 200, "cargo", 200);
        fake.add(202, 201, "rustc", 200);
        // cargo starts another rustc while the first move is under way.
        fake.machine().fork_on_attach = vec![(203, 201)];
        let moved = scopes.settle(LeaseId(1), 200, &by_name).await.unwrap();
        assert_eq!(moved, 4);
        for pid in 200..=203 {
            assert_eq!(fake.cgroup(pid), Some(cgroup(1)), "{pid}");
        }
        assert_eq!(fake.cgroup(HOLDER).as_deref(), Some(SESSION_SCOPE));
        let attaches = fake
            .calls()
            .iter()
            .filter(|call| matches!(call, Call::Attach { .. }))
            .count();
        assert_eq!(attaches, 2);
    }

    #[tokio::test]
    async fn test_settle_leaves_protected_processes_and_their_children() {
        let fake = session();
        let scopes = scopes(&fake);
        start(&scopes, 1, profile(4.0, 0), config(false, false))
            .await
            .unwrap();
        fake.add(200, HOLDER, "zsh", 200);
        fake.add(210, 200, CLAUDE, 200);
        fake.add(211, 210, "node", 200);
        fake.add(220, 200, "make", 200);
        let holders: HashSet<u32> = [HOLDER, 210].into();
        let protected =
            |pid: u32, info: &ProcInfo| holders.contains(&pid) || is_protected_name(&info.comm);
        let moved = scopes.settle(LeaseId(1), 200, &protected).await.unwrap();
        assert_eq!(moved, 2);
        assert_eq!(fake.cgroup(220), Some(cgroup(1)));
        assert_eq!(fake.cgroup(210).as_deref(), Some(SESSION_SCOPE));
        assert_eq!(fake.cgroup(211).as_deref(), Some(SESSION_SCOPE));
    }

    #[tokio::test]
    async fn test_settle_moves_the_rest_alone_when_one_exits() {
        let fake = session();
        let scopes = scopes(&fake);
        start(&scopes, 1, profile(4.0, 0), config(false, false))
            .await
            .unwrap();
        fake.add(200, HOLDER, "zsh", 200);
        fake.add(201, 200, "true", 200);
        fake.add(202, 200, "cc", 200);
        fake.machine().exit_on_attach = vec![201];
        let moved = scopes.settle(LeaseId(1), 200, &by_name).await.unwrap();
        assert_eq!(moved, 2);
        assert_eq!(fake.cgroup(202), Some(cgroup(1)));
    }

    #[tokio::test]
    async fn test_settle_without_a_scope_fails() {
        let fake = session();
        fake.add(200, HOLDER, "zsh", 200);
        let error = scopes(&fake)
            .settle(LeaseId(1), 200, &by_name)
            .await
            .unwrap_err();
        assert!(matches!(error, ScopeError::NoScope(LeaseId(1))), "{error}");
    }

    #[tokio::test]
    async fn test_usage_reports_cores_since_the_last_reading() {
        let fake = session();
        let scopes = scopes(&fake);
        start(&scopes, 1, profile(4.0, 0), config(false, false))
            .await
            .unwrap();
        let reading = |cpu_usage_usec, memory_bytes| CgroupReading {
            cpu_usage_usec,
            memory_bytes,
        };
        fake.machine().usage.insert(cgroup(1), reading(0, 0));
        let start = Instant::now();
        scopes.usage_at(start);
        fake.machine()
            .usage
            .insert(cgroup(1), reading(3_000_000, 512 * BYTES_PER_MB));
        let usage = scopes.usage_at(start + Duration::from_secs(2));
        let expected = ScopeUsage {
            cores: 1.5,
            ram_mb: 512,
        };
        assert_eq!(usage, [(LeaseId(1), expected)]);
    }

    #[tokio::test]
    async fn test_kill_ends_the_scope_and_reports_its_processes_without_the_anchor() {
        let fake = session();
        let scopes = scopes(&fake);
        scoped_build(&fake, &scopes).await;
        let anchor = anchor_of(&fake, 1);
        let killed = scopes.kill(LeaseId(1)).await.unwrap();
        assert_eq!(killed, [(200, "zsh".to_owned()), (201, "cargo".to_owned())]);
        assert!(![200, 201, anchor].iter().any(|pid| fake.alive(*pid)));
        assert!(fake.alive(HOLDER));
        assert_eq!(scopes.kill(LeaseId(1)).await, None);
    }

    #[tokio::test]
    async fn test_kill_leaves_a_scope_with_a_protected_process_to_the_tree_kill() {
        let fake = session();
        let scopes = scopes(&fake);
        scoped_build(&fake, &scopes).await;
        // A nested Claude session the command started after the move runs in the scope.
        fake.add(230, 201, CLAUDE, 230);
        fake.machine().procs.get_mut(&230).unwrap().cgroup = cgroup(1);
        let anchor = anchor_of(&fake, 1);
        assert_eq!(scopes.kill(LeaseId(1)).await, None);
        assert!(!fake.alive(anchor));
        assert!(fake.alive(200) && fake.alive(230));
    }

    #[tokio::test]
    async fn test_retain_ends_only_the_anchor_of_ended_leases() {
        let fake = session();
        let scopes = scopes(&fake);
        scoped_build(&fake, &scopes).await;
        start(&scopes, 2, profile(1.0, 0), config(false, false))
            .await
            .unwrap();
        scopes.retain(|id| id == LeaseId(2));
        assert!(!fake.alive(anchor_of(&fake, 1)));
        assert!(fake.alive(200) && fake.alive(201));
        assert_eq!(fake.cgroup(201), Some(cgroup(1)));
        assert!(fake.alive(anchor_of(&fake, 2)));
        assert_eq!(scopes.kill(LeaseId(1)).await, None);
    }

    #[tokio::test]
    async fn test_adopt_takes_back_active_scopes_and_releases_the_rest() {
        let fake = session();
        let other_install = format!("allot-5-{GRANTED}-ffffffff.scope");
        // Lease 6's ID came back after a database reset; its old scope still runs a job.
        let earlier_lease_6 = format!("allot-6-{}-{TAG}.scope", GRANTED - 86_400);
        for (pid, scope) in [
            (300, name(3)),
            (400, name(4)),
            (500, other_install),
            (600, earlier_lease_6),
        ] {
            fake.add(pid, 1, DAEMON_EXE, pid);
            let mut machine = fake.machine();
            let anchor = machine.procs.get_mut(&pid).unwrap();
            anchor.cgroup = format!("{APP_SLICE}/{scope}");
            anchor.cmdline = vec![DAEMON_EXE.into(), ANCHOR_FLAG.into(), scope.clone()];
            machine.units.push(scope);
        }
        fake.machine().units.push("other.scope".into());
        let cpu_capped = EnforcedLimits {
            cpu_cores: Some(2.0),
            ..EnforcedLimits::default()
        };
        fake.machine().limits.insert(cgroup(3), cpu_capped);

        let scopes = scopes(&fake);
        let active = [3, 5, 6].map(|id| (LeaseId(id), GRANTED_MS));
        let adopted = scopes.adopt(&active).await;
        let isolation = LeaseIsolation {
            kind: IsolationKind::Cgroup,
            scope: Some(name(3)),
            limits: cpu_capped,
        };
        assert_eq!(adopted, [(LeaseId(3), isolation)]);
        assert!(fake.alive(300));
        assert!(!fake.alive(400));
        assert!(fake.alive(500));
        assert!(!fake.alive(600));
        assert_eq!(scopes.kill(LeaseId(3)).await, Some(Vec::new()));
    }

    #[tokio::test]
    async fn test_disabled_scopes_do_nothing() {
        let scopes = Scopes::disabled();
        assert!(scopes.watch(LeaseId(1), HOLDER).is_none());
        assert!(scopes.usage().is_empty());
        assert_eq!(scopes.kill(LeaseId(1)).await, None);
        assert!(scopes.adopt(&[(LeaseId(1), GRANTED_MS)]).await.is_empty());
        let error = start(&scopes, 1, profile(1.0, 0), config(true, true))
            .await
            .unwrap_err();
        assert!(matches!(error, ScopeError::NoScope(LeaseId(1))), "{error}");
    }

    #[test]
    fn test_scope_names_round_trip_and_the_tag_is_stable() {
        let name = scope_name(LeaseId(42), GRANTED_MS, TAG);
        assert_eq!(name, "allot-42-1793193211-0a1b2c3d.scope");
        assert_eq!(parse_scope_name(&name, TAG), Some((LeaseId(42), GRANTED)));
        assert_eq!(
            parse_scope_name("allot-42-1793193211-ffffffff.scope", TAG),
            None
        );
        assert_eq!(parse_scope_name("allot-42-0a1b2c3d.scope", TAG), None);
        assert_eq!(parse_scope_name("allot-x-1-0a1b2c3d.scope", TAG), None);
        assert_eq!(install_tag(Path::new("/home/mehran/.allot")), "e95eec3c");
    }

    /// Runs against the real user manager where there is one: moves a tree this test starts,
    /// reads its usage under a one-core cap, and kills it with its scope.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_live_scope_moves_measures_and_kills_a_tree() {
        use std::{os::unix::process::ExitStatusExt, process::Command};

        let dir = tempfile::tempdir().unwrap();
        // An anchor whose last argument is the scope name and which starts no child: a child
        // forked before systemd moves the anchor would stay outside the scope.
        let anchor_args = vec!["-c".to_owned(), "import time; time.sleep(600)".to_owned()];
        let scopes = Scopes::connect(dir.path(), "python3".into(), anchor_args).await;
        if scopes.backend.is_none() {
            eprintln!("skipped: no systemd user manager on cgroup v2");
            return;
        }
        let id = LeaseId(1);
        let isolation = scopes
            .ensure(id, GRANTED_MS, &profile(1.0, 0), &config(true, false))
            .await;
        let mut command = Command::new("setsid")
            .args(["sh", "-c", "sleep 600 & while :; do :; done"])
            .spawn()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let settled = match &isolation {
            Ok(_) => scopes
                .settle(id, command.id(), &by_name)
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let backend = scopes.backend.as_ref().unwrap();
        let moved = tree(&*backend.host, command.id(), &by_name);
        tokio::time::sleep(Duration::from_millis(1_500)).await;
        let usage = scopes.usage();
        let killed = scopes.kill(id).await;
        let _ = command.kill();
        let status = command.wait().unwrap();
        if let Some(scope) = isolation.as_ref().ok().and_then(|iso| iso.scope.clone()) {
            let _ = Command::new("systemctl")
                .args(["--user", "stop", &scope])
                .status();
        }

        // The orphaned sleep is reaped by whoever adopted it, which can take a moment.
        let mut survivors = moved.clone();
        for _ in 0..100 {
            survivors.retain(|pid| backend.host.process(*pid).is_some());
            if survivors.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let isolation = isolation.unwrap();
        assert_eq!(isolation.limits.cpu_cores, Some(1.0));
        assert_eq!(moved.len(), 2, "{moved:?}");
        assert!(survivors.is_empty(), "{survivors:?}");
        assert!(settled.unwrap() >= 2);
        let [(_, usage)] = usage[..] else {
            panic!("{usage:?}");
        };
        assert!((0.5..=1.1).contains(&usage.cores), "{usage:?}");
        assert!(killed.is_some_and(|killed| killed.len() >= 2));
        assert_eq!(status.signal(), Some(9));
    }
}
