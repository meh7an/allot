//! `allot hook post-tool`: the PostToolUse and PostToolUseFailure hook for the Bash and
//! PowerShell tools. Tells Claude, once, when the user stopped one of its jobs or removed a
//! queued request from Allot, and after a heavy command that the lease it ran under
//! is still held.
//!
//! A killed command fails (exit code 255), so PostToolUseFailure carries most notices, and the
//! daemon ends the lease before it terminates the tree, so the notice is already there. The
//! hook says nothing when allotd is unavailable: it runs after every call, and the PreToolUse
//! hook already warns for the heavy ones.

use std::{fmt::Write as _, process::ExitCode};

use allot_core::classifier::{Classification, Classifier, Shell};
use allot_shared::{
    config::Config,
    types::{Lease, LeaseState, SessionKey},
};
use serde::Deserialize;

use super::{
    HookDaemon, MCP_SERVER, Outcome, ToolInput, connect, emit, notice, read_stdin, setup, shell,
};

/// The fields of the PostToolUse and PostToolUseFailure payloads the hook reads.
#[derive(Debug, Deserialize)]
struct PostInput {
    session_id: String,
    hook_event_name: String,
    tool_name: String,
    #[serde(default)]
    tool_input: ToolInput,
    #[serde(default)]
    tool_use_id: Option<String>,
}

/// Runs the hook on the payload from stdin.
pub fn post_tool() -> ExitCode {
    let stdin = match read_stdin() {
        Ok(stdin) => stdin,
        Err(error) => {
            return emit(&skipped(format_args!(
                "cannot read the hook input: {error}"
            )));
        }
    };
    let outcome = match setup() {
        Ok((paths, config)) => respond(&stdin, &config, || connect(&paths, &config)),
        Err(error) => skipped(format_args!("cannot load the configuration: {error}")),
    };
    emit(&outcome)
}

/// What to tell Claude after one tool call. `connect` is only called for the two tools and
/// the two events the hook handles.
fn respond<D: HookDaemon>(
    stdin: &str,
    config: &Config,
    connect: impl FnOnce() -> allot_client::Result<D>,
) -> Outcome {
    let input: PostInput = match serde_json::from_str(stdin) {
        Ok(input) => input,
        Err(error) => return skipped(format_args!("unexpected hook input: {error}")),
    };
    let Some(shell) = shell(&input.tool_name) else {
        return Outcome::Pass;
    };
    let (event, failed) = match input.hook_event_name.as_str() {
        "PostToolUse" => ("PostToolUse", false),
        "PostToolUseFailure" => ("PostToolUseFailure", true),
        _ => return Outcome::Pass,
    };
    let Ok(daemon) = connect() else {
        return Outcome::Pass;
    };
    let session = SessionKey(input.session_id.clone());
    let told = notice::deliver(&daemon, &session, input.tool_use_id.as_deref(), failed);
    let held = still_held(&daemon, &session, &input, shell, config);
    let context = match (told, held) {
        (Some(told), Some(held)) => format!("{told}\n{held}"),
        (Some(context), None) | (None, Some(context)) => context,
        (None, None) => return Outcome::Pass,
    };
    Outcome::Context { event, context }
}

/// A note that the active lease this call ran under is still held, after a heavy command that
/// the PreToolUse hook bound to it. A call running in the background has only started, so it
/// gets none.
fn still_held<D: HookDaemon>(
    daemon: &D,
    session: &SessionKey,
    input: &PostInput,
    shell: Shell,
    config: &Config,
) -> Option<String> {
    let tool_use_id = input
        .tool_use_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())?;
    if input.tool_input.run_in_background == Some(true) {
        return None;
    }
    let line = input.tool_input.command.as_deref()?;
    if !matches!(
        Classifier::new(&config.classifier).classify(line, shell),
        Classification::Heavy { .. }
    ) {
        return None;
    }
    let leases = daemon.leases(session).ok()?;
    let lease = leases.iter().find(|lease| {
        lease.state == LeaseState::Active
            && lease
                .binding
                .as_ref()
                .is_some_and(|binding| binding.tool_use_id == tool_use_id)
    })?;
    Some(held_text(lease, config.leases.idle_release_secs))
}

