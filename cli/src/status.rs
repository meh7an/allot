//! `allot status` and `allot leases`: a human view of the machine, its sessions, and their leases.

use std::{
    collections::HashSet,
    io::{self, IsTerminal, Write},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use allot_client::BlockingClient;
use allot_shared::{
    api::{HealthResponse, LeaseQuery},
    types::{EndCause, JobClass, Lease, LeaseState, ResourceUsage, SessionKey, Snapshot},
    ui::Notice,
};
use anstyle::Style;

use crate::output::{BAD, DIM, GOOD, HEADING, WAIT, paint, print_json};

const SNAPSHOT_WAIT: Duration = Duration::from_secs(5);
const SNAPSHOT_POLL: Duration = Duration::from_millis(200);
const HISTORY_SHOWN: usize = 20;
const SESSION_ID_SHOWN: usize = 8;
// Kills and removals older than this are history, not status.
const NOTICES_SHOWN_MS: i64 = 60 * 60 * 1_000;
/// Moves the cursor home and clears the screen, so each `--watch` frame replaces the last.
const CLEAR_SCREEN: &str = "\u{1b}[H\u{1b}[2J";

/// How `allot status` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusMode {
    /// Once, as text.
    Text,
    /// Once, as the daemon's `/ui/state`.
    Json,
    /// As text, again every interval until interrupted.
    Watch(Duration),
}

/// Prints machine usage, every session with its leases, and the rest, once or every interval,
/// or the daemon's whole UI state as JSON. Starts allotd when it is not running.
///
/// # Errors
///
/// Fails when allotd cannot be reached or started. While watching, only the first frame can
/// fail; later failures are shown in the frame and retried.
pub fn status(mode: StatusMode) -> anyhow::Result<()> {
    let client = BlockingClient::discover(true)?;
    match mode {
        StatusMode::Text => anstream::print!("{}", status_text(&client)?),
        StatusMode::Json => print_json(&when_ready(|| client.ui_state())?)?,
        StatusMode::Watch(interval) => watch(&client, interval)?,
    }
    Ok(())
}

fn status_text(client: &BlockingClient) -> anyhow::Result<String> {
    let snapshot = snapshot_when_ready(client)?;
    let health = client.health()?;
    let leases = current_leases(client)?;
    let notices = recent_notices(client)?;
    Ok(format_status(
        &health,
        &snapshot,
        &leases,
        &notices,
        now_ms(),
    ))
}

/// Redraws the status every `interval` until the process is interrupted. On a terminal each
/// frame replaces the last; piped, frames follow one another with a blank line between.
fn watch(client: &BlockingClient, interval: Duration) -> anyhow::Result<()> {
    let terminal = io::stdout().is_terminal();
    // Fail at once when allotd is unusable, rather than showing the error forever.
    let mut frame = status_text(client)?;
    loop {
        let header = paint(
            DIM,
            &format!(
                "every {}, Ctrl-C stops\n\n",
                duration(i64::try_from(interval.as_millis()).unwrap_or(i64::MAX))
            ),
        );
        match draw(terminal, &format!("{header}{frame}")) {
            // The reader went away, such as `head` that has its lines: done, not an error.
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
            other => other?,
        }
        thread::sleep(interval);
        frame = status_text(client)
            .unwrap_or_else(|error| format!("{} {error:#}\n", paint(BAD, "Error:")));
    }
}

