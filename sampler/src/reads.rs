//! Which processes a sampler tick reads, and their CPU load.
//!
//! Reading every process every second is nearly all of allotd's own cost on Linux and macOS, and
//! most of those processes are asleep. So a tick reads the new, the recently started and the
//! busy ones, and the rest in turn, each at least once per [`ROTATION_TICKS`]. Listing the PIDs
//! stays cheap, so a new process, such as a session that just took a lease, is known at once.

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use crate::cpu::ProcessTime;

/// An idle process is read once per this many ticks.
const ROTATION_TICKS: u64 = 10;

/// A process that used this share of a core at its last read counts as busy.
const BUSY_CORES: f32 = 0.01;

/// A new process is read every tick for this long: builds run many that live a few seconds, and
/// a first read is often too early to show any load.
const YOUNG_TICKS: u64 = 10;

#[derive(Debug, Clone, Copy)]
struct Seen {
    time: ProcessTime,
    read_at: Instant,
    first_tick: u64,
    cores: f32,
}

/// Remembers each process's last read, to pick the next ones and turn them into rates.
#[derive(Debug, Default)]
pub(crate) struct ProcessReads {
    tick: u64,
    seen: HashMap<u32, Seen>,
    previous_at: Option<Instant>,
}

impl ProcessReads {
    /// The PIDs to read in this tick, given every PID alive now. Includes those that have just
    /// gone, so that the process list drops them.
    pub(crate) fn due(&mut self, listed: &[u32]) -> Vec<u32> {
        self.tick += 1;
        let alive: HashSet<u32> = listed.iter().copied().collect();
        let mut due: Vec<u32> = self
            .seen
            .keys()
            .copied()
            .filter(|pid| !alive.contains(pid))
            .collect();
        self.seen.retain(|pid, _| alive.contains(pid));
        due.extend(listed.iter().copied().filter(|&pid| self.is_due(pid)));
        due
    }

    fn is_due(&self, pid: u32) -> bool {
        self.seen.get(&pid).is_none_or(|seen| {
            seen.cores >= BUSY_CORES
                || self.tick - seen.first_tick < YOUNG_TICKS
                || u64::from(pid) % ROTATION_TICKS == self.tick % ROTATION_TICKS
        })
    }

    /// Records the reads of this tick. Each process's load covers the time since its own last
    /// read; a process that appeared since the previous tick counts from its start. The first
    /// call only records a baseline, so its load reads zero.
    pub(crate) fn record(&mut self, now: Instant, reads: &[ProcessTime], max_cores: f32) {
        let tick_ms = self
            .previous_at
            .map(|at| now.saturating_duration_since(at).as_secs_f64() * 1_000.0);
        for read in reads {
            let before = self
                .seen
                .get(&read.pid)
                .filter(|seen| seen.time.start_time == read.start_time);
            let (used_ms, interval_ms) = match before {
                Some(seen) => (
                    read.cpu_ms.saturating_sub(seen.time.cpu_ms),
                    Some(now.saturating_duration_since(seen.read_at).as_secs_f64() * 1_000.0),
                ),
                None => (read.cpu_ms, tick_ms),
            };
            let cores = interval_ms
                .filter(|&ms| ms > 0.0)
                .map_or(0.0, |ms| (used_ms as f64 / ms) as f32)
                .min(max_cores);
            let first_tick = before.map_or(self.tick, |seen| seen.first_tick);
            self.seen.insert(
                read.pid,
                Seen {
                    time: *read,
                    read_at: now,
                    first_tick,
                    cores,
                },
            );
        }
        self.previous_at = Some(now);
    }

