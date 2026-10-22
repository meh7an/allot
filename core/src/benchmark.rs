//! Benchmark support: when a drained machine counts as quiet, and how much load from outside
//! the benchmark's session disturbed it while it ran.
//!
//! Quiet is judged on the average over a window rather than on every single second, because
//! an idle desktop still has one-second spikes; requiring each second to stay under the limit
//! restarted the wait forever on a machine idling just below it.

use std::collections::VecDeque;

use rh_shared::types::{LeaseId, NoiseReport, ResourceUsage, SessionKey, Snapshot};

use crate::admission;

/// When a drained machine counts as quiet, with times in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuietLimits {
    /// Average total CPU load at or below which the machine is quiet.
    pub cpu_pct: f32,
    /// Average load of the busiest GPU engine at or below which the machine is quiet.
    pub gpu_pct: f32,
    /// The window the loads are averaged over, counted from when no lease is active.
    pub window_ms: i64,
    /// How long a waiting benchmark may find the drained machine above the limits before it
    /// is granted anyway.
    pub max_wait_ms: i64,
}

/// Average loads over the quiet window, in percent.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Average {
    /// Total CPU load.
    pub cpu_pct: f32,
    /// Load of the busiest GPU engine.
    pub gpu_pct: f32,
}

/// What the quiet window says about starting a benchmark now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Quiet {
    /// The window is not full yet.
    Measuring {
        /// How much of the window has passed since the machine drained.
        measured_ms: i64,
        /// How much is left.
        left_ms: i64,
        /// The average so far.
        load: Average,
    },
    /// The window is full and its average is within the limits.
    Quiet(Average),
    /// The window is full but above the limits; the benchmark is granted anyway in `left_ms`.
    Noisy {
        /// The window's average.
        load: Average,
        /// Time until the benchmark is granted without quiet.
        left_ms: i64,
    },
    /// Above the limits for the longest wait: the benchmark starts anyway.
    Overdue(Average),
}

/// Machine load sampled since the last time no lease was active.
#[derive(Debug, Clone, Default)]
pub struct QuietWindow {
    drained_since_ms: Option<i64>,
    last_taken_at_ms: Option<i64>,
    samples: VecDeque<Load>,
}

#[derive(Debug, Clone, Copy)]
struct Load {
    taken_at_ms: i64,
    cpu_pct: f32,
    gpu_pct: f32,
}

impl QuietWindow {
    /// Adds `snapshot` while no lease is active (`drained`); an active lease empties the
    /// window, so it only ever holds load from a drained machine.
    pub fn observe(&mut self, now_ms: i64, snapshot: &Snapshot, drained: bool, window_ms: i64) {
        if !drained {
            *self = Self::default();
            return;
        }
        let since = *self.drained_since_ms.get_or_insert(now_ms);
        // Several ticks can see the same sample, and a sample taken before the drain still
        // carries the ended leases' load.
        let taken_at_ms = snapshot.taken_at_ms;
        let seen = self
            .last_taken_at_ms
            .is_some_and(|last| taken_at_ms <= last);
        if taken_at_ms < since || seen {
            return;
        }
        self.last_taken_at_ms = Some(taken_at_ms);
        self.samples.push_back(Load {
            taken_at_ms,
            cpu_pct: snapshot.cpu.usage_pct.avg_1s,
            gpu_pct: busiest_gpu_pct(snapshot),
        });
        while self
            .samples
            .front()
            .is_some_and(|load| load.taken_at_ms <= taken_at_ms - window_ms)
        {
            self.samples.pop_front();
        }
    }

    /// Whether a benchmark requested at `requested_at_ms` may start at `now_ms`.
    pub fn status(&self, now_ms: i64, requested_at_ms: i64, limits: &QuietLimits) -> Quiet {
        let load = self.average();
        let measured_ms = self
            .drained_since_ms
            .map_or(0, |since| (now_ms - since).clamp(0, limits.window_ms));
        let Some(since) = self.drained_since_ms else {
            return Quiet::Measuring {
                measured_ms,
                left_ms: limits.window_ms,
                load,
            };
        };
        if measured_ms < limits.window_ms || self.samples.is_empty() {
            return Quiet::Measuring {
                measured_ms,
                left_ms: limits.window_ms - measured_ms,
                load,
            };
        }
        if load.cpu_pct <= limits.cpu_pct && load.gpu_pct <= limits.gpu_pct {
            return Quiet::Quiet(load);
        }
        let waited_ms = now_ms - since.max(requested_at_ms);
        let left_ms = limits.max_wait_ms - waited_ms;
        if left_ms > 0 {
            Quiet::Noisy { load, left_ms }
        } else {
            Quiet::Overdue(load)
        }
    }

    fn average(&self) -> Average {
        let count = self.samples.len();
        if count == 0 {
            return Average::default();
        }
        let (cpu, gpu) = self.samples.iter().fold((0.0, 0.0), |(cpu, gpu), load| {
            (cpu + f64::from(load.cpu_pct), gpu + f64::from(load.gpu_pct))
        });
        Average {
            cpu_pct: (cpu / count as f64) as f32,
            gpu_pct: (gpu / count as f64) as f32,
        }
    }
}