/// The note states facts only; what to do about it is in the resource-guard skill, as for the
/// notices.
fn held_text(lease: &Lease, idle_release_secs: u64) -> String {
    let mut text = format!(
        "This command has finished, and lease {} (`{}`) that covered it is still held.",
        lease.id.0, lease.label
    );
    if idle_release_secs > 0 {
        let wait = if idle_release_secs.is_multiple_of(60) {
            format!("{} min", idle_release_secs / 60)
        } else {
            format!("{idle_release_secs} s")
        };
        // Writing to a String cannot fail.
        let _ = write!(
            text,
            " allotd ends it after {wait} with no command running under it and the session idle."
        );
    }
    text
}

/// A warning that the hook could not run; it never blocks anything.
fn skipped(why: std::fmt::Arguments<'_>) -> Outcome {
    Outcome::Warn(format!("{MCP_SERVER}: notice check skipped, {why}"))
}

#[cfg(test)]
mod tests {
    use allot_shared::types::{EndCause, LeaseBinding, LeaseId};
    use serde_json::Value;

    use super::*;
    use crate::hook::{
        render,
        test_support::{FIXTURE_SESSION, FakeDaemon, cpu, lease, never, notice, unreachable},
    };

    const BASH_FAILURE: &str =
        include_str!("../../fixtures/hook/post_bash_cargo_build_failure.json");
    const POWERSHELL_SUCCESS: &str =
        include_str!("../../fixtures/hook/post_powershell_cargo_test.json");
    const READ_TOOL: &str = include_str!("../../fixtures/hook/read_tool.json");
    const FAILED_CALL: &str = "toolu_00000000000000000000fail";
    const PASSED_CALL: &str = "toolu_00000000000000000000pass";

    /// The hook with the default config.
    fn respond<D: HookDaemon>(
        stdin: &str,
        connect: impl FnOnce() -> allot_client::Result<D>,
    ) -> Outcome {
        super::respond(stdin, &Config::default(), connect)
    }

    /// Active lease `id`, bound to the tool call `tool_use_id`.
    fn bound(id: i64, tool_use_id: &str) -> Lease {
        Lease {
            label: "cargo test --workspace".into(),
            binding: Some(LeaseBinding {
                tool_use_id: tool_use_id.into(),
                bound_at_ms: 1_793_000_000_000,
                root: None,
            }),
            ..lease(id, LeaseState::Active, cpu())
        }
    }

    fn with_input(fixture: &str, field: &str, value: Value) -> String {
        let mut payload: Value = serde_json::from_str(fixture).unwrap();
        payload["tool_input"][field] = value;
        payload.to_string()
    }

    fn context(outcome: Outcome) -> (&'static str, String) {
        match outcome {
            Outcome::Context { event, context } => (event, context),
            other => panic!("expected context, got {other:?}"),
        }
    }

    #[test]
    fn test_respond_failure_of_the_killed_call_says_this_command() {
        let daemon = FakeDaemon {
            notices: vec![notice(42, EndCause::Killed, Some(FAILED_CALL))],
            ..FakeDaemon::default()
        };
        let (event, text) = context(respond(BASH_FAILURE, || Ok(&daemon)));
        assert_eq!(event, "PostToolUseFailure");
        assert!(
            text.starts_with("The user stopped this command (`cargo build`, lease 42)"),
            "{text}"
        );
        assert!(text.contains("the exit code comes from that"), "{text}");
        assert_eq!(daemon.delivered(), [LeaseId(42)]);
    }