    /// Logical cores the process kept busy, as of its last read.
    pub(crate) fn cores(&self, pid: u32) -> f32 {
        self.seen.get(&pid).map_or(0.0, |seen| seen.cores)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const MAX_CORES: f32 = 64.0;

    fn time(pid: u32, start_time: u64, cpu_ms: u64) -> ProcessTime {
        ProcessTime {
            pid,
            start_time,
            cpu_ms,
        }
    }

    fn record_at(reads: &mut ProcessReads, start: Instant, secs: u64, times: &[ProcessTime]) {
        reads.record(start + Duration::from_secs(secs), times, MAX_CORES);
    }

    /// Ages `pid`, which used no CPU, past being young.
    fn settled(reads: &mut ProcessReads, pid: u32, start: Instant) -> Instant {
        let mut now = start;
        for _ in 0..=YOUNG_TICKS {
            reads.due(&[pid]);
            reads.record(now, &[time(pid, 1, 0)], MAX_CORES);
            now += Duration::from_secs(1);
        }
        now
    }

    #[test]
    fn test_due_reads_a_new_process_at_once() {
        let mut reads = ProcessReads::default();
        assert_eq!(reads.due(&[7]), [7]);
    }

    #[test]
    fn test_due_reads_an_idle_process_once_per_rotation() {
        let mut reads = ProcessReads::default();
        settled(&mut reads, 7, Instant::now());

        let hits = (0..ROTATION_TICKS)
            .filter(|_| !reads.due(&[7]).is_empty())
            .count();

        assert_eq!(hits, 1);
    }

    #[test]
    fn test_due_reads_a_busy_process_every_tick() {
        let mut reads = ProcessReads::default();
        let now = settled(&mut reads, 7, Instant::now());
        reads.record(now, &[time(7, 1, 500)], MAX_CORES);

        for _ in 0..ROTATION_TICKS {
            assert_eq!(reads.due(&[7]), [7]);
        }
    }

    #[test]
    fn test_due_reads_a_young_process_every_tick_even_without_load() {
        let mut reads = ProcessReads::default();
        let start = Instant::now();
        for tick in 0..YOUNG_TICKS {
            assert_eq!(reads.due(&[7]), [7], "tick {tick}");
            record_at(&mut reads, start, tick, &[time(7, 1, 0)]);
        }
    }

    #[test]
    fn test_due_lists_a_gone_process_once() {
        let mut reads = ProcessReads::default();
        reads.due(&[7]);
        reads.record(Instant::now(), &[time(7, 1, 0)], MAX_CORES);

        assert_eq!(reads.due(&[]), [7]);
        assert!(reads.due(&[]).is_empty());
    }

    #[test]
    fn test_record_first_call_is_only_a_baseline() {
        let mut reads = ProcessReads::default();
        reads.record(Instant::now(), &[time(7, 1, 5_000)], MAX_CORES);
        assert_eq!(reads.cores(7), 0.0);
    }

    #[test]
    fn test_record_averages_over_the_time_since_the_process_was_last_read() {
        let mut reads = ProcessReads::default();
        let start = Instant::now();
        reads.record(start, &[time(7, 1, 1_000), time(8, 1, 0)], MAX_CORES);
        record_at(&mut reads, start, 1, &[time(8, 1, 0)]);

        record_at(&mut reads, start, 10, &[time(7, 1, 3_000)]);

        assert!((reads.cores(7) - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_record_keeps_a_load_between_reads() {
        let mut reads = ProcessReads::default();
        let start = Instant::now();
        reads.record(start, &[time(7, 1, 0), time(8, 1, 0)], MAX_CORES);
        record_at(&mut reads, start, 1, &[time(7, 1, 500)]);

        record_at(&mut reads, start, 2, &[time(8, 1, 0)]);

        assert!((reads.cores(7) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_record_counts_a_new_process_from_its_start() {
        let mut reads = ProcessReads::default();
        let start = Instant::now();
        reads.record(start, &[time(7, 1, 0)], MAX_CORES);

        record_at(&mut reads, start, 1, &[time(8, 1, 700)]);

        assert!((reads.cores(8) - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_record_treats_a_reused_pid_as_a_new_process() {
        let mut reads = ProcessReads::default();
        let start = Instant::now();
        reads.record(start, &[time(7, 1, 90_000)], MAX_CORES);

        record_at(&mut reads, start, 1, &[time(7, 2, 300)]);

        assert!((reads.cores(7) - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_record_caps_a_load_at_the_machine() {
        let mut reads = ProcessReads::default();
        let start = Instant::now();
        reads.record(start, &[time(7, 1, 0)], 4.0);

        reads.record(start + Duration::from_secs(1), &[time(7, 1, 99_000)], 4.0);

        assert_eq!(reads.cores(7), 4.0);
    }
}
