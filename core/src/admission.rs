//! How much of the machine is left for new leases, and why a profile does or does not fit.
//!
//! An active lease counts with its reservation from the moment it is granted, and its session
//! counts at the larger of its leases' reservations and what it measures, so a lease that runs
//! over its reservation holds back new grants while use below it gives nothing back. Measured
//! load outside the sessions holding active leases counts as background. A lowered lease counts
//! at its declared cores for its first seconds, before its load shows in the averages.

use std::collections::HashSet;

use allot_shared::types::{
    Accuracy, GpuAdapter, JobProfile, Lease, LeaseId, ResourceUsage, Snapshot,
};

/// How long after its grant a lowered lease counts at its declared cores. Its job's load needs
/// 10 s to fill the averages, and load that runs outside its session's process tree, as in
/// containers, shows only as background, after the same delay.
pub const LOWERED_HOLD_MS: i64 = 15_000;

// Tolerance for float sums of reservations, so 8.0 cores fit in 8.0 cores of room.
const EPSILON: f64 = 1e-6;
// Load below this rounds to 0.0 cores in messages, so it is left out of them.
const SHOWN_CORES: f64 = 0.05;
// Memory below this rounds to 0 MB in messages.
const SHOWN_MB: f64 = 0.5;

/// Resources still free for new leases. Values go negative when the machine is overcommitted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Room {
    /// Logical cores.
    pub cores: f64,
    /// Physical memory.
    pub ram_mb: f64,
    /// GPU engine load on the primary adapter, or `None` without a GPU.
    pub gpu_pct: Option<f64>,
    /// Dedicated video memory on the primary adapter, or `None` when there is no GPU, its
    /// VRAM size is unknown, or it shares RAM.
    pub vram_mb: Option<f64>,
    /// Whether the primary adapter's video memory is RAM, so requested VRAM comes out of
    /// `ram_mb`.
    pub unified_memory: bool,
    /// Cores the reservations taken out of this room hold.
    pub reserved_cores: f64,
    /// How many reservations are taken out of this room.
    pub reservations: u32,
    /// Cores in use outside the sessions holding active leases.
    pub background_cores: f64,
    /// Cores lowered leases hold beyond their reservations in their first seconds.
    pub starting_cores: Tally,
    /// Cores sessions use beyond what their leases count at.
    pub overrun_cores: Tally,
    /// Memory sessions hold beyond their leases' reservations.
    pub overrun_ram_mb: Tally,
}

impl Room {
    /// Usable capacity (after `headroom_pct`) minus background load minus what `active`
    /// leases count at: their reservations, and each [`Extra`] beyond them.
    ///
    /// CPU and GPU use the 10 s averages, for the machine and the leased sessions alike, so a
    /// single spike does not block a grant and a session's load is counted once.
    pub fn measure(snapshot: &Snapshot, active: &[&Lease], headroom_pct: f32) -> Self {
        let keep = 1.0 - f64::from(headroom_pct) / 100.0;
        let leased = Leased::of(snapshot, active);

        let logical_cores = f64::from(snapshot.cpu.logical_cores);
        let machine_cores = f64::from(snapshot.cpu.usage_pct.avg_10s) / 100.0 * logical_cores;
        let memory = &snapshot.memory;
        let held_mb = memory.used_mb.saturating_sub(memory.reclaimable_mb);
        let background_cores = beyond(machine_cores, leased.cores_10s);
        let mut room = Self {
            cores: logical_cores * keep - background_cores,
            ram_mb: memory.total_mb as f64 * keep - beyond(held_mb as f64, leased.held_mb),
            gpu_pct: None,
            vram_mb: None,
            unified_memory: false,
            reserved_cores: 0.0,
            reservations: 0,
            background_cores,
            starting_cores: Tally::default(),
            overrun_cores: Tally::default(),
            overrun_ram_mb: Tally::default(),
        };
        let usage = leased.usage;
        if let Some(gpu) = primary_adapter(&snapshot.gpus) {
            // Where the source cannot measure processes exactly, the leased sessions' own GPU
            // use is not known, so each active lease is taken to use what it reserved.
            let reserved = |pick: fn(&JobProfile) -> f64| -> f64 {
                active.iter().map(|lease| pick(&lease.profile)).sum()
            };
            let leased_gpu_pct = match gpu.process_load {
                Accuracy::Exact => f64::from(usage.gpu_pct),
                _ => reserved(|profile| f64::from(profile.gpu_pct)),
            };
            let leased_vram_mb = match gpu.process_memory {
                Accuracy::Exact => usage.vram_mb as f64,
                _ => reserved(|profile| profile.vram_mb as f64),
            };
            let busiest = f64::from(gpu.engines.busiest(|load| load.avg_10s));
            room.gpu_pct = Some(100.0 * keep - beyond(busiest, leased_gpu_pct));
            // RAM use already counts the video memory of a unified adapter.
            room.unified_memory = gpu.unified_memory;
            room.vram_mb = gpu
                .dedicated_total_mb
                .filter(|_| !gpu.unified_memory)
                .map(|total_mb| {
                    total_mb as f64 * keep - beyond(gpu.dedicated_used_mb as f64, leased_vram_mb)
                });
        }
        for (lease, extra) in active.iter().zip(extras_of(snapshot, active, &leased)) {
            room.reserve(&lease.profile);
            room.count(lease.id, &extra);
        }
        room
    }

    /// Takes `profile` out of the room.
    pub fn reserve(&mut self, profile: &JobProfile) {
        self.shift(profile, -1.0);
    }

    /// Gives `profile` back to the room.
    pub fn release(&mut self, profile: &JobProfile) {
        self.shift(profile, 1.0);
    }

