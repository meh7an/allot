//! What the hooks tell Claude about the user's kills and removals, each exactly once.
//!
//! The text states facts only. How to react (ask retry or skip, or say the request was
//! cancelled) is in the resource-guard skill, because imperatives injected into a tool result
//! can trip prompt-injection defenses.

use rh_shared::{
    types::{EndCause, SessionKey},
    ui::Notice,
};

use super::HookDaemon;

/// The text for the session's undelivered notices, or `None` when there is nothing new to tell
/// or the daemon cannot be asked. Each notice is marked delivered before it is included, and
/// one another channel already delivered is left out. `current` is the hook's own tool call,
/// and `failed` says whether that call failed.
pub fn deliver(
    daemon: &impl HookDaemon,
    session: &SessionKey,
    current: Option<&str>,
    failed: bool,
) -> Option<String> {
    let notices = daemon.notices(session).ok()?;
    let lines: Vec<String> = notices
        .iter()
        .filter(|notice| {
            daemon
                .mark_delivered(notice.lease_id, session)
                .unwrap_or(false)
        })
        .map(|notice| describe(notice, current, failed))
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// One notice as a sentence for Claude.
pub fn describe(notice: &Notice, current: Option<&str>, failed: bool) -> String {
    let (command, id) = (&notice.command, notice.lease_id.0);
    if notice.cause == EndCause::Removed {
        return format!(
            "The user removed the queued request for `{command}` (lease {id}) from Resources \
             Helper before it started; it did not run."
        );
    }
    let this_call = current.is_some() && notice.tool_use_id.as_deref() == current;
    let mut text = if this_call {
        format!("The user stopped this command (`{command}`, lease {id}) from Resources Helper")
    } else {
        format!("The user stopped `{command}` (lease {id}) from Resources Helper")
    };
    if this_call && failed {
        text.push_str("; the exit code comes from that, not from the command itself");
    }
    text.push_str(". They have not said whether to retry or skip.");
    text
}

#[cfg(test)]
mod tests {
    use rh_shared::types::LeaseId;

    use super::*;
    use crate::hook::test_support::{FakeDaemon, notice};

    #[test]
    fn test_describe_kill_of_another_call_names_command_and_lease() {
        let text = describe(&notice(42, EndCause::Killed, Some("toolu_a")), None, false);
        assert_eq!(
            text,
            "The user stopped `cargo build` (lease 42) from Resources Helper. They have not \
             said whether to retry or skip."
        );
        let other = describe(
            &notice(42, EndCause::Killed, Some("toolu_a")),
            Some("toolu_b"),
            true,
        );
        assert_eq!(other, text);
    }

    #[test]
    fn test_describe_kill_of_the_failed_call_explains_the_exit_code() {
        let text = describe(
            &notice(42, EndCause::Killed, Some("toolu_a")),
            Some("toolu_a"),
            true,
        );
        assert_eq!(
            text,
            "The user stopped this command (`cargo build`, lease 42) from Resources Helper; \
             the exit code comes from that, not from the command itself. They have not said \
             whether to retry or skip."
        );
    }

    #[test]
    fn test_describe_kill_of_a_call_that_succeeded_says_this_command_only() {
        let text = describe(
            &notice(42, EndCause::Killed, Some("toolu_a")),
            Some("toolu_a"),
            false,
        );
        assert!(text.starts_with("The user stopped this command"), "{text}");
        assert!(!text.contains("exit code"), "{text}");
    }

    #[test]
    fn test_describe_kill_without_binding_never_matches_a_call() {
        let text = describe(&notice(42, EndCause::Killed, None), None, true);
        assert!(text.starts_with("The user stopped `cargo build`"), "{text}");
    }

    #[test]
    fn test_describe_removal_says_it_did_not_run() {
        let text = describe(&notice(43, EndCause::Removed, None), None, false);
        assert_eq!(
            text,
            "The user removed the queued request for `cargo build` (lease 43) from Resources \
             Helper before it started; it did not run."
        );
    }

    #[test]
    fn test_deliver_joins_notices_marks_them_and_tells_once() {
        let daemon = FakeDaemon {
            notices: vec![
                notice(42, EndCause::Killed, None),
                notice(43, EndCause::Removed, None),
            ],
            ..FakeDaemon::default()
        };
        let session = SessionKey("s".into());
        let text = deliver(&daemon, &session, None, false).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.lines().next().unwrap().contains("lease 42"));
        assert_eq!(daemon.delivered(), [LeaseId(42), LeaseId(43)]);
        assert_eq!(deliver(&daemon, &session, None, false), None);
    }

    #[test]
    fn test_deliver_leaves_out_notices_another_channel_delivered() {
        let daemon = FakeDaemon {
            notices: vec![
                notice(42, EndCause::Killed, None),
                notice(43, EndCause::Removed, None),
            ],
            taken_elsewhere: vec![LeaseId(42)],
            ..FakeDaemon::default()
        };
        let text = deliver(&daemon, &SessionKey("s".into()), None, false).unwrap();
        assert!(
            !text.contains("lease 42") && text.contains("lease 43"),
            "{text}"
        );
    }

    #[test]
    fn test_deliver_with_the_daemon_down_tells_nothing() {
        let daemon = FakeDaemon {
            notices: vec![notice(42, EndCause::Killed, None)],
            down: true,
            ..FakeDaemon::default()
        };
        assert_eq!(deliver(&daemon, &SessionKey("s".into()), None, false), None);
    }
}
