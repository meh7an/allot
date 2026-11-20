//! `allot daemon start|stop|status`.

use std::{
    thread,
    time::{Duration, Instant},
};

use allot_client::{BlockingClient, DaemonStart};
use allot_shared::stop::{StopRecord, StoppedBy};
use anyhow::bail;
use serde::Serialize;

use crate::{
    output::print_json,
    status::{duration, now_ms},
};

const STOP_WAIT: Duration = Duration::from_secs(10);
const STOP_POLL: Duration = Duration::from_millis(100);

/// What `allot daemon status --json` prints when allotd is not running, in place of its health
/// response, which has no `running` field: with the stop record while the user keeps it stopped.
#[derive(Debug, Default, Serialize)]
struct NotRunning {
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stopped: Option<StopRecord>,
}

/// Clears the stop record, then starts allotd in the background unless it is already running.
///
/// # Errors
///
/// Fails when the record cannot be removed, or allotd cannot be started or does not become
/// ready.
pub fn start() -> anyhow::Result<()> {
    let client = BlockingClient::discover(true)?;
    client.clear_stop_record()?;
    let started = client.ensure_running()?;
    let health = client.health()?;
    let verb = match started {
        DaemonStart::AlreadyRunning => "already running",
        DaemonStart::Started => "started",
    };
    println!(
        "allotd {verb} at {} (pid {})",
        client.base_url(),
        health.pid
    );
    Ok(())
}

/// Writes the stop record, then asks allotd to shut down and waits until it stops answering.
/// The record comes first, so no session starts allotd again the moment it exits; it stays
/// until allotd is started again.
///
/// # Errors
///
/// Fails when the record cannot be written, or allotd refuses the request or keeps answering
/// after the wait; the record is then removed again.
pub fn stop() -> anyhow::Result<()> {
    let client = BlockingClient::discover(false)?;
    let recorded = client.record_stop(StoppedBy::Cli)?;
    let stopped = shutdown(&client).inspect_err(|_| {
        let _ = client.clear_stop_record();
    })?;
    let what = match stopped {
        Some(_) => "allotd stopped",
        None => "allotd is not running",
    };
    if recorded {
        println!("{what}; it stays stopped until {UNTIL_STARTED}");
    } else {
        println!("{what}");
    }
    Ok(())
}

/// What ends a stop, after "it stays stopped until".
const UNTIL_STARTED: &str =
    "`allot daemon start`, Start service in the panel, or your next sign-in";

/// One line on a stop: who made it, how long ago, and what ends it.
pub(crate) fn describe_stop(record: &StopRecord, now_ms: i64) -> String {
    format!(
        "you stopped it {} {} ago; it stays stopped until {UNTIL_STARTED}, and heavy commands \
         run without leases meanwhile",
        record.by.how(),
        duration(now_ms - record.stopped_at_ms)
    )
}

/// Asks the allotd behind `client` to shut down and waits until it stops answering. Returns its
/// pid, or `None` when it was not running.
///
/// # Errors
///
/// Fails when allotd refuses the request or keeps answering after the wait.
pub fn shutdown(client: &BlockingClient) -> anyhow::Result<Option<u32>> {
    let pid = match client.health() {
        Ok(health) => health.pid,
        Err(error) if error.is_unsent() => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    match client.shutdown() {
        Err(error) if error.is_unsent() => return Ok(None),
        other => other?,
    }
    let deadline = Instant::now() + STOP_WAIT;
    // A session may start a new allotd the moment the old one exits; only the old one counts.
    while client.health().is_ok_and(|health| health.pid == pid) {
        if Instant::now() >= deadline {
            bail!(
                "allotd is still answering {} s after the shutdown request",
                STOP_WAIT.as_secs()
            );
        }
        thread::sleep(STOP_POLL);
    }
    Ok(Some(pid))
}

/// Reports whether allotd is running, without starting it, and whether the user stopped it.
/// With `json`, prints its health response, or `{"running": false}` when it is not running,
/// with the stop record as `stopped` while there is one.
///
/// # Errors
///
/// Fails when the local configuration cannot be read.
pub fn status(json: bool) -> anyhow::Result<()> {
    let client = BlockingClient::discover(false)?;
    match client.health() {
        Ok(health) if json => print_json(&health)?,
        Err(error) if json && error.is_unreachable() => print_json(&NotRunning {
            running: false,
            stopped: client.stop_record(),
        })?,
        Ok(health) => {
            println!(
                "allotd {} running at {} (pid {}, up {} s)",
                health.version,
                client.base_url(),
                health.pid,
                health.uptime_secs
            );
            if let Some(error) = health.config_error {
                println!("config rejected, allotd keeps the previous one: {error}");
            }
        }
        Err(error) if error.is_unreachable() => match client.stop_record() {
            Some(record) => println!("allotd is stopped: {}", describe_stop(&record, now_ms())),
            None => println!("allotd is not running at {}", client.base_url()),
        },
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_running_json_has_only_the_running_flag() {
        assert_eq!(
            serde_json::to_string(&NotRunning::default()).unwrap(),
            r#"{"running":false}"#
        );
    }

    #[test]
    fn test_not_running_json_carries_the_stop_record() {
        let stopped = NotRunning {
            running: false,
            stopped: Some(StopRecord {
                stopped_at_ms: 7,
                by: StoppedBy::Panel,
            }),
        };
        assert_eq!(
            serde_json::to_string(&stopped).unwrap(),
            r#"{"running":false,"stopped":{"stopped_at_ms":7,"by":"panel"}}"#
        );
    }

    #[test]
    fn test_describe_stop_names_the_source_the_age_and_the_way_back() {
        let record = StopRecord {
            stopped_at_ms: 1_000,
            by: StoppedBy::Cli,
        };
        assert_eq!(
            describe_stop(&record, 1_000 + 125_000),
            "you stopped it with `allot daemon stop` 2m 05s ago; it stays stopped until \
             `allot daemon start`, Start service in the panel, or your next sign-in, and heavy \
             commands run without leases meanwhile"
        );
    }
}