    /// Takes what `lease` counts at beyond its reservation out of the room.
    pub fn count(&mut self, lease: LeaseId, extra: &Extra) {
        self.cores -= extra.starting_cores + extra.overrun_cores;
        self.ram_mb -= extra.overrun_ram_mb;
        self.starting_cores.add(lease, extra.starting_cores);
        self.overrun_cores.add(lease, extra.overrun_cores);
        self.overrun_ram_mb.add(lease, extra.overrun_ram_mb);
    }

    /// Gives back everything `running` takes: its reservation and what it counts at beyond it.
    pub fn end(&mut self, running: &Running) {
        self.release(&running.profile);
        let extra = &running.extra;
        self.cores += extra.starting_cores + extra.overrun_cores;
        self.ram_mb += extra.overrun_ram_mb;
        self.starting_cores.remove(extra.starting_cores);
        self.overrun_cores.remove(extra.overrun_cores);
        self.overrun_ram_mb.remove(extra.overrun_ram_mb);
    }

    fn shift(&mut self, profile: &JobProfile, sign: f64) {
        self.cores += sign * f64::from(profile.cores);
        self.reserved_cores -= sign * f64::from(profile.cores);
        self.reservations = if sign < 0.0 {
            self.reservations.saturating_add(1)
        } else {
            self.reservations.saturating_sub(1)
        };
        self.ram_mb += sign * self.ram_needed_mb(profile);
        if let Some(gpu_pct) = &mut self.gpu_pct {
            *gpu_pct += sign * f64::from(profile.gpu_pct);
        }
        if let Some(vram_mb) = &mut self.vram_mb {
            *vram_mb += sign * profile.vram_mb as f64;
        }
    }

    /// What `asked` is granted in this room: all of it when it fits, else its cores lowered to
    /// what is free, rounded down to a tenth but never below `min_cores`. Only cores shrink;
    /// RAM, GPU and VRAM fit whole or not at all.
    ///
    /// # Errors
    ///
    /// One message per resource the room lacks even at `min_cores`, as from
    /// [`Room::shortfalls`].
    pub fn fit(&self, asked: &JobProfile, min_cores: f32) -> Result<JobProfile, Vec<String>> {
        let min_cores = min_cores.min(asked.cores);
        let reasons = self.shortfalls_down_to(asked, min_cores);
        if !reasons.is_empty() {
            return Err(reasons);
        }
        if !exceeds(f64::from(asked.cores), self.cores) {
            return Ok(*asked);
        }
        // Tenths keep grants readable; the epsilon keeps a free 3.9999 from becoming 3.9.
        let tenths = ((self.cores + EPSILON) * 10.0).floor() / 10.0;
        Ok(JobProfile {
            cores: (tenths as f32).max(min_cores),
            ..*asked
        })
    }

    /// One message per resource `profile` needs more of than the room has; empty means it
    /// fits. VRAM is not checked when the adapter does not report its size.
    pub fn shortfalls(&self, profile: &JobProfile) -> Vec<String> {
        self.shortfalls_down_to(profile, profile.cores)
    }

    /// [`Room::shortfalls`] for a profile whose cores may shrink to `min_cores`.
    fn shortfalls_down_to(&self, profile: &JobProfile, min_cores: f32) -> Vec<String> {
        let mut reasons = Vec::new();
        if exceeds(f64::from(min_cores), self.cores) {
            let at_least = if min_cores < profile.cores {
                format!(" (at least {min_cores:.1})")
            } else {
                String::new()
            };
            reasons.push(format!(
                "cores: {:.1} requested{at_least}, {:.1} free{}",
                profile.cores,
                self.cores.max(0.0),
                self.cores_taken()
            ));
        }
        let ram_needed_mb = self.ram_needed_mb(profile);
        if exceeds(ram_needed_mb, self.ram_mb) {
            let shared = if ram_needed_mb > profile.ram_mb as f64 {
                format!(
                    " ({} MB of it video memory, which shares RAM)",
                    profile.vram_mb
                )
            } else {
                String::new()
            };
            let overrun = match self.overrun_ram_mb.named() {
                Some(by) if self.overrun_ram_mb.amount >= SHOWN_MB => {
                    format!(
                        " ({:.0} MB more in use by {by})",
                        self.overrun_ram_mb.amount
                    )
                }
                _ => String::new(),
            };
            reasons.push(format!(
                "RAM: {ram_needed_mb:.0} MB requested{shared}, {:.0} MB free{overrun}",
                self.ram_mb.max(0.0)
            ));
        }
        let needs_gpu = profile.gpu_pct > 0.0 || profile.vram_mb > 0;
        match self.gpu_pct {
            None if needs_gpu => {
                reasons.push("GPU: requested, but no GPU adapter was found".into())
            }
            Some(free) if exceeds(f64::from(profile.gpu_pct), free) => reasons.push(format!(
                "GPU: {:.0}% requested, {:.0}% free",
                profile.gpu_pct,
                free.max(0.0)
            )),
            _ => {}
        }
        if let Some(free) = self.vram_mb
            && exceeds(profile.vram_mb as f64, free)
        {
            reasons.push(format!(
                "VRAM: {} MB requested, {:.0} MB free",
                profile.vram_mb,
                free.max(0.0)
            ));
        }
        reasons
    }

    /// What takes the cores that are not free, such as " (8.0 held by 1 lease, 1.5 more in use
    /// by lease 42, 5.6 in use outside leases)", or nothing when none does. Load outside leases
    /// can be a leased job's own, when it runs outside its session's process tree, as in
    /// containers.
    fn cores_taken(&self) -> String {
        let mut parts = Vec::new();
        if self.reservations > 0 {
            let leases = if self.reservations == 1 {
                "lease"
            } else {
                "leases"
            };
            parts.push(format!(
                "{:.1} held by {} {leases}",
                self.reserved_cores, self.reservations
            ));
        }
        if let Some(by) = self.starting_cores.named()
            && self.starting_cores.amount >= SHOWN_CORES
        {
            let whose = if self.starting_cores.leases == 1 {
                "its"
            } else {
                "their"
            };
            parts.push(format!(
                "{:.1} more held by {by} for {whose} first {} s",
                self.starting_cores.amount,
                LOWERED_HOLD_MS / 1_000
            ));
        }
        if let Some(by) = self.overrun_cores.named()
            && self.overrun_cores.amount >= SHOWN_CORES
        {
            parts.push(format!(
                "{:.1} more in use by {by}",
                self.overrun_cores.amount
            ));
        }
        if self.background_cores >= SHOWN_CORES {
            parts.push(format!(
                "{:.1} in use outside leases",
                self.background_cores
            ));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!(" ({})", parts.join(", "))
        }
    }