    #[test]
    fn test_respond_success_tells_about_another_call_by_name() {
        let daemon = FakeDaemon {
            notices: vec![notice(42, EndCause::Killed, Some(FAILED_CALL))],
            ..FakeDaemon::default()
        };
        let (event, text) = context(respond(POWERSHELL_SUCCESS, || Ok(&daemon)));
        assert_eq!(event, "PostToolUse");
        assert!(
            text.starts_with("The user stopped `cargo build` (lease 42)"),
            "{text}"
        );
    }

    #[test]
    fn test_respond_tells_a_removal_once() {
        let daemon = FakeDaemon {
            notices: vec![notice(43, EndCause::Removed, None)],
            ..FakeDaemon::default()
        };
        let (_, text) = context(respond(POWERSHELL_SUCCESS, || Ok(&daemon)));
        assert!(text.contains("removed the queued request"), "{text}");
        assert_eq!(respond(BASH_FAILURE, || Ok(&daemon)), Outcome::Pass);
    }

    #[test]
    fn test_respond_without_notices_passes() {
        let daemon = FakeDaemon::default();
        assert_eq!(respond(BASH_FAILURE, || Ok(&daemon)), Outcome::Pass);
    }

    #[test]
    fn test_respond_daemon_down_passes_silently() {
        assert_eq!(respond(BASH_FAILURE, unreachable), Outcome::Pass);
        let down = FakeDaemon {
            notices: vec![notice(42, EndCause::Killed, None)],
            down: true,
            ..FakeDaemon::default()
        };
        assert_eq!(respond(BASH_FAILURE, || Ok(&down)), Outcome::Pass);
    }

    #[test]
    fn test_respond_other_tool_or_event_passes_without_daemon() {
        assert_eq!(respond(READ_TOOL, never), Outcome::Pass);
        let mut payload: Value = serde_json::from_str(BASH_FAILURE).unwrap();
        payload["hook_event_name"] = "PreToolUse".into();
        assert_eq!(respond(&payload.to_string(), never), Outcome::Pass);
    }

    #[test]
    fn test_respond_asks_for_the_payload_session() {
        let daemon = FakeDaemon {
            notices: vec![notice(42, EndCause::Killed, None)],
            ..FakeDaemon::default()
        };
        respond(BASH_FAILURE, || Ok(&daemon));
        let payload: Value = serde_json::from_str(BASH_FAILURE).unwrap();
        assert_eq!(payload["session_id"], FIXTURE_SESSION);
        assert_eq!(daemon.delivered(), [LeaseId(42)]);
    }

    #[test]
    fn test_respond_malformed_input_warns() {
        let outcome = respond("not json", never);
        assert!(
            matches!(&outcome, Outcome::Warn(message) if message.contains("notice check skipped")),
            "{outcome:?}"
        );
    }

    #[test]
    fn test_respond_wrongly_typed_fields_warn() {
        let fixture: Value = serde_json::from_str(BASH_FAILURE).unwrap();
        let wrong = [serde_json::json!(42), serde_json::json!(["a"])];
        for field in ["session_id", "hook_event_name", "tool_name", "tool_use_id"] {
            for value in wrong
                .iter()
                .chain((field != "tool_use_id").then_some(&Value::Null))
            {
                let mut payload = fixture.clone();
                payload[field] = value.clone();
                let outcome = respond(&payload.to_string(), never);
                assert!(
                    matches!(&outcome, Outcome::Warn(message) if message.contains("hook input")),
                    "{field} = {value}: {outcome:?}"
                );
            }
        }
    }

    #[test]
    fn test_respond_malformed_payloads_never_panic() {
        for (seed, fixture) in (11..).zip([BASH_FAILURE, POWERSHELL_SUCCESS]) {
            for payload in allot_shared::mutate::mutations(fixture, seed, 500) {
                let daemon = FakeDaemon {
                    notices: vec![notice(42, EndCause::Killed, Some(FAILED_CALL))],
                    ..FakeDaemon::default()
                };
                let outcome = respond(&payload, || Ok(&daemon));
                if let Some(json) = render(&outcome) {
                    serde_json::from_str::<Value>(&json).unwrap();
                }
            }
        }
    }

