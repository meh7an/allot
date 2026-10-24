//! `rh hook post-tool`: the PostToolUse and PostToolUseFailure hook for the Bash and
//! PowerShell tools. Tells Claude, once, when the user stopped one of its jobs or removed a
//! queued request from Resources Helper.
//!
//! A killed command fails (exit code 255), so PostToolUseFailure carries most notices, and the
//! daemon ends the lease before it terminates the tree, so the notice is already there. The
//! hook says nothing when rhd is unavailable: it runs after every call, and the PreToolUse
//! hook already warns for the heavy ones.

use std::process::ExitCode;

use rh_shared::types::SessionKey;
use serde::Deserialize;

use super::{HookDaemon, MCP_SERVER, Outcome, connect, emit, notice, read_stdin, setup, shell};

/// The fields of the PostToolUse and PostToolUseFailure payloads the hook reads.
#[derive(Debug, Deserialize)]
struct PostInput {
    session_id: String,
    hook_event_name: String,
    tool_name: String,
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
        Ok((paths, config)) => respond(&stdin, || connect(&paths, &config)),
        Err(error) => skipped(format_args!("cannot load the configuration: {error}")),
    };
    emit(&outcome)
}

/// What to tell Claude after one tool call. `connect` is only called for the two tools and
/// the two events the hook handles.
fn respond<D: HookDaemon>(stdin: &str, connect: impl FnOnce() -> rh_client::Result<D>) -> Outcome {
    let input: PostInput = match serde_json::from_str(stdin) {
        Ok(input) => input,
        Err(error) => return skipped(format_args!("unexpected hook input: {error}")),
    };
    if shell(&input.tool_name).is_none() {
        return Outcome::Pass;
    }
    let (event, failed) = match input.hook_event_name.as_str() {
        "PostToolUse" => ("PostToolUse", false),
        "PostToolUseFailure" => ("PostToolUseFailure", true),
        _ => return Outcome::Pass,
    };
    let Ok(daemon) = connect() else {
        return Outcome::Pass;
    };
    let session = SessionKey(input.session_id);
    match notice::deliver(&daemon, &session, input.tool_use_id.as_deref(), failed) {
        Some(context) => Outcome::Context { event, context },
        None => Outcome::Pass,
    }
}

/// A warning that the hook could not run; it never blocks anything.
fn skipped(why: std::fmt::Arguments<'_>) -> Outcome {
    Outcome::Warn(format!("{MCP_SERVER}: notice check skipped, {why}"))
}

#[cfg(test)]
mod tests {
    use rh_shared::types::{EndCause, LeaseId};
    use serde_json::Value;

    use super::*;
    use crate::hook::{
        render,
        test_support::{FIXTURE_SESSION, FakeDaemon, never, notice, unreachable},
    };

    const BASH_FAILURE: &str =
        include_str!("../../fixtures/hook/post_bash_cargo_build_failure.json");
    const POWERSHELL_SUCCESS: &str =
        include_str!("../../fixtures/hook/post_powershell_cargo_test.json");
    const READ_TOOL: &str = include_str!("../../fixtures/hook/read_tool.json");
    const FAILED_CALL: &str = "toolu_00000000000000000000fail";

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
            for payload in rh_shared::mutate::mutations(fixture, seed, 500) {
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
}