    /// The RAM `profile` takes: its own, plus its video memory when that is RAM.
    fn ram_needed_mb(&self, profile: &JobProfile) -> f64 {
        ram_needed_mb(profile, self.unified_memory)
    }
}

/// Leases counted beyond their reservations for one reason, naming the one counted most.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tally {
    /// The total beyond the reservations, in cores or MB.
    pub amount: f64,
    /// How many leases add to it.
    pub leases: u32,
    /// The lease that added the most, and how much. It stays named while other leases still
    /// add to the tally, even after it ended.
    pub most: Option<(LeaseId, f64)>,
}

impl Tally {
    fn add(&mut self, lease: LeaseId, amount: f64) {
        if amount <= 0.0 {
            return;
        }
        self.amount += amount;
        self.leases = self.leases.saturating_add(1);
        if self.most.is_none_or(|(_, most)| amount > most) {
            self.most = Some((lease, amount));
        }
    }

    fn remove(&mut self, amount: f64) {
        if amount <= 0.0 {
            return;
        }
        self.leases = self.leases.saturating_sub(1);
        if self.leases == 0 {
            *self = Self::default();
        } else {
            self.amount -= amount;
        }
    }

    /// The lease that adds the most, such as "lease 42" or "lease 42 and 2 others"; `None`
    /// when no lease adds anything.
    fn named(&self) -> Option<String> {
        let (lease, _) = self.most?;
        Some(match self.leases.saturating_sub(1) {
            0 => format!("lease {}", lease.0),
            1 => format!("lease {} and 1 other", lease.0),
            others => format!("lease {} and {others} others", lease.0),
        })
    }
}

/// What an active lease counts at beyond its reservation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Extra {
    /// Cores a lowered lease holds up to its declared cores during [`LOWERED_HOLD_MS`].
    pub starting_cores: f64,
    /// Cores its session uses, by the 10 s average, beyond what its leases count at. A session
    /// holding several leases puts this on the one granted last.
    pub overrun_cores: f64,
    /// Memory its session holds beyond its leases' reservations, on the same lease.
    pub overrun_ram_mb: f64,
}

/// What each of `active` counts at beyond its reservation, in the same order.
pub fn extras(snapshot: &Snapshot, active: &[&Lease]) -> Vec<Extra> {
    extras_of(snapshot, active, &Leased::of(snapshot, active))
}

fn extras_of(snapshot: &Snapshot, active: &[&Lease], leased: &Leased) -> Vec<Extra> {
    let unified = primary_adapter(&snapshot.gpus).is_some_and(|gpu| gpu.unified_memory);
    let mut extras: Vec<Extra> = active
        .iter()
        .map(|lease| Extra {
            starting_cores: starting_cores(lease, snapshot.taken_at_ms),
            ..Extra::default()
        })
        .collect();
    for session in &snapshot.sessions {
        let leases: Vec<usize> = (0..active.len())
            .filter(|&index| active[index].holder.session == session.holder.session)
            .collect();
        let Some(&latest) = leases
            .iter()
            .max_by_key(|&&index| (active[index].granted_at_ms, active[index].id))
        else {
            continue;
        };
        let counted_cores: f64 = leases
            .iter()
            .map(|&index| f64::from(active[index].profile.cores) + extras[index].starting_cores)
            .sum();
        let reserved_mb: f64 = leases
            .iter()
            .map(|&index| ram_needed_mb(&active[index].profile, unified))
            .sum();
        extras[latest].overrun_cores = beyond(f64::from(session.cores_10s), counted_cores);
        extras[latest].overrun_ram_mb = beyond(session.usage.ram_mb as f64, reserved_mb);
    }
    // Memory the OS can reclaim is not taken, and a leased session's memory is reclaimable
    // before anyone else's, so overruns count only up to what the leased sessions still hold.
    let overrun_mb: f64 = extras.iter().map(|extra| extra.overrun_ram_mb).sum();
    if overrun_mb > leased.held_mb {
        let scale = leased.held_mb / overrun_mb;
        for extra in &mut extras {
            extra.overrun_ram_mb *= scale;
        }
    }
    extras
}

/// The cores a lowered `lease` holds beyond its reservation at `now_ms`: up to its declared
/// cores during its first [`LOWERED_HOLD_MS`], and none after.
fn starting_cores(lease: &Lease, now_ms: i64) -> f64 {
    let starting = lease
        .granted_at_ms
        .is_none_or(|granted_ms| now_ms.saturating_sub(granted_ms) < LOWERED_HOLD_MS);
    if starting {
        hold_cores(lease.declared_cores, &lease.profile)
    } else {
        0.0
    }
}

/// The cores a lease with `declared_cores` holds beyond `granted` while it starts.
pub fn hold_cores(declared_cores: Option<f32>, granted: &JobProfile) -> f64 {
    declared_cores.map_or(0.0, |declared| {
        beyond(f64::from(declared), f64::from(granted.cores))
    })
}

/// What the sessions holding active leases use together.
struct Leased {
    /// Their latest usage.
    usage: ResourceUsage,
    /// Their cores by the 10 s average.
    cores_10s: f64,
    /// The part of their memory the OS cannot reclaim: their memory counts as reclaimable
    /// before anyone else's, so only what exceeds the reclaimable part is still held.
    held_mb: f64,
}