/// Load of the busiest engine of any adapter over the last second.
fn busiest_gpu_pct(snapshot: &Snapshot) -> f32 {
    snapshot
        .gpus
        .iter()
        .map(|gpu| admission::busiest_engine_pct(&gpu.engines, |load| load.avg_1s))
        .fold(0.0, f32::max)
}

/// What a finished benchmark lease measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BenchmarkRun {
    /// Load from outside the holder's session.
    pub noise: NoiseReport,
    /// Peak usage of the holder's session, per resource.
    pub holder_peak: ResourceUsage,
    /// Average usage of the holder's session.
    pub holder_avg: ResourceUsage,
}

/// Accumulates load samples while a benchmark lease is active.
#[derive(Debug, Clone)]
pub struct RunMeter {
    lease_id: LeaseId,
    session: SessionKey,
    quiet_at_start: bool,
    last_taken_at_ms: i64,
    samples: u32,
    noise_cpu: Stat,
    noise_gpu: Stat,
    holder_sum: [f64; 4],
    holder_peak: ResourceUsage,
}

#[derive(Debug, Clone, Copy, Default)]
struct Stat {
    sum: f64,
    peak: f32,
}

impl Stat {
    fn add(&mut self, value: f32) {
        self.sum += f64::from(value);
        self.peak = self.peak.max(value);
    }

    fn mean(self, count: u32) -> f32 {
        (self.sum / f64::from(count)) as f32
    }
}

impl RunMeter {
    /// Starts measuring lease `lease_id` of `session`, granted when `granted_snapshot` was
    /// the latest sample. That sample predates the grant, so it is not counted.
    pub fn start(
        lease_id: LeaseId,
        session: SessionKey,
        quiet_at_start: bool,
        granted_snapshot: &Snapshot,
    ) -> Self {
        Self {
            lease_id,
            session,
            quiet_at_start,
            last_taken_at_ms: granted_snapshot.taken_at_ms,
            samples: 0,
            noise_cpu: Stat::default(),
            noise_gpu: Stat::default(),
            holder_sum: [0.0; 4],
            holder_peak: ResourceUsage::default(),
        }
    }

    /// The lease being measured.
    pub fn lease_id(&self) -> LeaseId {
        self.lease_id
    }

    /// Adds `snapshot` unless it was already counted.
    pub fn observe(&mut self, snapshot: &Snapshot) {
        if snapshot.taken_at_ms <= self.last_taken_at_ms {
            return;
        }
        self.last_taken_at_ms = snapshot.taken_at_ms;
        self.samples += 1;

        let mut others = snapshot.unattributed;
        let mut holder = ResourceUsage::default();
        for session in &snapshot.sessions {
            if session.holder.session == self.session {
                holder += session.usage;
            } else {
                others += session.usage;
            }
        }
        let logical_cores = snapshot.cpu.logical_cores.max(1) as f32;
        self.noise_cpu.add(others.cores / logical_cores * 100.0);
        self.noise_gpu.add(others.gpu_pct.min(100.0));

        let values = [
            f64::from(holder.cores),
            holder.ram_mb as f64,
            f64::from(holder.gpu_pct),
            holder.vram_mb as f64,
        ];
        for (sum, value) in self.holder_sum.iter_mut().zip(values) {
            *sum += value;
        }
        let peak = &mut self.holder_peak;
        peak.cores = peak.cores.max(holder.cores);
        peak.ram_mb = peak.ram_mb.max(holder.ram_mb);
        peak.gpu_pct = peak.gpu_pct.max(holder.gpu_pct);
        peak.vram_mb = peak.vram_mb.max(holder.vram_mb);
    }