    #[test]
    fn test_respond_after_a_bound_heavy_call_says_the_lease_is_still_held() {
        let daemon = FakeDaemon {
            leases: vec![bound(7, PASSED_CALL)],
            ..FakeDaemon::default()
        };
        let (event, text) = context(respond(POWERSHELL_SUCCESS, || Ok(&daemon)));
        assert_eq!(event, "PostToolUse");
        assert_eq!(
            text,
            "This command has finished, and lease 7 (`cargo test --workspace`) that covered it \
             is still held. allotd ends it after 3 min with no command running under it and the \
             session idle."
        );
        assert_eq!(daemon.asked(), [FIXTURE_SESSION]);
    }

    #[test]
    fn test_respond_still_held_after_a_failed_call_too() {
        let daemon = FakeDaemon {
            leases: vec![bound(7, FAILED_CALL)],
            ..FakeDaemon::default()
        };
        let (event, text) = context(respond(BASH_FAILURE, || Ok(&daemon)));
        assert_eq!(event, "PostToolUseFailure");
        assert!(text.contains("lease 7"), "{text}");
    }

    #[test]
    fn test_respond_still_held_follows_a_notice() {
        let daemon = FakeDaemon {
            leases: vec![bound(7, PASSED_CALL)],
            notices: vec![notice(42, EndCause::Killed, Some(FAILED_CALL))],
            ..FakeDaemon::default()
        };
        let (_, text) = context(respond(POWERSHELL_SUCCESS, || Ok(&daemon)));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].starts_with("The user stopped"), "{text}");
        assert!(lines[1].starts_with("This command has finished"), "{text}");
    }

    #[test]
    fn test_respond_still_held_names_the_configured_wait() {
        let daemon = FakeDaemon {
            leases: vec![bound(7, PASSED_CALL)],
            ..FakeDaemon::default()
        };
        let mut config = Config::default();
        config.leases.idle_release_secs = 0;
        let (_, text) = context(super::respond(POWERSHELL_SUCCESS, &config, || Ok(&daemon)));
        assert!(text.ends_with("that covered it is still held."), "{text}");
        config.leases.idle_release_secs = 90;
        let (_, text) = context(super::respond(POWERSHELL_SUCCESS, &config, || Ok(&daemon)));
        assert!(text.contains("after 90 s with"), "{text}");
    }

    #[test]
    fn test_respond_not_held_for_other_calls_or_ended_leases() {
        let ended = Lease {
            state: LeaseState::Expired,
            ..bound(8, PASSED_CALL)
        };
        let daemon = FakeDaemon {
            leases: vec![
                bound(7, FAILED_CALL),
                ended,
                lease(9, LeaseState::Active, cpu()),
            ],
            ..FakeDaemon::default()
        };
        assert_eq!(respond(POWERSHELL_SUCCESS, || Ok(&daemon)), Outcome::Pass);
    }

    #[test]
    fn test_respond_not_held_while_the_call_runs_in_the_background() {
        let daemon = FakeDaemon {
            leases: vec![bound(7, PASSED_CALL)],
            ..FakeDaemon::default()
        };
        let background = with_input(POWERSHELL_SUCCESS, "run_in_background", true.into());
        assert_eq!(respond(&background, || Ok(&daemon)), Outcome::Pass);
        assert!(daemon.asked().is_empty());
        let foreground = with_input(POWERSHELL_SUCCESS, "run_in_background", false.into());
        assert!(matches!(
            respond(&foreground, || Ok(&daemon)),
            Outcome::Context { .. }
        ));
    }

    #[test]
    fn test_respond_light_calls_never_ask_for_leases() {
        let daemon = FakeDaemon {
            leases: vec![bound(7, PASSED_CALL)],
            ..FakeDaemon::default()
        };
        let light = with_input(POWERSHELL_SUCCESS, "command", "git status".into());
        assert_eq!(respond(&light, || Ok(&daemon)), Outcome::Pass);
        assert!(daemon.asked().is_empty());
    }
}
