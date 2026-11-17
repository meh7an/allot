//! `allot history`: what past jobs used, and the estimates the daemon learned from them.

use allot_client::BlockingClient;
use allot_shared::{
    api::{JobSummary, RunStats},
    types::{BusyUsage, JobProfile, JobRun},
};

use crate::{
    output::{HEADING, paint, print_json},
    status::{duration, gb, now_ms},
};

/// Prints one entry per job fingerprint, or with `fingerprint` the runs of that job, as text or
/// as the daemon's job summaries or runs.
///
/// # Errors
///
/// Fails when allotd cannot be reached or started.
pub fn history(fingerprint: Option<&str>, limit: Option<u32>, json: bool) -> anyhow::Result<()> {
    let client = BlockingClient::discover(true)?;
    let now = now_ms();
    let text = match fingerprint {
        Some(fingerprint) => {
            let response = client.history_runs_with_stats(fingerprint, limit)?;
            if json {
                return print_json(&response.runs);
            }
            format_runs(fingerprint, &response.runs, response.stats.as_ref(), now)
        }
        None => {
            let jobs = client.history()?;
            if json {
                return print_json(&jobs);
            }
            format_jobs(&jobs, now)
        }
    };
    anstream::print!("{text}");
    Ok(())
}

fn format_jobs(jobs: &[JobSummary], now_ms: i64) -> String {
    if jobs.is_empty() {
        return "no job history yet\n".to_owned();
    }
    let mut text = String::from(
        "median, p90 and max over the newest runs without an overlapping lease: cores and GPU \
         while busy, peak RAM and VRAM, run time\n",
    );
    for job in jobs {
        text.push_str(&format!(
            "\n{}\n  {} run(s), last ended {} ago: {}\n",
            paint(HEADING, &job.fingerprint),
            job.runs,
            duration(now_ms - job.last_ended_at_ms),
            job.command
        ));
        match (&job.stats, &job.learned) {
            (Some(stats), _) => text.push_str(&format_stats(stats)),
            // An older allotd sends the p90 alone.
            (None, Some(profile)) => text.push_str(&format!(
                "  needs {} (from {} run(s))\n",
                format_profile(profile),
                job.estimate_runs
            )),
            (None, None) => {
                text.push_str("  no statistics: every run overlapped another lease or was killed\n")
            }
        }
    }
    text
}

/// Median, p90 and max as a small table; GPU and VRAM columns only when some run measured
/// them.
fn format_stats(stats: &RunStats) -> String {
    let gpu = stats.max.gpu_pct > 0.0 || stats.max.vram_mb > 0;
    let header = ["cores", "RAM", "GPU", "VRAM", "time"].map(str::to_owned);
    let mut text = stats_row(&format!("from {} run(s)", stats.runs), header, gpu);
    for (name, profile) in [
        ("median", stats.median),
        ("p90", stats.p90),
        ("max", stats.max),
    ] {
        let cells = [
            cores(profile.cores),
            gb(profile.ram_mb),
            format!("{:.0}%", profile.gpu_pct),
            gb(profile.vram_mb),
            duration(secs_to_ms(profile.est_secs)),
        ];
        text.push_str(&stats_row(name, cells, gpu));
    }
    text
}

fn stats_row(label: &str, cells: [String; 5], gpu: bool) -> String {
    let [cores, ram, gpu_pct, vram, time] = cells;
    let mut row = format!("  {label:<14}  {cores:>5}  {ram:>7}");
    if gpu {
        row.push_str(&format!("  {gpu_pct:>4}  {vram:>7}"));
    }
    row.push_str(&format!("  {time}\n"));
    row
}

/// Two decimals under one core, where a job whose work runs outside its session shows.
fn cores(cores: f32) -> String {
    if cores < 1.0 {
        format!("{cores:.2}")
    } else {
        format!("{cores:.1}")
    }
}

