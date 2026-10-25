//! `allot hook ...`: the Claude Code hooks for the Bash and PowerShell tools.
//!
//! `pre-tool` (PreToolUse) denies heavy commands the session holds no covering active lease
//! for, and binds the covering lease to the tool call it lets through so a kill can find the
//! command's processes. `post-tool` (PostToolUse and PostToolUseFailure) tells Claude once
//! when the user stopped one of its jobs or removed a queued request, and after a heavy
//! command that its lease is still held.
//!
//! Exit code 2 from a hook blocks the tool call, and clap uses 2 for argument errors, so the
//! hooks always exit 0 and deny only through their JSON output. Light commands never touch
//! the network in `pre-tool`, which keeps them under the 50 ms budget.

mod notice;
mod post_tool;
#[cfg(test)]
mod test_support;

use std::{
    ffi::OsStr,
    fmt::Write as _,
    io::{self, Read},
    process::ExitCode,
    time::Duration,
};

use allot_client::{BlockingClient, ClientOptions};
use allot_core::classifier::{Classification, Classifier, Shell, describe_rule};
use allot_shared::{
    api::{BindRequest, LeaseQuery},
    config::Config,
    paths::Paths,
    types::{JobClass, Lease, LeaseId, LeaseKind, LeaseState, SessionKey},
    ui::Notice,
};
use serde::{Deserialize, Serialize};

pub use post_tool::post_tool;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const MCP_SERVER: &str = "allot";

/// The fields of the PreToolUse payload the hook reads; the rest are ignored.
#[derive(Debug, Deserialize)]
struct HookInput {
    session_id: String,
    tool_name: String,
    #[serde(default)]
    tool_input: ToolInput,
    #[serde(default)]
    tool_use_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ToolInput {
    command: Option<String>,
    /// Set when the command keeps running after the call returns; `post-tool` reads it.
    #[serde(default)]
    run_in_background: Option<bool>,
}

/// What a hook tells Claude Code.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// No output: the normal permission flow decides.
    Pass,
    /// Like [`Outcome::Pass`], and Claude is told `context` with the result of hook event
    /// `event`.
    Context {
        event: &'static str,
        context: String,
    },
    /// Like [`Outcome::Pass`], and the user is shown why the check was skipped.
    Warn(String),
    /// The call is blocked; the reason goes to Claude.
    Deny(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookOutput<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    hook_specific_output: Option<SpecificOutput<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_message: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpecificOutput<'a> {
    hook_event_name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_decision: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_decision_reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_context: Option<&'a str>,
}

/// The daemon calls the hooks make, behind a trait so tests can use a fake.
trait HookDaemon {
    /// See [`BlockingClient::leases`], for one session.
    fn leases(&self, session: &SessionKey) -> allot_client::Result<Vec<Lease>>;
    /// See [`BlockingClient::bind`].
    fn bind(
        &self,
        id: LeaseId,
        session: &SessionKey,
        request: &BindRequest,
    ) -> allot_client::Result<()>;
    /// See [`BlockingClient::notices`].
    fn notices(&self, session: &SessionKey) -> allot_client::Result<Vec<Notice>>;
    /// See [`BlockingClient::mark_delivered`].
    fn mark_delivered(&self, id: LeaseId, session: &SessionKey) -> allot_client::Result<bool>;
}

impl HookDaemon for BlockingClient {
    fn leases(&self, session: &SessionKey) -> allot_client::Result<Vec<Lease>> {
        BlockingClient::leases(
            self,
            &LeaseQuery {
                session: Some(session.clone()),
                state: None,
            },
        )
    }

    fn bind(
        &self,
        id: LeaseId,
        session: &SessionKey,
        request: &BindRequest,
    ) -> allot_client::Result<()> {
        BlockingClient::bind(self, id, session, request)
    }

    fn notices(&self, session: &SessionKey) -> allot_client::Result<Vec<Notice>> {
        BlockingClient::notices(self, session)
    }

