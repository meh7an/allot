//! The MCP server and its five tools.

use std::sync::Arc;

use allot_client::ClientError;
use allot_shared::{
    api::{AcquireRequest, CanFitRequest, LeaseQuery, Verdict},
    types::{EndCause, Holder, JobProfile, Lease, LeaseId, LeaseKind, LeaseState, NoiseReport},
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use serde::Serialize;

use crate::{
    daemon::DaemonApi,
    held::HeldLeases,
    params::{AcquireParams, CanFitParams, DEFAULT_WAIT_SECS, ListLeasesParams, ReleaseParams},
    summary::SnapshotSummary,
};

const INSTRUCTIONS: &str = "Coordinates heavy jobs between Claude sessions on this machine. \
Before a heavy command (builds, test suites, training, rendering, benchmarks), call \
acquire_lease with a class or profile and the command as label; run the command only once \
the lease is active, then call release_lease. Use system_snapshot and can_fit to judge the \
machine, and kind benchmark_exclusive when measurements must not be disturbed. Leases are \
kept alive automatically and released when the session ends.";

/// The tools of one Claude session, backed by the daemon through `C`.
pub struct AllotServer<C: DaemonApi> {
    client: Arc<C>,
    holder: Holder,
    held: Arc<HeldLeases>,
    /// The session's working directory, for requests that name none.
    cwd: String,
    tool_router: ToolRouter<Self>,
}

/// What `acquire_lease` reports.
#[derive(Debug, Serialize)]
struct AcquireOutcome {
    lease_id: i64,
    state: LeaseState,
    kind: LeaseKind,
    profile: JobProfile,
    label: String,
    /// Why the lease ended, when it did without starting or while it waited.
    #[serde(skip_serializing_if = "Option::is_none")]
    end_cause: Option<EndCause>,
    verdict: Verdict,
    next_step: &'static str,
}

/// What `release_lease` reports: the lease, plus the noise report of a benchmark.
#[derive(Debug, Serialize)]
struct ReleaseOutcome {
    #[serde(flatten)]
    lease: Lease,
    #[serde(skip_serializing_if = "Option::is_none")]
    noise: Option<NoiseReport>,
    /// What the user did to the lease, when they stopped it.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'static str>,
}

/// What `list_leases` reports.
#[derive(Debug, Serialize)]
struct LeaseList {
    leases: Vec<Lease>,
    /// This session's leases the user stopped or removed that it had not been told about.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stopped_by_user: Vec<StoppedLease>,
}

/// One lease the user stopped or removed from Allot.
#[derive(Debug, Serialize)]
struct StoppedLease {
    lease_id: i64,
    label: String,
    end_cause: EndCause,
    note: &'static str,
}

#[tool_router]
impl<C: DaemonApi> AllotServer<C> {
    /// A server acting for `holder`, tracking acquired leases in `held`. Requests without a
    /// working directory get `cwd`.
    pub fn new(client: Arc<C>, holder: Holder, held: Arc<HeldLeases>, cwd: String) -> Self {
        Self {
            client,
            holder,
            held,
            cwd,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Current machine load: CPU, memory, each GPU, every Claude session's \
                       usage (is_current marks yours), and load from everything else.",
        annotations(read_only_hint = true)
    )]
    async fn system_snapshot(&self) -> Result<String, String> {
        let snapshot = self.client.snapshot().await.map_err(|e| e.to_string())?;
        to_json(&SnapshotSummary::new(&snapshot, &self.holder.session))
    }

    #[tool(
        description = "Whether a job with this profile could start right now, with the \
                       limiting resources and an estimated wait. With label (the command), \
                       the profile is raised to what earlier runs of it needed. Changes \
                       nothing.",
        annotations(read_only_hint = true)
    )]
    async fn can_fit(
        &self,
        Parameters(params): Parameters<CanFitParams>,
    ) -> Result<String, String> {
        let cwd = params
            .label
            .is_some()
            .then(|| self.cwd_or_default(params.cwd));
        let request = CanFitRequest {
            profile: params.profile.to_spec()?,
            kind: params.kind.into(),
            label: params.label,
            cwd,
        };
        to_json(
            &self
                .client
                .can_fit(&request)
                .await
                .map_err(|e| e.to_string())?,
        )
    }

    #[tool(
        description = "Reserves resources for a heavy command before running it. Waits up \
                          to wait_secs for the lease to start; if it is still queued it keeps \
                          its place, and calling again with its lease_id keeps waiting. Run \
                          the command only when state is active, then call release_lease. \
                          The reserved profile is raised to what earlier runs of the same \
                          command needed. A waiting benchmark reports its progress in \
                          verdict.benchmark."
    )]
    async fn acquire_lease(
        &self,
        Parameters(params): Parameters<AcquireParams>,
    ) -> Result<String, String> {
        let wait_timeout_secs = params.wait_secs.unwrap_or(DEFAULT_WAIT_SECS);
        let response = match params.lease_id {
            Some(id) => self
                .client
                .wait(LeaseId(id), &self.holder.session, wait_timeout_secs)
                .await
                .map_err(|error| {
                    let mut message = error.to_string();
                    if matches!(error, ClientError::Interrupted { .. }) {
                        message.push_str(&format!(
                            ". A restarted allotd keeps the lease: call acquire_lease with only \
                             lease_id {id} again."
                        ));
                    }
                    message
                })?,
            None => {
                if params.label.trim().is_empty() {
                    return Err("give `label`, the command the lease is for".to_owned());
                }
                let request = AcquireRequest {
                    holder: self.holder.clone(),
                    profile: params.profile.to_spec()?,
                    kind: params.kind.into(),
                    label: params.label,
                    cwd: Some(self.cwd_or_default(params.cwd)),
                    wait_timeout_secs,
                };
                self.client.acquire(&request).await.map_err(|error| {
                    let mut message = error.to_string();
                    if matches!(error, ClientError::Interrupted { .. }) {
                        message.push_str(
                            ". The lease may exist anyway: call list_leases, then resume it with \
                             acquire_lease and only its lease_id, or release it. Do not run \
                             the command before a lease is active.",
                        );
                    }
                    message
                })?
            }
        };
        let lease = response.lease;
        let next_step = match lease.state {
            LeaseState::Active => {
                self.held.insert(lease.id);
                "Run the command now, then call release_lease with this lease_id."
            }
            LeaseState::Queued => {
                self.held.insert(lease.id);
                "Still queued and keeping its place. Call acquire_lease with this lease_id \
                 to keep waiting before running the command, or release_lease to give up."
            }
            _ => {
                self.held.remove(lease.id);
                match lease.end_cause {
                    Some(EndCause::Removed) => {
                        "The user removed this request from the queue in Allot; it \
                         did not run. Do not run the command or acquire it again; tell the user \
                         in one line that it was cancelled."
                    }
                    Some(EndCause::Killed) => {
                        "The user stopped this job from Allot. Do not run the \
                         command again on your own; ask the user whether to retry or skip."
                    }
                    _ if lease.state == LeaseState::Expired && lease.granted_at_ms.is_some() => {
                        "allotd ended this lease after it started, because nothing ran under it \
                         while the session was idle. The command still needs to run: acquire a \
                         new lease for it (without lease_id), then run it."
                    }
                    _ => "The lease ended before it started; do not run the command.",
                }
            }
        };
        self.mark_told(lease.id, lease.end_cause).await;
        to_json(&AcquireOutcome {
            lease_id: lease.id.0,
            state: lease.state,
            kind: lease.kind,
            profile: lease.profile,
            label: lease.label,
            end_cause: lease.end_cause,
            verdict: response.verdict,
            next_step,
        })
    }

    #[tool(
        description = "Ends a lease once its command has finished (or to give up a queued \
                          one). Releasing an ended lease is harmless. Ending a benchmark lease \
                          returns its noise report: load from outside this session while it \
                          ran."
    )]
    async fn release_lease(
        &self,
        Parameters(params): Parameters<ReleaseParams>,
    ) -> Result<String, String> {
        let id = LeaseId(params.lease_id);
        let response = self
            .client
            .release(id, &self.holder.session)
            .await
            .map_err(|e| e.to_string())?;
        self.held.remove(id);
        let note = stopped_note(response.lease.end_cause);
        self.mark_told(id, response.lease.end_cause).await;
        to_json(&ReleaseOutcome {
            lease: response.lease,
            noise: response.noise,
            note,
        })
    }

    #[tool(
        description = "Active and queued leases: this session's by default, or every \
                       session's with all_sessions. stopped_by_user lists this session's \
                       leases the user stopped or removed from Allot since it was \
                       last told.",
        annotations(read_only_hint = true)
    )]
    async fn list_leases(
        &self,
        Parameters(params): Parameters<ListLeasesParams>,
    ) -> Result<String, String> {
        let session = (!params.all_sessions).then(|| self.holder.session.clone());
        let mut leases = Vec::new();
        for state in [LeaseState::Active, LeaseState::Queued] {
            let query = LeaseQuery {
                session: session.clone(),
                state: Some(state),
            };
            leases.extend(
                self.client
                    .leases(&query)
                    .await
                    .map_err(|e| e.to_string())?,
            );
        }
        let stopped_by_user = self.untold_stops().await;
        to_json(&LeaseList {
            leases,
            stopped_by_user,
        })
    }
}

