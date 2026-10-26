//! `allot probe --sample`: live machine samples, for checking the sampler against Task Manager.

use std::{
    env, process, thread,
    time::{Duration, Instant},
};

use allot_core::sampler::{Sampler, SnapshotBuilder};
use allot_sampler::{SystemSampler, current_ancestry};
use allot_shared::{
    platform::SESSION_ROOTS,
    types::{Holder, ResourceUsage, SessionKey, Snapshot},
};

/// Prints one sample per second for `secs` seconds, then the sampler's own CPU cost.
///
/// When run from a Claude Code session, that session's usage is shown separately.
///
/// # Errors
///
/// Fails when the process table or the system counters cannot be read.
pub fn run(secs: u64) -> anyhow::Result<()> {
    let sessions: Vec<Holder> = current_session()?.into_iter().collect();
    let roots = SESSION_ROOTS.join(" or ");
    match sessions.first() {
        Some(holder) => println!("session root: {roots} pid {}", holder.pid),
        None => println!("no {roots} ancestor; session usage not shown"),
    }

    let mut sampler = SystemSampler::new()?;
    let mut builder = SnapshotBuilder::new();
    let own_pid = process::id();
    let start = Instant::now();
    let mut own_cores = 0.0;
    let mut logical_cores = 1;
    for tick in 1..=secs {
        let deadline = start + Duration::from_secs(tick);
        thread::sleep(deadline.saturating_duration_since(Instant::now()));
        let raw = sampler.sample()?;
        own_cores += raw
            .processes
            .iter()
            .find(|process| process.pid == own_pid)
            .map_or(0.0, |process| process.usage.cores);
        let snapshot = builder.build(raw, &sessions);
        if tick == 1 {
            for (index, gpu) in snapshot.gpus.iter().enumerate() {
                println!("gpu{index}: {} (luid {})", gpu.name, gpu.luid);
            }
        }
        print!("{}", format_snapshot(tick, &snapshot));
        logical_cores = snapshot.cpu.logical_cores.max(1);
    }

    let avg_cores = own_cores / secs as f32;
    println!(
        "sampler cost: {avg_cores:.3} cores on average = {:.2}% of the machine",
        avg_cores / logical_cores as f32 * 100.0
    );
    Ok(())
}

/// The nearest `claude.exe` ancestor, keyed by this session's ID when Claude set one.
fn current_session() -> anyhow::Result<Option<Holder>> {
    let root = current_ancestry()?.into_iter().find(|node| {
        SESSION_ROOTS
            .iter()
            .any(|root| node.name.eq_ignore_ascii_case(root))
    });
    Ok(root.map(|node| Holder {
        session: SessionKey(env::var("CLAUDE_CODE_SESSION_ID").unwrap_or_default()),
        pid: node.pid,
        pid_start_time: node.start_time,
    }))
}

fn format_snapshot(tick: u64, snapshot: &Snapshot) -> String {
    let cpu = snapshot.cpu.usage_pct;
    let memory = &snapshot.memory;
    let mut text = format!(
        "{tick:>3}s cpu {:5.1}% (10s {:5.1}%, 60s {:5.1}%)  ram {} / {}  commit {} / {}\n",
        cpu.avg_1s,
        cpu.avg_10s,
        cpu.avg_60s,
        gb(memory.used_mb),
        gb(memory.total_mb),
        gb(memory.commit_used_mb),
        gb(memory.commit_limit_mb),
    );
    for (index, gpu) in snapshot.gpus.iter().enumerate() {
        let engines = gpu.engines;
        text.push_str(&format!(
            "     gpu{index} 3d {:3.0}%  compute {:3.0}%  copy {:3.0}%  video {:3.0}%  \
             vram {} / {}  shared {}\n",
            engines.three_d_pct.avg_1s,
            engines.compute_pct.avg_1s,
            engines.copy_pct.avg_1s,
            engines.video_pct.avg_1s,
            gb(gpu.dedicated_used_mb),
            gpu.dedicated_total_mb.map_or_else(|| "?".to_owned(), gb),
            gb(gpu.shared_used_mb),
        ));
    }
    for session in &snapshot.sessions {
        text.push_str(&format!(
            "     session {}  ({} processes)\n",
            format_usage(&session.usage),
            session.processes.len()
        ));
    }
    text.push_str(&format!(
        "     other   {}\n",
        format_usage(&snapshot.unattributed)
    ));
    text
}

fn format_usage(usage: &ResourceUsage) -> String {
    format!(
        "{:6.2} cores  ram {}  gpu {:3.0}%  vram {}",
        usage.cores,
        gb(usage.ram_mb),
        usage.gpu_pct,
        gb(usage.vram_mb)
    )
}

fn gb(mb: u64) -> String {
    format!("{:.1} GB", mb as f64 / 1024.0)
}
