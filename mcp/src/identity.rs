//! Which Claude session this server belongs to.
//!
//! Claude Code starts each stdio MCP server as a direct child of the session's `claude`
//! (`claude.exe` on Windows) and sets `CLAUDE_CODE_SESSION_ID` in its environment, on Linux
//! and macOS too, so the parent process is the lease holder and the variable is the session
//! key. Unlike hooks and Bash commands, MCP servers get no `CLAUDE_PID`.

use std::env;

use allot_core::ancestry::ProcessNode;
use allot_shared::types::{Holder, SessionKey};
use anyhow::Context;

const SESSION_ID_VAR: &str = "CLAUDE_CODE_SESSION_ID";

/// The holder for this server's leases: its parent process, keyed by the session ID.
///
/// # Errors
///
/// Fails when the parent process cannot be found.
pub fn current_holder() -> anyhow::Result<Holder> {
    let chain = allot_sampler::current_ancestry()?;
    let parent = chain
        .get(1)
        .context("allot-mcp has no visible parent process")?;
    Ok(holder_for(parent, env::var(SESSION_ID_VAR).ok()))
}

fn holder_for(parent: &ProcessNode, session_id: Option<String>) -> Holder {
    let session = session_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| {
            eprintln!(
                "allot-mcp: {SESSION_ID_VAR} is not set; using the parent PID as the session key"
            );
            format!("pid-{}", parent.pid)
        });
    Holder {
        session: SessionKey(session),
        pid: parent.pid,
        pid_start_time: parent.start_time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude() -> ProcessNode {
        ProcessNode {
            pid: 60_012,
            parent_pid: Some(1),
            name: "claude.exe".into(),
            exe: None,
            start_time: 1_793_000_000,
        }
    }

    #[test]
    fn test_holder_for_uses_the_session_id_and_parent() {
        let holder = holder_for(&claude(), Some("3f2a9c1e".into()));
        assert_eq!(holder.session, SessionKey("3f2a9c1e".into()));
        assert_eq!((holder.pid, holder.pid_start_time), (60_012, 1_793_000_000));
    }

    #[test]
    fn test_holder_for_without_session_id_falls_back_to_parent_pid() {
        assert_eq!(holder_for(&claude(), None).session.0, "pid-60012");
        assert_eq!(
            holder_for(&claude(), Some("  ".into())).session.0,
            "pid-60012"
        );
    }
}