impl<C: DaemonApi> AllotServer<C> {
    /// Records that this session was told about the user's kill or removal of lease `id`, so
    /// the hooks do not tell it again. A failure only means a hook repeats it.
    async fn mark_told(&self, id: LeaseId, cause: Option<EndCause>) {
        if !cause.is_some_and(EndCause::is_notice) {
            return;
        }
        if let Err(error) = self.client.mark_delivered(id, &self.holder.session).await {
            eprintln!(
                "allot-mcp: marking the notice of lease {} failed: {error}",
                id.0
            );
        }
    }

    /// This session's kills and removals not told yet, each marked told. Leaves out any that
    /// a hook told in the meantime, and is empty when the daemon cannot be asked.
    async fn untold_stops(&self) -> Vec<StoppedLease> {
        let session = &self.holder.session;
        let notices = match self.client.notices(session).await {
            Ok(notices) => notices,
            Err(error) => {
                eprintln!("allot-mcp: listing notices failed: {error}");
                return Vec::new();
            }
        };
        let mut stopped = Vec::new();
        for notice in notices {
            let Some(note) = stopped_note(Some(notice.cause)) else {
                continue;
            };
            if let Ok(true) = self.client.mark_delivered(notice.lease_id, session).await {
                stopped.push(StoppedLease {
                    lease_id: notice.lease_id.0,
                    label: notice.command,
                    end_cause: notice.cause,
                    note,
                });
            }
        }
        stopped
    }