fn format_profile(profile: &JobProfile) -> String {
    let mut text = format!("{:.1} cores, {}", profile.cores, gb(profile.ram_mb));
    if profile.gpu_pct > 0.0 || profile.vram_mb > 0 {
        text.push_str(&format!(
            ", gpu {:.0}%, vram {}",
            profile.gpu_pct,
            gb(profile.vram_mb)
        ));
    }
    text.push_str(&format!(", {}", duration(secs_to_ms(profile.est_secs))));
    text
}

fn format_runs(
    fingerprint: &str,
    runs: &[JobRun],
    stats: Option<&RunStats>,
    now_ms: i64,
) -> String {
    if runs.is_empty() {
        return format!("no runs of {fingerprint}\n");
    }
    let mut text = format!("{}\n", paint(HEADING, fingerprint));
    if let Some(stats) = stats {
        text.push_str(&format_stats(stats));
        text.push('\n');
    }
    for run in runs {
        let lease = run
            .lease_id
            .map_or_else(|| "-".to_owned(), |id| id.0.to_string());
        let busy = |value: fn(&BusyUsage) -> f32, digits: usize| {
            run.busy.as_ref().map_or_else(
                || "-".to_owned(),
                |busy| format!("{:.digits$}", value(busy)),
            )
        };
        text.push_str(&format!(
            "  lease {lease:<5} {:>8} ago  took {:<8} cores avg {:.1} busy {} peak {:.1}, \
             ram peak {}",
            duration(now_ms - run.ended_at_ms),
            duration(run.ended_at_ms - run.started_at_ms),
            run.avg.cores,
            busy(|busy| busy.cores, 1),
            run.peak.cores,
            gb(run.peak.ram_mb)
        ));
        if run.peak.gpu_pct > 0.0 || run.peak.vram_mb > 0 {
            text.push_str(&format!(
                ", gpu avg {:.0}% busy {}%, vram peak {}",
                run.avg.gpu_pct,
                busy(|busy| busy.gpu_pct, 0),
                gb(run.peak.vram_mb)
            ));
        }
        if run.overlapped {
            text.push_str("  (overlapped)");
        }
        text.push_str(&format!("  {}\n", run.command));
    }
    text
}