    /// The measurements, or `None` when no sample arrived during the lease.
    pub fn finish(&self) -> Option<BenchmarkRun> {
        let count = self.samples;
        if count == 0 {
            return None;
        }
        let mean = |sum: f64| sum / f64::from(count);
        let [cores, ram_mb, gpu_pct, vram_mb] = self.holder_sum.map(mean);
        Some(BenchmarkRun {
            noise: NoiseReport {
                avg_cpu_pct: self.noise_cpu.mean(count),
                peak_cpu_pct: self.noise_cpu.peak,
                avg_gpu_pct: self.noise_gpu.mean(count),
                peak_gpu_pct: self.noise_gpu.peak,
                quiet_at_start: self.quiet_at_start,
            },
            holder_peak: self.holder_peak,
            holder_avg: ResourceUsage {
                cores: cores as f32,
                ram_mb: ram_mb.round() as u64,
                gpu_pct: gpu_pct as f32,
                vram_mb: vram_mb.round() as u64,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{gpu, session_usage, snapshot};

    const LIMITS: QuietLimits = QuietLimits {
        cpu_pct: 10.0,
        gpu_pct: 10.0,
        window_ms: 10_000,
        max_wait_ms: 60_000,
    };

    fn at(taken_at_ms: i64, cpu_pct: f32) -> Snapshot {
        Snapshot {
            taken_at_ms,
            ..snapshot(cpu_pct, 0, None)
        }
    }

    /// Feeds one sample per second from `from_s` to `to_s` inclusive, all at `cpu_pct`.
    fn feed(window: &mut QuietWindow, from_s: i64, to_s: i64, cpu_pct: f32) {
        for second in from_s..=to_s {
            let now_ms = second * 1_000;
            window.observe(now_ms, &at(now_ms, cpu_pct), true, LIMITS.window_ms);
        }
    }

    #[test]
    fn test_quiet_window_one_second_spike_does_not_restart_the_wait() {
        let mut window = QuietWindow::default();
        feed(&mut window, 0, 4, 7.0);
        feed(&mut window, 5, 5, 30.0);
        feed(&mut window, 6, 10, 7.0);
        // Ten samples (1..=10 s) averaging 9.3%.
        let status = window.status(10_000, 0, &LIMITS);
        assert!(
            matches!(status, Quiet::Quiet(load) if load.cpu_pct < 10.0),
            "{status:?}"
        );
    }

    #[test]
    fn test_quiet_window_measures_until_the_window_is_full() {
        let mut window = QuietWindow::default();
        feed(&mut window, 0, 6, 2.0);
        let status = window.status(6_000, 0, &LIMITS);
        let expected = Quiet::Measuring {
            measured_ms: 6_000,
            left_ms: 4_000,
            load: Average {
                cpu_pct: 2.0,
                gpu_pct: 0.0,
            },
        };
        assert_eq!(status, expected);
    }

    #[test]
    fn test_quiet_window_noisy_machine_is_granted_after_the_longest_wait() {
        let mut window = QuietWindow::default();
        feed(&mut window, 0, 20, 15.0);
        let status = window.status(20_000, 5_000, &LIMITS);
        assert!(
            matches!(
                status,
                Quiet::Noisy {
                    left_ms: 45_000,
                    ..
                }
            ),
            "{status:?}"
        );
        feed(&mut window, 21, 65, 15.0);
        assert!(matches!(
            window.status(65_000, 5_000, &LIMITS),
            Quiet::Overdue(_)
        ));
    }

    #[test]
    fn test_quiet_window_ignores_repeated_and_pre_drain_samples() {
        let mut window = QuietWindow::default();
        // Drained at 5 s, but the latest sample is from 4 s, while leases still ran.
        window.observe(5_000, &at(4_000, 90.0), true, LIMITS.window_ms);
        feed(&mut window, 6, 15, 3.0);
        window.observe(15_500, &at(15_000, 90.0), true, LIMITS.window_ms);
        let status = window.status(15_000, 0, &LIMITS);
        assert!(
            matches!(status, Quiet::Quiet(load) if load.cpu_pct == 3.0),
            "{status:?}"
        );
    }

    #[test]
    fn test_quiet_window_active_lease_empties_it() {
        let mut window = QuietWindow::default();
        feed(&mut window, 0, 10, 1.0);
        window.observe(11_000, &at(11_000, 1.0), false, LIMITS.window_ms);
        feed(&mut window, 12, 13, 1.0);
        let status = window.status(13_000, 0, &LIMITS);
        assert!(
            matches!(
                status,
                Quiet::Measuring {
                    measured_ms: 1_000,
                    ..
                }
            ),
            "{status:?}"
        );
    }

    #[test]
    fn test_quiet_window_counts_the_busiest_gpu_engine() {
        let mut window = QuietWindow::default();
        for second in 0..=10 {
            let sample = Snapshot {
                taken_at_ms: second * 1_000,
                ..snapshot(1.0, 0, Some(gpu(40.0, 0, None)))
            };
            window.observe(second * 1_000, &sample, true, LIMITS.window_ms);
        }
        let status = window.status(10_000, 0, &LIMITS);
        assert!(matches!(status, Quiet::Noisy { load, .. } if load.gpu_pct == 40.0));
    }

    #[test]
    fn test_run_meter_separates_holder_load_from_noise() {
        let granted = at(0, 0.0);
        let mut meter = RunMeter::start(LeaseId(4), SessionKey("bench".into()), true, &granted);
        meter.observe(&granted);
        assert_eq!(meter.finish(), None);

        for (second, other_cores) in [(1, 0.8), (2, 0.0), (2, 9.0)] {
            let mut sample = at(second * 1_000, 50.0);
            sample.sessions = vec![
                session_usage("bench", 8.0),
                session_usage("other", other_cores),
            ];
            sample.unattributed.cores = 0.16;
            meter.observe(&sample);
        }
        let run = meter.finish().unwrap();
        // 16 cores: (0.8 + 0.16) / 16 = 6%, then 0.16 / 16 = 1%; the repeat at 2 s is ignored.
        assert!((run.noise.avg_cpu_pct - 3.5).abs() < 1e-4, "{run:?}");
        assert!((run.noise.peak_cpu_pct - 6.0).abs() < 1e-4, "{run:?}");
        assert!(run.noise.quiet_at_start);
        assert_eq!(run.holder_peak.cores, 8.0);
        assert_eq!(run.holder_avg.cores, 8.0);
        assert_eq!(meter.lease_id(), LeaseId(4));
    }
}