    /// `cwd` as given, or the session's working directory when it is missing or blank.
    fn cwd_or_default(&self, cwd: Option<String>) -> String {
        cwd.filter(|cwd| !cwd.trim().is_empty())
            .unwrap_or_else(|| self.cwd.clone())
    }
}

#[tool_handler(router = self.tool_router)]
impl<C: DaemonApi> ServerHandler for AllotServer<C> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("allot-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// What the user did to a lease, in words, or `None` for an ordinary end.
fn stopped_note(cause: Option<EndCause>) -> Option<&'static str> {
    match cause? {
        EndCause::Killed => Some(
            "The user stopped this job from Allot. They have not said whether to \
             retry or skip.",
        ),
        EndCause::Removed => Some(
            "The user removed this request from the queue in Allot before it \
             started; it did not run.",
        ),
        _ => None,
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::{
        params::{ClassParam, KindParam, ProfileParams},
        test_support::{MockDaemon, holder, lease},
    };

    fn server_with(daemon: MockDaemon) -> (AllotServer<MockDaemon>, Arc<MockDaemon>) {
        let daemon = Arc::new(daemon);
        let server = AllotServer::new(
            daemon.clone(),
            holder("me"),
            Arc::default(),
            r"E:\session".into(),
        );
        (server, daemon)
    }

    fn json(output: Result<String, String>) -> Value {
        serde_json::from_str(&output.unwrap()).unwrap()
    }

    fn acquire(class: ClassParam) -> Parameters<AcquireParams> {
        Parameters(AcquireParams {
            profile: ProfileParams {
                class: Some(class),
                ..ProfileParams::default()
            },
            label: "cargo build --release".into(),
            ..AcquireParams::default()
        })
    }

    #[tokio::test]
    async fn test_system_snapshot_summarizes_and_marks_this_session() {
        let (server, _) = server_with(MockDaemon::default());
        let summary = json(server.system_snapshot().await);
        assert_eq!(summary["logical_cores"], 16);
        assert_eq!(summary["gpus"][0]["busiest_engine_pct"], 40.0);
        assert_eq!(summary["sessions"][0]["session"], "me");
        assert_eq!(summary["sessions"][0]["is_current"], true);
        assert_eq!(summary["sessions"][1]["is_current"], false);
        assert_eq!(summary["sessions"][0]["processes"], 1);
        assert!(summary.get("per_core_pct").is_none());
    }

    #[tokio::test]
    async fn test_can_fit_passes_the_profile_and_kind() {
        let (server, _) = server_with(MockDaemon::default());
        let params = CanFitParams {
            profile: ProfileParams {
                class: Some(ClassParam::GpuHeavy),
                ..ProfileParams::default()
            },
            kind: KindParam::Normal,
            ..CanFitParams::default()
        };
        let verdict = json(server.can_fit(Parameters(params)).await);
        assert_eq!(verdict["fits"], false);
        assert_eq!(verdict["wait_estimate_secs"], 30);
    }

    #[tokio::test]
    async fn test_can_fit_with_a_label_sends_it_with_the_session_directory() {
        let (server, _) = server_with(MockDaemon::default());
        let cpu = ProfileParams {
            class: Some(ClassParam::CpuHeavy),
            ..ProfileParams::default()
        };
        let labelled = CanFitParams {
            profile: cpu.clone(),
            label: Some("cargo build".into()),
            ..CanFitParams::default()
        };
        let verdict = json(server.can_fit(Parameters(labelled.clone())).await);
        assert_eq!(
            verdict["reasons"][0],
            r"history of cargo build in E:\session"
        );

        let elsewhere = CanFitParams {
            cwd: Some(r"E:\other".into()),
            ..labelled
        };
        let verdict = json(server.can_fit(Parameters(elsewhere)).await);
        assert_eq!(verdict["reasons"][0], r"history of cargo build in E:\other");

        let unlabelled = CanFitParams {
            profile: cpu,
            ..CanFitParams::default()
        };
        let verdict = json(server.can_fit(Parameters(unlabelled)).await);
        assert_eq!(verdict["reasons"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_acquire_sends_the_session_directory_unless_given_one() {
        let (server, daemon) = server_with(MockDaemon::default());
        server
            .acquire_lease(acquire(ClassParam::CpuHeavy))
            .await
            .unwrap();
        let mut params = acquire(ClassParam::CpuHeavy);
        params.0.cwd = Some(r"E:\other".into());
        server.acquire_lease(params).await.unwrap();
        let mut blank = acquire(ClassParam::CpuHeavy);
        blank.0.cwd = Some("  ".into());
        server.acquire_lease(blank).await.unwrap();

        let cwds = daemon.lease_cwds();
        assert_eq!(cwds, [r"E:\session", r"E:\other", r"E:\session"]);
    }

    #[tokio::test]
    async fn test_acquire_active_lease_is_held_and_explained() {
        let (server, _) = server_with(MockDaemon::default());
        let outcome = json(server.acquire_lease(acquire(ClassParam::CpuHeavy)).await);
        assert_eq!(outcome["state"], "active");
        assert_eq!(outcome["label"], "cargo build --release");
        assert!(
            outcome["next_step"]
                .as_str()
                .unwrap()
                .starts_with("Run the command now")
        );
        assert_eq!(server.held.ids(), [LeaseId(1)]);
    }

    #[tokio::test]
    async fn test_acquire_queued_lease_is_held_and_says_to_wait() {
        let (server, _) = server_with(MockDaemon::default());
        let outcome = json(server.acquire_lease(acquire(ClassParam::GpuHeavy)).await);
        assert_eq!(outcome["state"], "queued");
        assert!(
            outcome["next_step"]
                .as_str()
                .unwrap()
                .contains("acquire_lease with this lease_id")
        );
        assert_eq!(server.held.ids(), [LeaseId(1)]);
    }

    #[tokio::test]
    async fn test_acquire_with_lease_id_resumes_the_queued_lease() {
        let (server, daemon) = server_with(MockDaemon::default());
        server
            .acquire_lease(acquire(ClassParam::GpuHeavy))
            .await
            .unwrap();
        let resume = Parameters(AcquireParams {
            lease_id: Some(1),
            ..AcquireParams::default()
        });
        let outcome = json(server.acquire_lease(resume).await);
        assert_eq!(outcome["lease_id"], 1);
        assert_eq!(outcome["state"], "active");
        assert_eq!(daemon.leases_count(), 1);
        assert_eq!(server.held.ids(), [LeaseId(1)]);
    }

    fn resume(id: i64) -> Parameters<AcquireParams> {
        Parameters(AcquireParams {
            lease_id: Some(id),
            ..AcquireParams::default()
        })
    }

    #[tokio::test]
    async fn test_resume_of_a_removed_lease_says_it_did_not_run_and_tells_once() {
        let (server, daemon) = server_with(MockDaemon::default());
        server
            .acquire_lease(acquire(ClassParam::GpuHeavy))
            .await
            .unwrap();
        daemon.stop(LeaseId(1), EndCause::Removed);

        let outcome = json(server.acquire_lease(resume(1)).await);
        assert_eq!(outcome["state"], "expired");
        assert_eq!(outcome["end_cause"], "removed");
        let next_step = outcome["next_step"].as_str().unwrap();
        assert!(next_step.contains("it did not run"), "{next_step}");
        assert!(next_step.contains("cancelled"), "{next_step}");
        assert!(server.held.ids().is_empty());
        assert_eq!(daemon.delivered(), [LeaseId(1)]);
    }

    #[tokio::test]
    async fn test_resume_of_a_killed_lease_says_to_ask_retry_or_skip() {
        let (server, daemon) = server_with(MockDaemon::default());
        server
            .acquire_lease(acquire(ClassParam::CpuHeavy))
            .await
            .unwrap();
        daemon.stop(LeaseId(1), EndCause::Killed);
        let outcome = json(server.acquire_lease(resume(1)).await);
        assert_eq!(outcome["end_cause"], "killed");
        let next_step = outcome["next_step"].as_str().unwrap();
        assert!(next_step.contains("retry or skip"), "{next_step}");
        assert_eq!(daemon.delivered(), [LeaseId(1)]);
    }

    #[tokio::test]
    async fn test_resume_of_a_lease_ended_after_its_grant_says_to_acquire_again() {
        let (server, daemon) = server_with(MockDaemon::default());
        server
            .acquire_lease(acquire(ClassParam::GpuHeavy))
            .await
            .unwrap();
        daemon.expire_unused(LeaseId(1));
        let outcome = json(server.acquire_lease(resume(1)).await);
        assert_eq!(outcome["end_cause"], "expired");
        let next_step = outcome["next_step"].as_str().unwrap();
        assert!(next_step.contains("acquire a new lease"), "{next_step}");
        assert!(server.held.ids().is_empty());

        server
            .acquire_lease(acquire(ClassParam::GpuHeavy))
            .await
            .unwrap();
        daemon.stop(LeaseId(2), EndCause::Expired);
        let outcome = json(server.acquire_lease(resume(2)).await);
        let next_step = outcome["next_step"].as_str().unwrap();
        assert!(next_step.contains("do not run the command"), "{next_step}");
    }

    #[tokio::test]
    async fn test_ordinary_acquire_has_no_cause_and_tells_nothing() {
        let (server, daemon) = server_with(MockDaemon::default());
        let outcome = json(server.acquire_lease(acquire(ClassParam::CpuHeavy)).await);
        assert!(outcome.get("end_cause").is_none());
        assert!(daemon.delivered().is_empty());
    }

    #[tokio::test]
    async fn test_release_of_a_killed_lease_reports_the_kill_and_tells_once() {
        let (server, daemon) = server_with(MockDaemon::default());
        server
            .acquire_lease(acquire(ClassParam::CpuHeavy))
            .await
            .unwrap();
        daemon.stop(LeaseId(1), EndCause::Killed);
        let released = json(
            server
                .release_lease(Parameters(ReleaseParams { lease_id: 1 }))
                .await,
        );
        assert_eq!(released["state"], "expired");
        assert_eq!(released["end_cause"], "killed");
        assert!(
            released["note"]
                .as_str()
                .unwrap()
                .starts_with("The user stopped this job")
        );
        assert_eq!(daemon.delivered(), [LeaseId(1)]);
        assert!(server.held.ids().is_empty());
    }

    #[tokio::test]
    async fn test_list_leases_reports_untold_stops_once() {
        let (server, daemon) = server_with(MockDaemon::default());
        daemon.add_lease(lease(1, holder("me"), LeaseState::Active));
        daemon.add_lease(lease(2, holder("me"), LeaseState::Queued));
        daemon.add_lease(lease(3, holder("them"), LeaseState::Active));
        daemon.stop(LeaseId(2), EndCause::Removed);
        daemon.stop(LeaseId(3), EndCause::Killed);

        let listed = json(
            server
                .list_leases(Parameters(ListLeasesParams::default()))
                .await,
        );
        assert_eq!(listed["leases"].as_array().unwrap().len(), 1);
        let stopped = listed["stopped_by_user"].as_array().unwrap();
        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0]["lease_id"], 2);
        assert_eq!(stopped[0]["end_cause"], "removed");
        assert_eq!(stopped[0]["label"], "cargo build");
        assert_eq!(daemon.delivered(), [LeaseId(2)]);

        let again = json(
            server
                .list_leases(Parameters(ListLeasesParams::default()))
                .await,
        );
        assert!(again.get("stopped_by_user").is_none());
    }

    #[tokio::test]
    async fn test_acquire_new_lease_without_label_is_a_tool_error() {
        let (server, _) = server_with(MockDaemon::default());
        let mut params = acquire(ClassParam::CpuHeavy);
        params.0.label = "  ".into();
        let error = server.acquire_lease(params).await.unwrap_err();
        assert!(error.contains("label"), "{error}");
    }

    #[tokio::test]
    async fn test_release_of_a_benchmark_reports_its_noise() {
        let (server, _) = server_with(MockDaemon::default());
        let mut params = acquire(ClassParam::CpuHeavy);
        params.0.kind = KindParam::BenchmarkExclusive;
        server.acquire_lease(params).await.unwrap();
        let released = json(
            server
                .release_lease(Parameters(ReleaseParams { lease_id: 1 }))
                .await,
        );
        assert_eq!(released["state"], "released");
        assert_eq!(released["noise"]["avg_cpu_pct"], 1.5);
        assert_eq!(released["noise"]["quiet_at_start"], true);
    }

    #[tokio::test]
    async fn test_acquire_with_bad_profile_is_a_tool_error() {
        let (server, daemon) = server_with(MockDaemon::default());
        let mut params = acquire(ClassParam::CpuHeavy);
        params.0.profile.cores = Some(4.0);
        let error = server.acquire_lease(params).await.unwrap_err();
        assert!(error.contains("not several"), "{error}");
        assert!(daemon.heartbeats().is_empty() && server.held.ids().is_empty());
    }

    #[tokio::test]
    async fn test_release_lease_stops_holding_it() {
        let (server, daemon) = server_with(MockDaemon::default());
        server
            .acquire_lease(acquire(ClassParam::CpuHeavy))
            .await
            .unwrap();
        let released = json(
            server
                .release_lease(Parameters(ReleaseParams { lease_id: 1 }))
                .await,
        );
        assert_eq!(released["state"], "released");
        assert!(released.get("noise").is_none());
        assert!(server.held.ids().is_empty());
        assert_eq!(daemon.releases(), [LeaseId(1)]);
    }

    #[tokio::test]
    async fn test_list_leases_defaults_to_this_session() {
        let (server, daemon) = server_with(MockDaemon::default());
        daemon.add_lease(lease(1, holder("me"), LeaseState::Active));
        daemon.add_lease(lease(2, holder("them"), LeaseState::Queued));
        daemon.add_lease(lease(3, holder("me"), LeaseState::Released));

        let mine = json(
            server
                .list_leases(Parameters(ListLeasesParams::default()))
                .await,
        );
        assert_eq!(mine["leases"].as_array().unwrap().len(), 1);
        assert_eq!(mine["leases"][0]["id"], 1);
        let everyone = ListLeasesParams { all_sessions: true };
        let all = json(server.list_leases(Parameters(everyone)).await);
        assert_eq!(all["leases"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_unreachable_daemon_is_a_readable_tool_error() {
        let (server, _) = server_with(MockDaemon::unreachable());
        let error = server.system_snapshot().await.unwrap_err();
        assert!(error.contains("not reachable"), "{error}");
    }

    #[tokio::test]
    async fn test_interrupted_acquire_says_how_to_find_or_resume_the_lease() {
        let (server, _) = server_with(MockDaemon::interrupted());
        let error = server
            .acquire_lease(acquire(ClassParam::CpuHeavy))
            .await
            .unwrap_err();
        assert!(error.contains("did not answer"), "{error}");
        assert!(error.contains("call list_leases"), "{error}");

        let resume = Parameters(AcquireParams {
            lease_id: Some(7),
            ..AcquireParams::default()
        });
        let error = server.acquire_lease(resume).await.unwrap_err();
        assert!(error.contains("only lease_id 7 again"), "{error}");
        assert!(server.held.ids().is_empty());
    }

    #[test]
    fn test_get_info_enables_tools_and_gives_instructions() {
        let (server, _) = server_with(MockDaemon::default());
        let info = server.get_info();
        assert!(info.capabilities.tools.is_some());
        assert!(info.instructions.unwrap().contains("acquire_lease"));
        assert_eq!(info.server_info.name, "allot-mcp");
    }

    #[test]
    fn test_tool_router_lists_the_five_tools() {
        let mut names: Vec<String> = AllotServer::<MockDaemon>::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "acquire_lease",
                "can_fit",
                "list_leases",
                "release_lease",
                "system_snapshot"
            ]
        );
    }
}