fn secs_to_ms(secs: u64) -> i64 {
    i64::try_from(secs.saturating_mul(1_000)).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use allot_shared::types::{LeaseId, ResourceUsage};

    use super::*;
    use crate::output::plain;

    fn profile() -> JobProfile {
        JobProfile {
            cores: 6.25,
            ram_mb: 2_458,
            gpu_pct: 0.0,
            vram_mb: 0,
            est_secs: 95,
        }
    }

    fn run(overlapped: bool) -> JobRun {
        let usage = |cores, ram_mb| ResourceUsage {
            cores,
            ram_mb,
            gpu_pct: 0.0,
            vram_mb: 0,
        };
        JobRun {
            lease_id: Some(LeaseId(41)),
            session: None,
            end_cause: None,
            fingerprint: "cargo build|e:\\app".into(),
            command: "cargo build 2>&1".into(),
            cwd: "E:\\app".into(),
            declared: profile(),
            peak: usage(14.0, 2_458),
            avg: usage(6.2, 1_800),
            busy: Some(BusyUsage {
                cores: 9.3,
                gpu_pct: 0.0,
            }),
            started_at_ms: 0,
            ended_at_ms: 95_000,
            noise: None,
            overlapped,
        }
    }

    fn stats() -> RunStats {
        RunStats {
            runs: 10,
            median: JobProfile {
                cores: 4.3,
                ram_mb: 1_843,
                est_secs: 70,
                ..profile()
            },
            p90: profile(),
            max: JobProfile {
                cores: 7.1,
                ram_mb: 3_174,
                est_secs: 122,
                ..profile()
            },
        }
    }

    fn gpu_stats() -> RunStats {
        let gpu = |gpu_pct, vram_mb, cores| JobProfile {
            cores,
            gpu_pct,
            vram_mb,
            ..profile()
        };
        RunStats {
            runs: 3,
            median: gpu(40.0, 2_048, 0.28),
            p90: gpu(85.0, 6_144, 1.5),
            max: gpu(100.0, 7_680, 2.0),
        }
    }

    #[test]
    fn test_format_jobs_snapshot_shows_median_p90_and_max_per_job() {
        let job = JobSummary {
            fingerprint: r"cargo build|e:\app".into(),
            command: "cargo build 2>&1".into(),
            cwd: r"E:\app".into(),
            runs: 12,
            estimate_runs: 10,
            last_ended_at_ms: 95_000,
            learned: Some(profile()),
            stats: Some(stats()),
        };
        let trainer = JobSummary {
            fingerprint: r"python train.py|e:\ml".into(),
            command: "python train.py".into(),
            runs: 3,
            estimate_runs: 3,
            learned: Some(gpu_stats().p90),
            stats: Some(gpu_stats()),
            ..job.clone()
        };
        let older_daemon = JobSummary {
            stats: None,
            ..job.clone()
        };
        let unknown = JobSummary {
            learned: None,
            stats: None,
            estimate_runs: 0,
            ..job.clone()
        };
        let text = plain(format_jobs(&[job, trainer, older_daemon, unknown], 397_000));
        let (legend, jobs) = text.split_once("\n\n").unwrap();
        assert!(
            legend.starts_with("median, p90 and max over the newest runs without an overlapping"),
            "{legend}"
        );
        let expected = r"cargo build|e:\app
  12 run(s), last ended 5m 02s ago: cargo build 2>&1
  from 10 run(s)  cores      RAM  time
  median            4.3   1.8 GB  1m 10s
  p90               6.2   2.4 GB  1m 35s
  max               7.1   3.1 GB  2m 02s

python train.py|e:\ml
  3 run(s), last ended 5m 02s ago: python train.py
  from 3 run(s)   cores      RAM   GPU     VRAM  time
  median           0.28   2.4 GB   40%   2.0 GB  1m 35s
  p90               1.5   2.4 GB   85%   6.0 GB  1m 35s
  max               2.0   2.4 GB  100%   7.5 GB  1m 35s

cargo build|e:\app
  12 run(s), last ended 5m 02s ago: cargo build 2>&1
  needs 6.2 cores, 2.4 GB, 1m 35s (from 10 run(s))

cargo build|e:\app
  12 run(s), last ended 5m 02s ago: cargo build 2>&1
  no statistics: every run overlapped another lease or was killed
";
        assert_eq!(jobs, expected);
        assert_eq!(format_jobs(&[], 0), "no job history yet\n");
    }

    #[test]
    fn test_format_runs_puts_the_statistics_above_the_runs() {
        let text = plain(format_runs(
            r"cargo build|e:\app",
            &[run(false)],
            Some(&stats()),
            155_000,
        ));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "  from 10 run(s)  cores      RAM  time");
        assert_eq!(lines[3], "  p90               6.2   2.4 GB  1m 35s");
        assert_eq!(lines[5], "");
        assert!(lines[6].starts_with("  lease 41"), "{text}");
    }

    #[test]
    fn test_format_runs_marks_overlapped_runs() {
        let older = JobRun {
            busy: None,
            ..run(false)
        };
        let text = plain(format_runs(
            "cargo build|e:\\app",
            &[run(false), run(true), older],
            None,
            155_000,
        ));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "cargo build|e:\\app");
        assert_eq!(
            lines[1],
            "  lease 41      1m 00s ago  took 1m 35s   cores avg 6.2 busy 9.3 peak 14.0, \
             ram peak 2.4 GB  cargo build 2>&1"
        );
        assert!(lines[2].contains("(overlapped)  cargo build"), "{text}");
        assert!(
            lines[3].contains("cores avg 6.2 busy - peak 14.0"),
            "{text}"
        );
        assert_eq!(format_runs("x|", &[], None, 0), "no runs of x|\n");
    }
}