/// The newest kills and removals; none from an allotd built before they existed.
fn recent_notices(client: &BlockingClient) -> anyhow::Result<Vec<Notice>> {
    match client.ui_state() {
        Ok(state) => Ok(state.notices),
        Err(error) if error.status() == Some(404) => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

/// Prints active and queued leases, or with `all` the most recent ones in any state, as text or
/// as the daemon's lease objects.
///
/// # Errors
///
/// Fails when allotd cannot be reached or started.
pub fn leases(all: bool, json: bool) -> anyhow::Result<()> {
    let client = BlockingClient::discover(true)?;
    let leases = if all {
        let every = client.leases(&LeaseQuery::default())?;
        let skip = every.len().saturating_sub(HISTORY_SHOWN);
        every.into_iter().skip(skip).collect()
    } else {
        current_leases(&client)?
    };
    if json {
        return print_json(&leases);
    }
    anstream::print!("{}", format_leases(&leases, now_ms()));
    Ok(())
}

fn format_leases(leases: &[Lease], now_ms: i64) -> String {
    if leases.is_empty() {
        return "no leases\n".to_owned();
    }
    leases
        .iter()
        .map(|lease| {
            format!(
                "{}  {}\n",
                short(&lease.holder.session),
                format_lease(lease, now_ms)
            )
        })
        .collect()
}

/// Writes one `--watch` frame over the last one on a terminal, or after it with a blank line.
fn draw(terminal: bool, frame: &str) -> io::Result<()> {
    if terminal {
        // Not colour, so it is written even under NO_COLOR; the ANSI stream also turns on
        // escape sequences in a Windows console.
        let mut raw = anstream::AutoStream::always_ansi(io::stdout());
        write!(raw, "{CLEAR_SCREEN}")?;
        raw.flush()?;
    }
    let mut out = anstream::stdout();
    write!(out, "{frame}")?;
    if !terminal {
        writeln!(out)?;
    }
    out.flush()
}

fn snapshot_when_ready(client: &BlockingClient) -> anyhow::Result<Snapshot> {
    when_ready(|| client.snapshot())
}

/// A freshly started allotd answers 503 until its first sample; wait that out briefly.
fn when_ready<T>(call: impl Fn() -> allot_client::Result<T>) -> anyhow::Result<T> {
    let deadline = Instant::now() + SNAPSHOT_WAIT;
    loop {
        match call() {
            Err(error) if error.status() == Some(503) && Instant::now() < deadline => {
                thread::sleep(SNAPSHOT_POLL);
            }
            other => return Ok(other?),
        }
    }
}

fn current_leases(client: &BlockingClient) -> anyhow::Result<Vec<Lease>> {
    let mut leases = Vec::new();
    for state in [LeaseState::Active, LeaseState::Queued] {
        let query = LeaseQuery {
            session: None,
            state: Some(state),
        };
        leases.extend(client.leases(&query)?);
    }
    Ok(leases)
}

fn format_status(
    health: &HealthResponse,
    snapshot: &Snapshot,
    leases: &[Lease],
    notices: &[Notice],
    now_ms: i64,
) -> String {
    let cpu = snapshot.cpu.usage_pct;
    let memory = &snapshot.memory;
    let mut text = format!(
        "allotd {} (pid {}, up {})\n",
        health.version,
        health.pid,
        duration(i64::try_from(health.uptime_secs).unwrap_or(i64::MAX) * 1_000)
    );
    if let Some(error) = &health.config_error {
        text.push_str(&format!(
            "config  {}, still using the previous one: {error}\n",
            paint(BAD, "REJECTED")
        ));
    }
    text.push_str(&format!(
        "cpu     {:5.1}% (10s {:.1}%, 60s {:.1}%) of {} logical cores\n",
        cpu.avg_1s, cpu.avg_10s, cpu.avg_60s, snapshot.cpu.logical_cores
    ));
    text.push_str(&format!(
        "ram     {} / {}, commit {} / {}\n",
        gb(memory.used_mb),
        gb(memory.total_mb),
        gb(memory.commit_used_mb),
        gb(memory.commit_limit_mb)
    ));
    for (index, gpu) in snapshot.gpus.iter().enumerate() {
        text.push_str(&format!(
            "gpu{index}    {}: busiest engine {:.0}%, vram {} / {}\n",
            gpu.name,
            gpu.engines.busiest(|load| load.avg_1s),
            gb(gpu.dedicated_used_mb),
            gpu.dedicated_total_mb.map_or_else(|| "?".to_owned(), gb)
        ));
    }

    text.push_str(&format!("\n{}\n", paint(HEADING, "sessions")));
    if snapshot.sessions.is_empty() {
        text.push_str("  none registered\n");
    }
    let mut shown: HashSet<&SessionKey> = HashSet::new();
    for session in &snapshot.sessions {
        shown.insert(&session.holder.session);
        text.push_str(&format!(
            "  {}  pid {:<6} {}\n",
            short(&session.holder.session),
            session.holder.pid,
            format_usage(&session.usage)
        ));
        for lease in leases
            .iter()
            .filter(|l| l.holder.session == session.holder.session)
        {
            text.push_str(&format!("    {}\n", format_lease(lease, now_ms)));
        }
    }
    for lease in leases.iter().filter(|l| !shown.contains(&l.holder.session)) {
        text.push_str(&format!(
            "  {}  (not sampled) {}\n",
            short(&lease.holder.session),
            format_lease(lease, now_ms)
        ));
    }
    text.push_str(&format!(
        "other              {}\n",
        format_usage(&snapshot.unattributed)
    ));

    let recent: Vec<&Notice> = notices
        .iter()
        .filter(|notice| now_ms - notice.ended_at_ms <= NOTICES_SHOWN_MS)
        .collect();
    if !recent.is_empty() {
        text.push_str(&format!("\n{}\n", paint(HEADING, "stopped by the user")));
    }
    for notice in recent {
        let told = if notice.delivered_at_ms.is_some() {
            ""
        } else {
            "  (session not told yet)"
        };
        text.push_str(&format!(
            "  {}  lease {:<4} {} {} ago  {}{told}\n",
            short(&notice.session),
            notice.lease_id.0,
            paint(BAD, &format!("{:<8}", notice.cause.as_str())),
            duration(now_ms - notice.ended_at_ms),
            notice.command
        ));
    }
    text
}

fn format_usage(usage: &ResourceUsage) -> String {
    format!(
        "{:5.2} cores  ram {}  gpu {:3.0}%  vram {}",
        usage.cores,
        gb(usage.ram_mb),
        usage.gpu_pct,
        gb(usage.vram_mb)
    )
}

fn format_lease(lease: &Lease, now_ms: i64) -> String {
    let profile = &lease.profile;
    let class = lease.class.map_or("custom", JobClass::as_str);
    let since = match lease.state {
        LeaseState::Active => lease.granted_at_ms,
        LeaseState::Queued => Some(lease.requested_at_ms),
        _ => lease.ended_at_ms,
    };
    let age = since.map_or_else(String::new, |at| format!(" for {}", duration(now_ms - at)));
    // A kill or removal is stored as expired; the cause is what the user wants to see.
    let notice = lease.end_cause.filter(|cause| cause.is_notice());
    let shown_state = notice.map_or(lease.state.as_str(), EndCause::as_str);
    let mut text = format!(
        "lease {:<4} {} {:<9} {:.1} cores, {}",
        lease.id.0,
        paint(
            state_style(lease.state, notice.is_some()),
            &format!("{shown_state:<8}")
        ),
        class,
        profile.cores,
        gb(profile.ram_mb)
    );
    if profile.gpu_pct > 0.0 || profile.vram_mb > 0 {
        text.push_str(&format!(
            ", gpu {:.0}%, vram {}",
            profile.gpu_pct,
            gb(profile.vram_mb)
        ));
    }
    text.push_str(&format!("{age}  {}", lease.label));
    text
}

/// Active is good news, queued is waiting, and a kill or removal is the user stopping it;
/// anything else that ended is past and needs no colour.
fn state_style(state: LeaseState, stopped_by_user: bool) -> Style {
    match state {
        _ if stopped_by_user => BAD,
        LeaseState::Active => GOOD,
        LeaseState::Queued => WAIT,
        _ => Style::new(),
    }
}

fn short(session: &SessionKey) -> String {
    let id: String = session.0.chars().take(SESSION_ID_SHOWN).collect();
    paint(DIM, &format!("{id:<SESSION_ID_SHOWN$}"))
}

pub(crate) fn gb(mb: u64) -> String {
    format!("{:.1} GB", mb as f64 / 1024.0)
}

/// `42s`, `12m 05s`, or `2h 03m`.
pub(crate) fn duration(ms: i64) -> String {
    let secs = ms.max(0) / 1_000;
    match secs {
        0..60 => format!("{secs}s"),
        60..3_600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3_600, secs % 3_600 / 60),
    }
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use allot_shared::types::{
        CpuStats, Holder, JobProfile, LeaseId, LeaseKind, MemStats, Rolling, SessionUsage,
    };

    use super::*;
    use crate::output::plain;

    #[test]
    fn test_format_lease_colours_the_state_without_moving_the_columns() {
        let queued = format_lease(&lease(4, "s", LeaseState::Queued), 2_000);
        assert!(queued.contains(&paint(WAIT, "queued  ")), "{queued:?}");
        let active = format_lease(&lease(5, "s", LeaseState::Active), 2_000);
        assert!(active.contains(&paint(GOOD, "active  ")), "{active:?}");
        assert_eq!(
            plain(queued).find("cpu_heavy"),
            plain(active).find("cpu_heavy")
        );
    }

    #[test]
    fn test_format_leases_lists_each_lease_under_its_session() {
        let leases = [
            lease(12, "3f2a9c1e-aaaa-bbbb", LeaseState::Active),
            lease(13, "77777777-cccc", LeaseState::Queued),
        ];
        let text = plain(format_leases(&leases, 62_000));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(
            lines[0].starts_with("3f2a9c1e  lease 12   active "),
            "{text}"
        );
        assert!(
            lines[1].starts_with("77777777  lease 13   queued "),
            "{text}"
        );
        assert_eq!(format_leases(&[], 0), "no leases\n");
    }

    #[test]
    fn test_lease_json_is_the_daemon_wire_shape() {
        let lease = lease(12, "3f2a9c1e", LeaseState::Active);
        let json = serde_json::to_value([&lease]).unwrap();
        assert_eq!(json[0]["id"], 12);
        assert_eq!(json[0]["state"], "active");
        let back: Vec<Lease> = serde_json::from_value(json).unwrap();
        assert_eq!(back, [lease]);
    }

    fn holder(session: &str) -> Holder {
        Holder {
            session: SessionKey(session.into()),
            pid: 60_012,
            pid_start_time: 1,
        }
    }

    fn lease(id: i64, session: &str, state: LeaseState) -> Lease {
        Lease {
            id: LeaseId(id),
            holder: holder(session),
            kind: LeaseKind::Normal,
            state,
            class: Some(JobClass::CpuHeavy),
            profile: JobProfile {
                cores: 8.0,
                ram_mb: 4_096,
                gpu_pct: 0.0,
                vram_mb: 0,
                est_secs: 300,
            },
            label: "cargo build".into(),
            cwd: String::new(),
            requested_at_ms: 1_000,
            granted_at_ms: Some(2_000),
            ended_at_ms: None,
            expires_at_ms: 60_000,
            end_cause: None,
            binding: None,
            isolation: None,
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            taken_at_ms: 0,
            cpu: CpuStats {
                logical_cores: 16,
                usage_pct: Rolling {
                    avg_1s: 23.4,
                    avg_10s: 20.0,
                    avg_60s: 15.0,
                },
                per_core_pct: vec![],
            },
            memory: MemStats {
                total_mb: 32_768,
                used_mb: 21_504,
                available_mb: 11_264,
                commit_used_mb: 40_960,
                commit_limit_mb: 56_320,
                reclaimable_mb: 0,
            },
            gpus: vec![],
            sessions: vec![SessionUsage {
                holder: holder("3f2a9c1e-aaaa-bbbb"),
                usage: ResourceUsage {
                    cores: 2.3,
                    ram_mb: 410,
                    gpu_pct: 0.0,
                    vram_mb: 0,
                },
                processes: vec![],
            }],
            unattributed: ResourceUsage::default(),
            drives: Vec::new(),
        }
    }

    #[test]
    fn test_format_status_groups_leases_under_their_session() {
        let health = HealthResponse {
            version: "0.1.0".into(),
            pid: 1_234,
            uptime_secs: 3_725,
            config_error: None,
        };
        let leases = [
            lease(12, "3f2a9c1e-aaaa-bbbb", LeaseState::Active),
            lease(13, "77777777-cccc", LeaseState::Queued),
        ];
        let text = plain(format_status(&health, &snapshot(), &leases, &[], 62_000));
        assert!(
            text.starts_with("allotd 0.1.0 (pid 1234, up 1h 02m)\n"),
            "{text}"
        );
        assert!(text.contains("cpu      23.4% (10s 20.0%, 60s 15.0%) of 16 logical cores"));
        assert!(text.contains("ram     21.0 GB / 32.0 GB, commit 40.0 GB / 55.0 GB"));
        assert!(
            text.contains("  3f2a9c1e  pid 60012   2.30 cores"),
            "{text}"
        );
        assert!(
            text.contains(
                "    lease 12   active   cpu_heavy 8.0 cores, 4.0 GB for 1m 00s  cargo build"
            ),
            "{text}"
        );
        assert!(
            text.contains("  77777777  (not sampled) lease 13"),
            "{text}"
        );
        assert!(!text.contains("config"), "{text}");
        assert!(!text.contains("stopped by the user"), "{text}");
    }

    #[test]
    fn test_format_status_lists_kills_and_removals_of_the_last_hour() {
        let health = HealthResponse {
            version: "0.2.0".into(),
            pid: 1_234,
            uptime_secs: 5,
            config_error: None,
        };
        let notice = |lease_id, cause, ended_at_ms, delivered_at_ms| Notice {
            lease_id: LeaseId(lease_id),
            session: SessionKey("3f2a9c1e-aaaa-bbbb".into()),
            cause,
            command: "cargo clippy".into(),
            tool_use_id: None,
            ended_at_ms,
            delivered_at_ms,
        };
        let now = 10_000_000;
        let notices = [
            notice(39, EndCause::Killed, now - 240_000, None),
            notice(38, EndCause::Removed, now - 600_000, Some(now - 590_000)),
            notice(37, EndCause::Killed, now - NOTICES_SHOWN_MS - 1, None),
        ];
        let text = plain(format_status(&health, &snapshot(), &[], &notices, now));
        assert!(
            text.contains(
                "stopped by the user\n  3f2a9c1e  lease 39   killed   4m 00s ago  cargo clippy  \
                 (session not told yet)\n  3f2a9c1e  lease 38   removed  10m 00s ago  cargo \
                 clippy\n"
            ),
            "{text}"
        );
        assert!(!text.contains("lease 37"), "{text}");
    }

    #[test]
    fn test_format_status_shows_a_rejected_config() {
        let health = HealthResponse {
            version: "0.1.0".into(),
            pid: 1_234,
            uptime_secs: 5,
            config_error: Some("config.toml: invalid configuration: x".into()),
        };
        let text = plain(format_status(&health, &snapshot(), &[], &[], 0));
        assert!(
            text.contains("config  REJECTED, still using the previous one: config.toml:"),
            "{text}"
        );
    }

    #[test]
    fn test_format_lease_shows_gpu_only_when_reserved() {
        let mut gpu_lease = lease(1, "s", LeaseState::Queued);
        gpu_lease.profile.gpu_pct = 80.0;
        gpu_lease.profile.vram_mb = 4_096;
        let text = plain(format_lease(&gpu_lease, 31_000));
        assert!(text.contains(", gpu 80%, vram 4.0 GB for 30s"), "{text}");
        assert!(!plain(format_lease(&lease(2, "s", LeaseState::Active), 0)).contains("gpu"));
    }

    #[test]
    fn test_format_lease_shows_a_kill_or_removal_instead_of_expired() {
        let ended = |cause| Lease {
            state: LeaseState::Expired,
            ended_at_ms: Some(2_000),
            end_cause: Some(cause),
            ..lease(3, "s", LeaseState::Expired)
        };
        assert!(
            plain(format_lease(&ended(EndCause::Killed), 3_000))
                .starts_with("lease 3    killed   ")
        );
        assert!(
            plain(format_lease(&ended(EndCause::Removed), 3_000))
                .starts_with("lease 3    removed  ")
        );
        assert!(
            plain(format_lease(&ended(EndCause::Expired), 3_000))
                .starts_with("lease 3    expired  ")
        );
    }

    #[test]
    fn test_duration_formats() {
        assert_eq!(duration(-5), "0s");
        assert_eq!(duration(42_000), "42s");
        assert_eq!(duration(725_000), "12m 05s");
        assert_eq!(duration(7_380_000), "2h 03m");
    }
}