    fn mark_delivered(&self, id: LeaseId, session: &SessionKey) -> allot_client::Result<bool> {
        BlockingClient::mark_delivered(self, id, session)
    }
}

/// Whether the command line is `allot hook ...`, so argument errors must not exit 2.
pub fn invoked() -> bool {
    std::env::args_os().nth(1).as_deref() == Some(OsStr::new("hook"))
}

/// Reports an argument error of `allot hook` as a warning and lets the tool call through.
pub fn argument_error(error: &clap::Error) -> ExitCode {
    let first_line = error.to_string();
    let first_line = first_line.lines().next().unwrap_or_default();
    emit(&Outcome::Warn(format!(
        "{MCP_SERVER}: lease check skipped, the hook command is wrong ({first_line})"
    )))
}

/// Runs the PreToolUse check on the payload from stdin.
pub fn pre_tool() -> ExitCode {
    let stdin = match read_stdin() {
        Ok(stdin) => stdin,
        Err(error) => {
            return emit(&skipped(format_args!(
                "cannot read the hook input: {error}"
            )));
        }
    };
    let outcome = match setup() {
        Ok((paths, config)) => evaluate(&stdin, &config, || connect(&paths, &config)),
        Err(error) => skipped(format_args!("cannot load the configuration: {error}")),
    };
    emit(&outcome)
}

fn read_stdin() -> io::Result<String> {
    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    Ok(stdin)
}

fn setup() -> allot_shared::Result<(Paths, Config)> {
    let paths = Paths::from_env()?;
    let config = Config::load(&paths.config_file())?;
    Ok((paths, config))
}

/// A client with the hooks' short timeouts; it never starts allotd.
fn connect(paths: &Paths, config: &Config) -> allot_client::Result<BlockingClient> {
    BlockingClient::new(ClientOptions {
        connect_timeout: CONNECT_TIMEOUT,
        request_timeout: REQUEST_TIMEOUT,
        ..ClientOptions::local(paths, config, false)
    })
}

/// The shell a tool runs commands in, for the two tools the hooks handle.
fn shell(tool_name: &str) -> Option<Shell> {
    match tool_name {
        "Bash" => Some(Shell::Bash),
        "PowerShell" => Some(Shell::PowerShell),
        _ => None,
    }
}

/// Decides on one payload. `connect` is only called for heavy commands.
fn evaluate<D: HookDaemon>(
    stdin: &str,
    config: &Config,
    connect: impl FnOnce() -> allot_client::Result<D>,
) -> Outcome {
    let input: HookInput = match serde_json::from_str(stdin) {
        Ok(input) => input,
        Err(error) => return skipped(format_args!("unexpected hook input: {error}")),
    };
    let Some(shell) = shell(&input.tool_name) else {
        return Outcome::Pass;
    };
    let Some(line) = input.tool_input.command else {
        return Outcome::Pass;
    };
    let (class, rule, command) = match Classifier::new(&config.classifier).classify(&line, shell) {
        Classification::Heavy {
            class,
            matched_rule,
            command,
        } => (class, describe_rule(&matched_rule), command),
        _ => return Outcome::Pass,
    };
    let session = SessionKey(input.session_id);
    let found = connect().and_then(|daemon| {
        let leases = daemon.leases(&session)?;
        Ok((daemon, leases))
    });
    let (daemon, leases) = match found {
        Ok(found) => found,
        Err(error) if config.hook.fail_open => {
            return skipped(format_args!(
                "allotd is unavailable, `{command}` runs unchecked: {error}"
            ));
        }
        Err(error) => {
            return Outcome::Deny(format!(
                "Blocked by {MCP_SERVER}: `{command}` is {} work, allotd is unavailable ({error}), \
                 and hook.fail_open is off. Ask the user to start it with `allot daemon start`, \
                 then retry.",
                class.as_str()
            ));
        }
    };
    let decision = judge(class, &rule, &command, &leases);
    let tool_use_id = input.tool_use_id.filter(|id| !id.trim().is_empty());
    if let (Ok(&id), Some(tool_use_id)) = (decision.as_ref(), tool_use_id) {
        let request = BindRequest {
            tool_use_id,
            command: Some(command),
        };
        // A failed bind only means a kill finds no processes; the command still runs.
        let _ = daemon.bind(id, &session, &request);
    }
    let told = notice::deliver(&daemon, &session, None, false);
    match (decision, told) {
        (Ok(_), None) => Outcome::Pass,
        (Ok(_), Some(context)) => Outcome::Context {
            event: "PreToolUse",
            context,
        },
        (Err(reason), None) => Outcome::Deny(reason),
        (Err(reason), Some(context)) => Outcome::Deny(format!("{context}\n\n{reason}")),
    }
}

/// The decision for a heavy `command` of `class`, given every lease of its session: the
/// covering active lease to bind the call to (the newest granted one), or the deny reason.
fn judge(class: JobClass, rule: &str, command: &str, leases: &[Lease]) -> Result<LeaseId, String> {
    let in_state = |state| leases.iter().filter(move |lease| lease.state == state);
    if let Some(lease) = in_state(LeaseState::Active)
        .filter(|lease| covers(lease, class))
        .max_by_key(|lease| (lease.granted_at_ms, lease.id.0))
    {
        return Ok(lease.id);
    }
    let name = class.as_str();
    if let Some(queued) = in_state(LeaseState::Queued).find(|lease| covers(lease, class)) {
        return Err(format!(
            "Blocked by {MCP_SERVER}: `{command}` is {name} work and lease {} (\"{}\") that \
             covers it is still queued. Keep waiting on it (acquire_lease with only lease_id \
             {}) until it is active, then retry. Do not acquire another lease for it.",
            queued.id.0, queued.label, queued.id.0
        ));
    }
    let mut reason = format!(
        "Blocked by {MCP_SERVER}: `{command}` is {name} work (rule `{rule}`) and this session \
         holds no active lease that covers it. Call acquire_lease from the {MCP_SERVER} MCP \
         server with a profile sized to this run (class \"{name}\" at most; a preset or custom \
         profile when it needs less) and the command as label, run the command once the lease \
         is active, then call release_lease."
    );
    for lease in in_state(LeaseState::Active) {
        // Writing to a String cannot fail.
        let _ = write!(
            reason,
            " Active lease {} (\"{}\") does not cover {name} work.",
            lease.id.0, lease.label
        );
    }
    Err(reason)
}

/// Whether `lease` reserves what a command of `class` uses: cores for CPU work, GPU time or
/// VRAM for GPU work. A benchmark lease has the machine to itself and covers everything.
fn covers(lease: &Lease, class: JobClass) -> bool {
    if lease.kind == LeaseKind::BenchmarkExclusive {
        return true;
    }
    let (cpu, gpu) = match class {
        JobClass::Light => (false, false),
        JobClass::CpuHeavy => (true, false),
        JobClass::GpuHeavy => (false, true),
        _ => (true, true),
    };
    let profile = &lease.profile;
    (!cpu || profile.cores > 0.0) && (!gpu || profile.gpu_pct > 0.0 || profile.vram_mb > 0)
}

/// A warning that the check was skipped; the hook fails open on its own errors.
fn skipped(why: std::fmt::Arguments<'_>) -> Outcome {
    Outcome::Warn(format!("{MCP_SERVER}: lease check skipped, {why}"))
}

/// The hook's stdout for `outcome`; `None` for [`Outcome::Pass`].
fn render(outcome: &Outcome) -> Option<String> {
    let output = match outcome {
        Outcome::Pass => return None,
        Outcome::Context { event, context } => HookOutput {
            hook_specific_output: Some(SpecificOutput {
                hook_event_name: event,
                permission_decision: None,
                permission_decision_reason: None,
                additional_context: Some(context),
            }),
            system_message: None,
        },
        Outcome::Warn(message) => HookOutput {
            hook_specific_output: None,
            system_message: Some(message),
        },
        Outcome::Deny(reason) => HookOutput {
            hook_specific_output: Some(SpecificOutput {
                hook_event_name: "PreToolUse",
                permission_decision: Some("deny"),
                permission_decision_reason: Some(reason),
                additional_context: None,
            }),
            system_message: None,
        },
    };
    // Plain structs of strings always serialize.
    serde_json::to_string(&output).ok()
}

fn emit(outcome: &Outcome) -> ExitCode {
    if let Some(json) = render(outcome) {
        println!("{json}");
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use allot_shared::types::EndCause;
    use serde_json::Value;

    use super::{test_support::*, *};

    const BASH_CARGO_BUILD: &str = include_str!("../fixtures/hook/bash_cargo_build.json");
    const BASH_LIGHT: &str = include_str!("../fixtures/hook/bash_light.json");
    const BASH_HELP: &str = include_str!("../fixtures/hook/bash_cargo_build_help.json");
    const BASH_TRAIN: &str = include_str!("../fixtures/hook/bash_python_train.json");
    const POWERSHELL_CARGO_TEST: &str = include_str!("../fixtures/hook/powershell_cargo_test.json");
    const READ_TOOL: &str = include_str!("../fixtures/hook/read_tool.json");
    const FIXTURE_TOOL_USE: &str = "toolu_00000000000000000000test";

    fn with(leases: Vec<Lease>) -> impl FnOnce() -> allot_client::Result<FakeDaemon> {
        move || {
            Ok(FakeDaemon {
                leases,
                ..FakeDaemon::default()
            })
        }
    }

    fn denial(outcome: Outcome) -> String {
        match outcome {
            Outcome::Deny(reason) => reason,
            other => panic!("expected a deny, got {other:?}"),
        }
    }

    #[test]
    fn test_evaluate_light_command_passes_without_daemon() {
        assert_eq!(
            evaluate(BASH_LIGHT, &Config::default(), never),
            Outcome::Pass
        );
    }

    #[test]
    fn test_evaluate_light_flag_passes_without_daemon() {
        assert_eq!(
            evaluate(BASH_HELP, &Config::default(), never),
            Outcome::Pass
        );
    }

    #[test]
    fn test_evaluate_other_tool_passes_without_daemon() {
        assert_eq!(
            evaluate(READ_TOOL, &Config::default(), never),
            Outcome::Pass
        );
    }

    #[test]
    fn test_evaluate_heavy_asks_daemon_for_the_payload_session() {
        let daemon = FakeDaemon::default();
        evaluate(BASH_CARGO_BUILD, &Config::default(), || Ok(&daemon));
        assert_eq!(daemon.asked(), [FIXTURE_SESSION]);
    }

    #[test]
    fn test_evaluate_heavy_without_lease_denies_with_acquire_guidance() {
        let reason = denial(evaluate(BASH_CARGO_BUILD, &Config::default(), with(vec![])));
        assert!(reason.contains("`cargo build --release`"), "{reason}");
        assert!(reason.contains("class \"cpu_heavy\""), "{reason}");
        assert!(
            reason.contains("cargo build|test|run|bench -> cpu_heavy"),
            "{reason}"
        );
        assert!(reason.contains("acquire_lease") && reason.contains("release_lease"));
    }

    #[test]
    fn test_evaluate_heavy_with_covering_active_lease_passes() {
        let leases = vec![lease(1, LeaseState::Active, cpu())];
        let outcome = evaluate(BASH_CARGO_BUILD, &Config::default(), with(leases));
        assert_eq!(outcome, Outcome::Pass);
    }

    #[test]
    fn test_evaluate_powershell_heavy_with_covering_lease_passes() {
        let leases = vec![lease(1, LeaseState::Active, cpu())];
        let outcome = evaluate(POWERSHELL_CARGO_TEST, &Config::default(), with(leases));
        assert_eq!(outcome, Outcome::Pass);
    }

    #[test]
    fn test_evaluate_powershell_heavy_without_lease_denies() {
        let outcome = evaluate(POWERSHELL_CARGO_TEST, &Config::default(), with(vec![]));
        assert!(denial(outcome).contains("`cargo test --workspace`"));
    }

    #[test]
    fn test_evaluate_ended_leases_do_not_cover() {
        let leases = vec![
            lease(1, LeaseState::Released, cpu()),
            lease(2, LeaseState::Expired, cpu()),
        ];
        denial(evaluate(BASH_CARGO_BUILD, &Config::default(), with(leases)));
    }

    #[test]
    fn test_evaluate_gpu_command_with_cpu_lease_denies_and_names_it() {
        let leases = vec![lease(3, LeaseState::Active, cpu())];
        let reason = denial(evaluate(BASH_TRAIN, &Config::default(), with(leases)));
        assert!(reason.contains("class \"gpu_heavy\""), "{reason}");
        assert!(reason.contains("Active lease 3 (\"job 3\") does not cover gpu_heavy"));
    }

    #[test]
    fn test_evaluate_custom_profile_with_vram_covers_gpu_command() {
        let leases = vec![lease(4, LeaseState::Active, gpu())];
        let outcome = evaluate(BASH_TRAIN, &Config::default(), with(leases));
        assert_eq!(outcome, Outcome::Pass);
    }

    #[test]
    fn test_evaluate_gpu_only_lease_does_not_cover_cpu_command() {
        let leases = vec![lease(4, LeaseState::Active, gpu())];
        denial(evaluate(BASH_CARGO_BUILD, &Config::default(), with(leases)));
    }

    #[test]
    fn test_evaluate_benchmark_lease_covers_everything() {
        let mut benchmark = lease(5, LeaseState::Active, gpu());
        benchmark.kind = LeaseKind::BenchmarkExclusive;
        benchmark.profile.vram_mb = 0;
        let outcome = evaluate(BASH_CARGO_BUILD, &Config::default(), with(vec![benchmark]));
        assert_eq!(outcome, Outcome::Pass);
    }

    #[test]
    fn test_evaluate_queued_covering_lease_says_wait() {
        let leases = vec![lease(6, LeaseState::Queued, cpu())];
        let reason = denial(evaluate(BASH_CARGO_BUILD, &Config::default(), with(leases)));
        assert!(reason.contains("lease 6 (\"job 6\")") && reason.contains("still queued"));
        assert!(
            reason.contains("acquire_lease with only lease_id 6"),
            "{reason}"
        );
        assert!(!reason.contains("Call acquire_lease"), "{reason}");
    }

    #[test]
    fn test_evaluate_pass_binds_the_covering_lease_to_the_tool_call() {
        let daemon = FakeDaemon {
            leases: vec![lease(1, LeaseState::Active, cpu())],
            ..FakeDaemon::default()
        };
        evaluate(BASH_CARGO_BUILD, &Config::default(), || Ok(&daemon));
        let binds = daemon.binds();
        assert_eq!(binds.len(), 1);
        let (id, request) = &binds[0];
        assert_eq!(*id, LeaseId(1));
        assert_eq!(request.tool_use_id, FIXTURE_TOOL_USE);
        assert_eq!(request.command.as_deref(), Some("cargo build --release"));
    }

    #[test]
    fn test_evaluate_binds_the_newest_granted_of_several_covering_leases() {
        let mut older = lease(7, LeaseState::Active, cpu());
        older.granted_at_ms = Some(1_000);
        let mut newer = lease(3, LeaseState::Active, cpu());
        newer.granted_at_ms = Some(2_000);
        let daemon = FakeDaemon {
            leases: vec![older, newer, lease(9, LeaseState::Active, gpu())],
            ..FakeDaemon::default()
        };
        evaluate(BASH_CARGO_BUILD, &Config::default(), || Ok(&daemon));
        assert_eq!(daemon.binds()[0].0, LeaseId(3));
    }

    #[test]
    fn test_evaluate_deny_binds_nothing() {
        let daemon = FakeDaemon {
            leases: vec![lease(6, LeaseState::Queued, cpu())],
            ..FakeDaemon::default()
        };
        denial(evaluate(BASH_CARGO_BUILD, &Config::default(), || {
            Ok(&daemon)
        }));
        assert!(daemon.binds().is_empty());
    }

    #[test]
    fn test_evaluate_without_tool_use_id_passes_without_binding() {
        let mut payload: Value = serde_json::from_str(BASH_CARGO_BUILD).unwrap();
        for tool_use_id in [Value::Null, Value::from(" ")] {
            payload["tool_use_id"] = tool_use_id;
            let daemon = FakeDaemon {
                leases: vec![lease(1, LeaseState::Active, cpu())],
                ..FakeDaemon::default()
            };
            let outcome = evaluate(&payload.to_string(), &Config::default(), || Ok(&daemon));
            assert_eq!(outcome, Outcome::Pass);
            assert!(daemon.binds().is_empty());
        }
    }

    #[test]
    fn test_evaluate_failed_bind_still_passes() {
        let daemon = FakeDaemon {
            leases: vec![lease(1, LeaseState::Active, cpu())],
            fail_bind: true,
            ..FakeDaemon::default()
        };
        let outcome = evaluate(BASH_CARGO_BUILD, &Config::default(), || Ok(&daemon));
        assert_eq!(outcome, Outcome::Pass);
    }

    #[test]
    fn test_evaluate_pass_delivers_a_pending_notice_once() {
        let daemon = FakeDaemon {
            leases: vec![lease(1, LeaseState::Active, cpu())],
            notices: vec![notice(42, EndCause::Killed, Some("toolu_old"))],
            ..FakeDaemon::default()
        };
        let outcome = evaluate(BASH_CARGO_BUILD, &Config::default(), || Ok(&daemon));
        let Outcome::Context { event, context } = outcome else {
            panic!("expected context, got {outcome:?}");
        };
        assert_eq!(event, "PreToolUse");
        assert!(
            context.starts_with("The user stopped `cargo build` (lease 42)"),
            "{context}"
        );
        assert_eq!(daemon.delivered(), [LeaseId(42)]);

        let again = evaluate(BASH_CARGO_BUILD, &Config::default(), || Ok(&daemon));
        assert_eq!(again, Outcome::Pass);
    }

    #[test]
    fn test_evaluate_deny_carries_a_pending_notice_before_the_reason() {
        let daemon = FakeDaemon {
            notices: vec![notice(43, EndCause::Removed, None)],
            ..FakeDaemon::default()
        };
        let reason = denial(evaluate(BASH_CARGO_BUILD, &Config::default(), || {
            Ok(&daemon)
        }));
        let (told, blocked) = reason.split_once("\n\n").unwrap();
        assert!(told.contains("removed the queued request"), "{told}");
        assert!(blocked.starts_with("Blocked by allot"), "{blocked}");
    }

    #[test]
    fn test_evaluate_daemon_down_fails_open_with_warning() {
        let outcome = evaluate(BASH_CARGO_BUILD, &Config::default(), unreachable);
        let Outcome::Warn(message) = outcome else {
            panic!("expected a warning, got {outcome:?}");
        };
        assert!(message.contains("allotd is unavailable") && message.contains("cargo build"));
    }

    #[test]
    fn test_evaluate_daemon_down_with_fail_open_off_denies() {
        let mut config = Config::default();
        config.hook.fail_open = false;
        let reason = denial(evaluate(BASH_CARGO_BUILD, &config, unreachable));
        assert!(reason.contains("allot daemon start"), "{reason}");
    }

    #[test]
    fn test_evaluate_malformed_input_warns() {
        let outcome = evaluate("not json", &Config::default(), never);
        assert!(matches!(outcome, Outcome::Warn(message) if message.contains("hook input")));
    }

    #[test]
    fn test_render_deny_uses_permission_decision() {
        let json: Value =
            serde_json::from_str(&render(&Outcome::Deny("why".into())).unwrap()).unwrap();
        let decision = &json["hookSpecificOutput"];
        assert_eq!(decision["hookEventName"], "PreToolUse");
        assert_eq!(decision["permissionDecision"], "deny");
        assert_eq!(decision["permissionDecisionReason"], "why");
        assert!(decision.get("additionalContext").is_none());
        assert!(json.get("systemMessage").is_none());
    }

    #[test]
    fn test_render_context_is_additional_context_only() {
        let outcome = Outcome::Context {
            event: "PostToolUseFailure",
            context: "fact".into(),
        };
        let json: Value = serde_json::from_str(&render(&outcome).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PostToolUseFailure",
                    "additionalContext": "fact",
                },
            })
        );
    }

    #[test]
    fn test_render_warn_is_system_message_only() {
        let json: Value =
            serde_json::from_str(&render(&Outcome::Warn("hm".into())).unwrap()).unwrap();
        assert_eq!(json, serde_json::json!({ "systemMessage": "hm" }));
    }

    #[test]
    fn test_render_pass_prints_nothing() {
        assert_eq!(render(&Outcome::Pass), None);
    }

    /// Whatever the payload, the hook decides without panicking and prints valid JSON or
    /// nothing; heavy commands only reach the daemon lookup.
    fn assert_decides_cleanly(payload: &str) {
        let outcome = evaluate(payload, &Config::default(), unreachable);
        if let Some(json) = render(&outcome) {
            serde_json::from_str::<Value>(&json).unwrap();
        }
    }

    #[test]
    fn test_evaluate_malformed_payloads_never_panic() {
        let fixtures = [
            BASH_CARGO_BUILD,
            BASH_LIGHT,
            BASH_HELP,
            BASH_TRAIN,
            POWERSHELL_CARGO_TEST,
            READ_TOOL,
        ];
        for (seed, fixture) in (1..).zip(fixtures) {
            for payload in allot_shared::mutate::mutations(fixture, seed, 500) {
                assert_decides_cleanly(&payload);
            }
        }
    }

    #[test]
    fn test_evaluate_wrongly_typed_fields_warn() {
        let fixture: Value = serde_json::from_str(BASH_CARGO_BUILD).unwrap();
        let wrong = [
            serde_json::json!(42),
            serde_json::json!(["cargo", "build"]),
            serde_json::json!({ "command": 1 }),
        ];
        for field in ["session_id", "tool_name", "tool_input", "tool_use_id"] {
            for value in wrong.iter().chain(
                // Only the optional field may be null.
                (field != "tool_use_id").then_some(&Value::Null),
            ) {
                let mut payload = fixture.clone();
                payload[field] = value.clone();
                let outcome = evaluate(&payload.to_string(), &Config::default(), never);
                assert!(
                    matches!(&outcome, Outcome::Warn(message) if message.contains("hook input")),
                    "{field} = {value}: {outcome:?}"
                );
            }
        }
        let mut payload = fixture;
        payload["tool_input"]["command"] = serde_json::json!(["cargo", "build"]);
        let outcome = evaluate(&payload.to_string(), &Config::default(), never);
        assert!(matches!(outcome, Outcome::Warn(_)), "{outcome:?}");
    }
}