impl Leased {
    fn of(snapshot: &Snapshot, active: &[&Lease]) -> Self {
        let sessions: HashSet<_> = active.iter().map(|lease| &lease.holder.session).collect();
        let mut usage = ResourceUsage::default();
        let mut cores_10s = 0.0;
        for session in &snapshot.sessions {
            if sessions.contains(&session.holder.session) {
                usage += session.usage;
                cores_10s += f64::from(session.cores_10s);
            }
        }
        Self {
            usage,
            cores_10s,
            held_mb: usage.ram_mb.saturating_sub(snapshot.memory.reclaimable_mb) as f64,
        }
    }
}

fn ram_needed_mb(profile: &JobProfile, unified_memory: bool) -> f64 {
    let vram_mb = if unified_memory { profile.vram_mb } else { 0 };
    profile.ram_mb.saturating_add(vram_mb) as f64
}

/// The adapter leases reserve GPU and VRAM on: the one with the most dedicated memory, so a
/// discrete GPU wins over an integrated one. Ties keep the first adapter.
pub fn primary_adapter(gpus: &[GpuAdapter]) -> Option<&GpuAdapter> {
    gpus.iter().reduce(|best, gpu| {
        if gpu.dedicated_total_mb.unwrap_or(0) > best.dedicated_total_mb.unwrap_or(0) {
            gpu
        } else {
            best
        }
    })
}

/// A reservation and when it is expected to end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Running {
    /// Expected end, in milliseconds since the Unix epoch.
    pub end_ms: i64,
    /// The reservation.
    pub profile: JobProfile,
    /// What it counts at beyond the reservation, taken to last until it ends.
    pub extra: Extra,
}

impl Running {
    /// An active lease, expected to end at its grant time plus its estimated run time.
    pub fn of(lease: &Lease) -> Self {
        let start = lease.granted_at_ms.unwrap_or(lease.requested_at_ms);
        Self::starting(start, &lease.profile, lease.asked_profile().cores)
    }

    /// A reservation of `profile`, granted for a request of `asked_cores`, that starts at
    /// `start_ms`.
    pub fn starting(start_ms: i64, profile: &JobProfile, asked_cores: f32) -> Self {
        Self {
            end_ms: start_ms.saturating_add(est_ms(profile, asked_cores)),
            profile: *profile,
            extra: Extra::default(),
        }
    }
}

/// Each of `active` as a [`Running`] reservation with what it counts at beyond it.
pub fn running(snapshot: &Snapshot, active: &[&Lease]) -> Vec<Running> {
    active
        .iter()
        .zip(extras(snapshot, active))
        .map(|(lease, extra)| Running {
            extra,
            ..Running::of(lease)
        })
        .collect()
}

/// The estimated run time in milliseconds of a reservation of `profile` granted for a request
/// of `asked_cores`. A grant shrunk below its request is taken to run longer in proportion,
/// which overstates it for a job that does not scale, so whatever waits on it starts no later
/// than planned.
pub fn est_ms(profile: &JobProfile, asked_cores: f32) -> i64 {
    let full_ms = i64::try_from(profile.est_secs.saturating_mul(1_000)).unwrap_or(i64::MAX);
    if profile.cores <= 0.0 || asked_cores <= profile.cores {
        return full_ms;
    }
    let stretch = f64::from(asked_cores) / f64::from(profile.cores);
    // The cast saturates, so a huge stretch stays at i64::MAX.
    (full_ms as f64 * stretch).ceil() as i64
}

/// The earliest time `profile` fits, with its cores shrunk down to `min_cores` if need be, as
/// the `running` reservations end in order of their expected end, each giving back what it
/// counts at, with the background unchanged, and the room at that time before `profile` is taken out. `None` when it does
/// not fit even with every reservation ended.
pub fn first_fit(
    mut room: Room,
    running: &[Running],
    profile: &JobProfile,
    min_cores: f32,
    now_ms: i64,
) -> Option<(i64, Room)> {
    if room.fit(profile, min_cores).is_ok() {
        return Some((now_ms, room));
    }
    let mut by_end = running.to_vec();
    by_end.sort_by_key(|reservation| reservation.end_ms);
    for reservation in by_end {
        room.end(&reservation);
        if room.fit(profile, min_cores).is_ok() {
            return Some((reservation.end_ms.max(now_ms), room));
        }
    }
    None
}

/// Seconds until `profile` fits, per [`first_fit`].
pub fn wait_estimate_secs(
    room: Room,
    running: &[Running],
    profile: &JobProfile,
    min_cores: f32,
    now_ms: i64,
) -> Option<u64> {
    first_fit(room, running, profile, min_cores, now_ms)
        .map(|(at_ms, _)| ms_to_secs_ceil(at_ms - now_ms))
}

/// Rounds a duration in milliseconds up to whole seconds; negative durations are 0.
pub fn ms_to_secs_ceil(ms: i64) -> u64 {
    u64::try_from(ms.max(0)).unwrap_or(0).div_ceil(1_000)
}

/// How far `value` goes beyond `base`, or 0.
fn beyond(value: f64, base: f64) -> f64 {
    (value - base).max(0.0)
}

fn exceeds(requested: f64, free: f64) -> bool {
    requested > free + EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{active_lease, gpu, profile, session_usage, snapshot, unified_gpu};

    struct Case {
        name: &'static str,
        snapshot: Snapshot,
        active: Vec<Lease>,
        profile: JobProfile,
        shortfall: Option<&'static str>,
    }

    #[test]
    fn test_room_shortfalls_table() {
        let cases = [
            Case {
                name: "idle machine fits a big build",
                snapshot: snapshot(0.0, 0, None),
                active: vec![],
                profile: profile(12.0, 16_000),
                shortfall: None,
            },
            Case {
                name: "background CPU leaves too few cores",
                snapshot: snapshot(50.0, 0, None),
                active: vec![],
                profile: profile(8.0, 1_000),
                shortfall: Some("cores: 8.0 requested, 6.4 free"),
            },
            Case {
                name: "background RAM leaves too little memory",
                snapshot: snapshot(0.0, 20_000, None),
                active: vec![],
                profile: profile(1.0, 12_000),
                shortfall: Some("RAM: 12000 MB requested"),
            },
            Case {
                name: "reservation of an active lease counts before its load shows",
                snapshot: snapshot(0.0, 0, None),
                active: vec![active_lease(1, "a", profile(8.0, 1_000))],
                profile: profile(8.0, 1_000),
                shortfall: Some("cores: 8.0 requested, 6.4 free"),
            },
            Case {
                name: "load of a leased session is not counted twice",
                snapshot: snapshot_with_session(50.0, "a", 8.0),
                active: vec![active_lease(1, "a", profile(8.0, 1_000))],
                profile: profile(6.0, 1_000),
                shortfall: None,
            },
            Case {
                name: "load of an unleased session is background",
                snapshot: snapshot_with_session(50.0, "b", 8.0),
                active: vec![active_lease(1, "a", profile(1.0, 1_000))],
                profile: profile(6.0, 1_000),
                shortfall: Some("cores: 6.0 requested, 5.4 free"),
            },
            Case {
                name: "GPU request without an adapter",
                snapshot: snapshot(0.0, 0, None),
                active: vec![],
                profile: gpu_profile(50.0, 0),
                shortfall: Some("no GPU adapter"),
            },
            Case {
                name: "busy GPU leaves too little engine room",
                snapshot: snapshot(0.0, 0, Some(gpu(60.0, 1_000, Some(12_288)))),
                active: vec![],
                profile: gpu_profile(50.0, 0),
                shortfall: Some("GPU: 50% requested, 30% free"),
            },
            Case {
                name: "VRAM above the adapter's free memory",
                snapshot: snapshot(0.0, 0, Some(gpu(0.0, 8_000, Some(12_288)))),
                active: vec![],
                profile: gpu_profile(10.0, 4_000),
                shortfall: Some("VRAM: 4000 MB requested"),
            },
            Case {
                name: "unknown VRAM size is not checked",
                snapshot: snapshot(0.0, 0, Some(gpu(0.0, 8_000, None))),
                active: vec![],
                profile: gpu_profile(10.0, 400_000),
                shortfall: None,
            },
            Case {
                name: "unified memory counts VRAM against RAM",
                snapshot: snapshot(0.0, 22_000, Some(unified_gpu(0.0, 4_000))),
                active: vec![],
                profile: gpu_profile(10.0, 8_000),
                shortfall: Some(
                    "RAM: 9000 MB requested (8000 MB of it video memory, which shares RAM)",
                ),
            },
            Case {
                name: "unified memory fits VRAM and RAM together",
                snapshot: snapshot(0.0, 22_000, Some(unified_gpu(0.0, 4_000))),
                active: vec![],
                profile: gpu_profile(10.0, 6_000),
                shortfall: None,
            },
            Case {
                name: "unified memory takes active VRAM reservations out of RAM",
                snapshot: snapshot(0.0, 0, Some(unified_gpu(0.0, 0))),
                active: vec![active_lease(1, "a", gpu_profile(10.0, 23_000))],
                profile: gpu_profile(10.0, 6_000),
                shortfall: Some("RAM: 7000 MB requested"),
            },
        ];
        for case in cases {
            let active: Vec<&Lease> = case.active.iter().collect();
            let room = Room::measure(&case.snapshot, &active, 10.0);
            let reasons = room.shortfalls(&case.profile);
            match case.shortfall {
                None => assert!(reasons.is_empty(), "{}: {reasons:?}", case.name),
                Some(expected) => assert!(
                    reasons.iter().any(|reason| reason.contains(expected)),
                    "{}: expected {expected:?} in {reasons:?}",
                    case.name
                ),
            }
        }
    }

    fn gpu_profile(gpu_pct: f32, vram_mb: u64) -> JobProfile {
        JobProfile {
            gpu_pct,
            vram_mb,
            ..profile(1.0, 1_000)
        }
    }

    fn snapshot_with_session(cpu_pct: f32, session: &str, cores: f32) -> Snapshot {
        let mut snapshot = snapshot(cpu_pct, 0, None);
        snapshot.sessions.push(session_usage(session, cores));
        snapshot
    }

    fn snapshot_with_reclaimable(used_mb: u64, reclaimable_mb: u64) -> Snapshot {
        let mut snapshot = snapshot(0.0, used_mb, None);
        snapshot.memory.reclaimable_mb = reclaimable_mb;
        snapshot
    }

    #[test]
    fn test_cores_shortfall_names_what_holds_the_cores() {
        // Lease 1770: an 8-core lease whose job ran outside its session, at 35% of 16 cores.
        let lease = active_lease(1, "a", profile(8.0, 1_000));
        let room = Room::measure(&snapshot(35.0, 0, None), &[&lease], 15.0);
        assert_eq!(
            room.shortfalls(&profile(8.2, 1_000)),
            ["cores: 8.2 requested, 0.0 free (8.0 held by 1 lease, 5.6 in use outside leases)"]
        );

        let other = active_lease(2, "b", profile(2.0, 1_000));
        let room = Room::measure(&snapshot(0.0, 0, None), &[&lease, &other], 15.0);
        assert_eq!(
            room.shortfalls(&profile(8.0, 1_000)),
            ["cores: 8.0 requested, 3.6 free (10.0 held by 2 leases)"]
        );

        let room = Room::measure(&snapshot(0.0, 0, None), &[], 15.0);
        assert_eq!(
            room.shortfalls(&profile(64.0, 1_000)),
            ["cores: 64.0 requested, 13.6 free"]
        );
    }

    #[test]
    fn test_room_released_reservations_leave_the_message() {
        let lease = active_lease(1, "a", profile(8.0, 1_000));
        let mut room = Room::measure(&snapshot(0.0, 0, None), &[&lease], 15.0);
        room.release(&lease.profile);
        assert_eq!(room.reservations, 0);
        assert_eq!(
            room.shortfalls(&profile(64.0, 1_000)),
            ["cores: 64.0 requested, 13.6 free"]
        );
    }

    #[test]
    fn test_room_does_not_count_reclaimable_memory_as_load() {
        let room = Room::measure(&snapshot_with_reclaimable(30_000, 18_000), &[], 10.0);
        // 29491.2 MB usable, 12000 MB held.
        assert!((room.ram_mb - 17_491.2).abs() < 1e-6, "{room:?}");
    }

    #[test]
    fn test_room_counts_a_leased_session_once_with_reclaimable_memory() {
        let lease = active_lease(1, "a", profile(1.0, 4_000));
        let measure = |session_mb: u64, reclaimable_mb: u64| {
            let mut snapshot = snapshot_with_reclaimable(30_000, reclaimable_mb);
            let mut session = session_usage("a", 0.0);
            session.usage.ram_mb = session_mb;
            snapshot.sessions.push(session);
            Room::measure(&snapshot, &[&lease], 10.0).ram_mb
        };
        // The session's memory is all reclaimable: 12000 MB held by others, 4000 MB reserved.
        assert!((measure(6_000, 18_000) - 13_491.2).abs() < 1e-6);
        // 2000 MB of it is not: 28000 MB held, 2000 MB of that the session's, 4000 MB reserved.
        assert!((measure(4_000, 2_000) + 508.8).abs() < 1e-6);
    }

    #[test]
    fn test_room_on_unified_memory_has_no_separate_vram() {
        let room = Room::measure(&snapshot(0.0, 0, Some(unified_gpu(0.0, 4_000))), &[], 10.0);
        assert!(room.unified_memory);
        assert_eq!(room.vram_mb, None);
        assert!(room.gpu_pct.is_some());

        let discrete = Room::measure(&snapshot(0.0, 0, Some(gpu(0.0, 0, Some(8_192)))), &[], 10.0);
        assert!(!discrete.unified_memory);
        assert!(discrete.vram_mb.is_some());
    }

    #[test]
    fn test_room_without_exact_process_figures_counts_a_gpu_lease_once() {
        // A leased job uses 60% and 3000 MB; the source cannot say which process does.
        let unmeasured = GpuAdapter {
            process_load: Accuracy::Unavailable,
            process_memory: Accuracy::Unavailable,
            ..gpu(60.0, 3_000, Some(8_000))
        };
        let lease = active_lease(1, "a", gpu_profile(50.0, 2_000));
        let room = Room::measure(&snapshot(0.0, 0, Some(unmeasured)), &[&lease], 10.0);
        // 90% usable, 10% of the load beyond the reservation, 50% reserved.
        assert!((room.gpu_pct.unwrap() - 30.0).abs() < 1e-9, "{room:?}");
        // 7200 MB usable, 1000 MB beyond the reservation, 2000 MB reserved.
        assert!((room.vram_mb.unwrap() - 4_200.0).abs() < 1e-9, "{room:?}");

        // With exact figures the same load comes from an unleased process: background.
        let measured = Room::measure(
            &snapshot(0.0, 0, Some(gpu(60.0, 3_000, Some(8_000)))),
            &[&lease],
            10.0,
        );
        assert!(
            (measured.gpu_pct.unwrap() + 20.0).abs() < 1e-9,
            "{measured:?}"
        );
    }

    #[test]
    fn test_room_on_unified_memory_counts_a_gpu_leases_memory_once() {
        // macOS charges GPU allocations to the process footprint, so the leased session's
        // 7000 MB of RAM holds the 6000 MB its job allocated for the GPU.
        let lease = active_lease(1, "a", gpu_profile(50.0, 6_000));
        let mut snapshot = snapshot(0.0, 10_000, Some(unified_gpu(60.0, 6_000)));
        let mut session = session_usage("a", 0.0);
        session.usage.ram_mb = 7_000;
        snapshot.sessions.push(session);
        let room = Room::measure(&snapshot, &[&lease], 10.0);
        // 29491.2 MB usable, 3000 MB of background, 1000 + 6000 MB reserved.
        assert!((room.ram_mb - 19_491.2).abs() < 1e-6, "{room:?}");
        // Estimated process load: 90% usable, 10% beyond the reservation, 50% reserved.
        assert!((room.gpu_pct.unwrap() - 30.0).abs() < 1e-6, "{room:?}");
        assert_eq!(room.vram_mb, None);
    }

    #[test]
    fn test_primary_adapter_prefers_most_vram_then_first() {
        let integrated = gpu(0.0, 0, Some(512));
        let discrete = gpu(0.0, 0, Some(12_288));
        let gpus = [integrated.clone(), discrete.clone(), discrete.clone()];
        assert_eq!(
            primary_adapter(&gpus).map(|g| g.dedicated_total_mb),
            Some(Some(12_288))
        );
        assert!(std::ptr::eq(primary_adapter(&gpus).unwrap(), &gpus[1]));
        assert!(primary_adapter(&[]).is_none());
    }

    #[test]
    fn test_wait_estimate_counts_leases_ending_soonest_first() {
        let snapshot = snapshot(0.0, 0, None);
        let short = active_lease(1, "a", profile(6.0, 1_000));
        let long = JobProfile {
            est_secs: 900,
            ..profile(6.0, 1_000)
        };
        let long = active_lease(2, "b", long);
        let active = [&long, &short];
        let running: Vec<Running> = active.iter().map(|lease| Running::of(lease)).collect();
        let room = Room::measure(&snapshot, &active, 10.0);
        // `active_lease` grants at 0 ms; the short lease is expected to end at 300 s.
        let wait = wait_estimate_secs(room, &running, &profile(8.0, 1_000), 8.0, 100_500);
        assert_eq!(wait, Some(200));
        let (at_ms, room_then) =
            first_fit(room, &running, &profile(8.0, 1_000), 8.0, 100_500).unwrap();
        assert_eq!(at_ms, 300_000);
        assert!((room_then.cores - 8.4).abs() < 1e-3);
    }

    #[test]
    fn test_wait_estimate_impossible_profile_is_none() {
        let snapshot = snapshot(0.0, 0, None);
        let room = Room::measure(&snapshot, &[], 10.0);
        assert_eq!(
            wait_estimate_secs(room, &[], &profile(64.0, 1_000), 64.0, 0),
            None
        );
        assert_eq!(
            wait_estimate_secs(room, &[], &profile(1.0, 1_000), 1.0, 0),
            Some(0)
        );
    }

    #[test]
    fn test_ms_to_secs_ceil_rounds_up_and_clamps() {
        assert_eq!(ms_to_secs_ceil(-5), 0);
        assert_eq!(ms_to_secs_ceil(0), 0);
        assert_eq!(ms_to_secs_ceil(1), 1);
        assert_eq!(ms_to_secs_ceil(2_000), 2);
    }

    /// An idle 16-core room with `cores` free.
    fn room_with(cores: f64) -> Room {
        Room {
            cores,
            ..Room::measure(&snapshot(0.0, 0, None), &[], 10.0)
        }
    }

    #[test]
    fn test_fit_shrinks_cores_to_a_tenth_of_what_is_free() {
        let room = room_with(3.8137);
        assert_eq!(room.fit(&profile(4.0, 1_000), 1.0).unwrap().cores, 3.8);
        assert_eq!(
            room.fit(&profile(3.0, 1_000), 1.0).unwrap(),
            profile(3.0, 1_000)
        );
        // Rounding never pushes a grant below a minimum that fits.
        assert_eq!(room.fit(&profile(4.0, 1_000), 3.81).unwrap().cores, 3.81);
        assert_eq!(
            room_with(3.999_999_9)
                .fit(&profile(4.0, 0), 1.0)
                .unwrap()
                .cores,
            4.0
        );
    }

    #[test]
    fn test_fit_below_the_minimum_names_it() {
        let reasons = room_with(3.8137)
            .fit(&profile(4.0, 1_000), 3.9)
            .unwrap_err();
        assert_eq!(reasons, ["cores: 4.0 requested (at least 3.9), 3.8 free"]);
        let reasons = room_with(3.8137)
            .fit(&profile(4.0, 1_000), 4.0)
            .unwrap_err();
        assert_eq!(reasons, ["cores: 4.0 requested, 3.8 free"]);
    }

    #[test]
    fn test_fit_never_shrinks_memory() {
        let room = Room {
            ram_mb: 512.0,
            ..room_with(2.0)
        };
        let reasons = room.fit(&profile(4.0, 1_024), 1.0).unwrap_err();
        assert_eq!(reasons, ["RAM: 1024 MB requested, 512 MB free"]);
    }

    /// A 16-core machine whose 10 s average is `machine_cores`, with `sessions` given as
    /// (name, 10 s cores, latest cores), at `taken_at_ms`.
    fn busy(machine_cores: f32, sessions: &[(&str, f32, f32)], taken_at_ms: i64) -> Snapshot {
        let mut snapshot = snapshot(machine_cores / 16.0 * 100.0, 0, None);
        snapshot.taken_at_ms = taken_at_ms;
        for &(name, cores_10s, latest) in sessions {
            let mut session = session_usage(name, latest);
            session.cores_10s = cores_10s;
            snapshot.sessions.push(session);
        }
        snapshot
    }

    /// An active lease of `cores` that history lowered from `declared_cores`.
    fn lowered_lease(id: i64, session: &str, cores: f32, declared_cores: f32) -> Lease {
        Lease {
            declared_cores: Some(declared_cores),
            ..active_lease(id, session, profile(cores, 1_000))
        }
    }

    #[test]
    fn test_room_overrun_reduces_room_and_names_the_lease() {
        let lease = active_lease(1, "a", profile(2.0, 1_000));
        let room = Room::measure(&busy(5.5, &[("a", 5.5, 5.5)], 0), &[&lease], 15.0);
        // 13.6 usable, 2.0 reserved, 3.5 beyond the reservation.
        assert!((room.cores - 8.1).abs() < 1e-6, "{room:?}");
        assert_eq!(
            room.shortfalls(&profile(10.0, 1_000)),
            ["cores: 10.0 requested, 8.1 free (2.0 held by 1 lease, 3.5 more in use by lease 1)"]
        );
    }

    #[test]
    fn test_room_use_below_the_reservation_gives_no_room_back() {
        let lease = active_lease(1, "a", profile(8.0, 4_000));
        let mut snapshot = busy(2.0, &[("a", 2.0, 2.0)], 0);
        snapshot.memory.used_mb = 1_000;
        snapshot.sessions[0].usage.ram_mb = 1_000;
        let room = Room::measure(&snapshot, &[&lease], 15.0);
        assert!((room.cores - 5.6).abs() < 1e-6, "{room:?}");
        // 27852.8 MB usable, 4000 MB reserved.
        assert!((room.ram_mb - 23_852.8).abs() < 1e-6, "{room:?}");
        assert_eq!(room.overrun_cores, Tally::default());
        assert_eq!(room.overrun_ram_mb, Tally::default());
    }

    #[test]
    fn test_room_counts_a_session_once_by_its_10_s_average() {
        // The machine's 10 s average holds 6 cores, all the session's; its latest second is
        // nearly idle. Taking that second as its use would count 5 cores as background too.
        let lease = active_lease(1, "a", profile(4.0, 1_000));
        let room = Room::measure(&busy(6.0, &[("a", 6.0, 1.0)], 0), &[&lease], 15.0);
        assert!(room.background_cores.abs() < 1e-6, "{room:?}");
        // 13.6 usable, 4.0 reserved, 2.0 beyond it.
        assert!((room.cores - 7.6).abs() < 1e-6, "{room:?}");

        // Load from another session stays background and does not count as an overrun.
        let room = Room::measure(&busy(6.0, &[("a", 1.0, 1.0)], 0), &[&lease], 15.0);
        assert!((room.background_cores - 5.0).abs() < 1e-6, "{room:?}");
        assert!((room.cores - 4.6).abs() < 1e-6, "{room:?}");
        assert_eq!(room.overrun_cores, Tally::default());
    }

    #[test]
    fn test_room_holds_a_lowered_lease_at_its_declared_cores_while_it_starts() {
        // Lease 2112's pattern: lowered to 0.4 of 8.0 cores, its job's load outside the session.
        let lease = lowered_lease(1, "a", 0.4, 8.0);
        let starting = Room::measure(&busy(0.0, &[("a", 0.0, 0.0)], 14_999), &[&lease], 15.0);
        assert!((starting.cores - 5.6).abs() < 1e-6, "{starting:?}");
        assert_eq!(
            starting.shortfalls(&profile(8.0, 1_000)),
            [
                "cores: 8.0 requested, 5.6 free (0.4 held by 1 lease, 7.6 more held by lease 1 \
                 for its first 15 s)"
            ]
        );

        // After the hold its outside load counts once, as background.
        let started = Room::measure(&busy(8.0, &[("a", 0.0, 0.0)], 15_000), &[&lease], 15.0);
        assert_eq!(started.starting_cores, Tally::default());
        assert!((started.cores - 5.2).abs() < 1e-6, "{started:?}");

        // A lease that history did not lower holds nothing extra.
        let declared = active_lease(2, "b", profile(0.4, 1_000));
        let room = Room::measure(&busy(0.0, &[], 0), &[&declared], 15.0);
        assert!((room.cores - 13.2).abs() < 1e-6, "{room:?}");
    }

    #[test]
    fn test_room_overrun_of_a_held_lease_counts_beyond_its_declared_cores() {
        let lease = lowered_lease(1, "a", 2.0, 4.0);
        let room = Room::measure(&busy(5.0, &[("a", 5.0, 5.0)], 1_000), &[&lease], 15.0);
        // 2.0 reserved, 2.0 held while it starts, 1.0 in use beyond both.
        assert!((room.cores - 8.6).abs() < 1e-6, "{room:?}");
        assert!((room.starting_cores.amount - 2.0).abs() < 1e-6);
        assert!((room.overrun_cores.amount - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_room_puts_a_sessions_overrun_on_its_latest_lease() {
        let first = active_lease(1, "a", profile(2.0, 1_000));
        let second = Lease {
            granted_at_ms: Some(500),
            ..active_lease(2, "a", profile(2.0, 1_000))
        };
        let other = active_lease(3, "b", profile(1.0, 1_000));
        let snapshot = busy(8.0, &[("a", 6.0, 6.0), ("b", 2.0, 2.0)], 1_000);
        let active = [&first, &second, &other];
        assert_eq!(
            extras(&snapshot, &active)
                .iter()
                .map(|extra| extra.overrun_cores)
                .collect::<Vec<_>>(),
            [0.0, 2.0, 1.0]
        );
        let room = Room::measure(&snapshot, &active, 15.0);
        assert_eq!(
            room.shortfalls(&profile(12.0, 1_000)),
            [
                "cores: 12.0 requested, 5.6 free (5.0 held by 3 leases, 3.0 more in use by lease \
                 2 and 1 other)"
            ]
        );
    }

    #[test]
    fn test_room_ram_overrun_counts_only_memory_the_session_holds() {
        let lease = active_lease(1, "a", profile(1.0, 1_000));
        let measure = |reclaimable_mb: u64| {
            let mut snapshot = snapshot_with_reclaimable(3_000, reclaimable_mb);
            let mut session = session_usage("a", 0.0);
            session.usage.ram_mb = 3_000;
            snapshot.sessions.push(session);
            Room::measure(&snapshot, &[&lease], 15.0)
        };
        let room = measure(0);
        // 27852.8 MB usable, 1000 MB reserved, 2000 MB beyond it.
        assert!((room.ram_mb - 24_852.8).abs() < 1e-6, "{room:?}");
        assert_eq!(
            room.shortfalls(&profile(1.0, 26_000)),
            ["RAM: 26000 MB requested, 24853 MB free (2000 MB more in use by lease 1)"]
        );
        // With 2500 MB of it reclaimable the session holds only 500 MB.
        let room = measure(2_500);
        assert!(
            (room.overrun_ram_mb.amount - 500.0).abs() < 1e-6,
            "{room:?}"
        );
    }

    #[test]
    fn test_first_fit_gives_back_an_overrun_when_its_lease_ends() {
        let lease = active_lease(1, "a", profile(4.0, 1_000));
        let snapshot = busy(10.0, &[("a", 10.0, 10.0)], 0);
        let room = Room::measure(&snapshot, &[&lease], 15.0);
        let running = running(&snapshot, &[&lease]);
        assert!((running[0].extra.overrun_cores - 6.0).abs() < 1e-6);
        // 3.6 free now; 13.6 once the lease and its overrun end at 300 s.
        let (at_ms, then) = first_fit(room, &running, &profile(12.0, 1_000), 12.0, 0).unwrap();
        assert_eq!(at_ms, 300_000);
        assert!((then.cores - 13.6).abs() < 1e-6, "{then:?}");
        assert_eq!(then.overrun_cores, Tally::default());
    }

    #[test]
    fn test_est_ms_stretches_a_shrunk_grant() {
        let granted = JobProfile {
            est_secs: 100,
            ..profile(4.0, 0)
        };
        assert_eq!(est_ms(&granted, 4.0), 100_000);
        assert_eq!(est_ms(&granted, 8.0), 200_000);
        assert_eq!(est_ms(&granted, 2.0), 100_000);
    }
}
