//! Golden files for the tray app: the JSON Schemas of [`UiState`] and of the kill and remove
//! responses, one fixture per flyout scenario and response shape, kept in `contract/` at the
//! repository root, and the C# types generated from the schemas in the tray's logic library.
//!
//! The tests fail when a file no longer matches the types. Rerun them with
//! `ALLOT_UPDATE_CONTRACT=1` to rewrite the files, then review the diff.

mod csharp;

use std::{env, fs, path::PathBuf};

use schemars::{Schema, generate::SchemaSettings};
use serde::Serialize;

use crate::{
    api::{
        BenchmarkProgress, HealthResponse, KillResponse, KilledProcess, RemoveResponse,
        UnkilledProcess, Verdict,
    },
    types::{
        Accuracy, BoundProcess, DriveUsage, EndCause, EnforcedLimits, Holder, IsolationKind,
        JobClass, JobProfile, Lease, LeaseBinding, LeaseId, LeaseIsolation, LeaseKind, LeaseState,
        NoiseReport, ResourceUsage, SessionKey,
    },
    ui::{
        ActiveJob, CpuMeter, EndedLease, GpuMeter, JobProcess, Meters, Notice, QueuedJob, RamMeter,
        RecentRun, UiSession, UiState,
    },
};

const UPDATE_VAR: &str = "ALLOT_UPDATE_CONTRACT";
const SCHEMA_FILE: &str = "contract/ui-state.schema.json";
const KILL_SCHEMA_FILE: &str = "contract/kill-response.schema.json";
const REMOVE_SCHEMA_FILE: &str = "contract/remove-response.schema.json";
const CSHARP_FILE: &str = "tray/src/Allot.Tray.Core/Contract/UiState.g.cs";

// 2026-10-24 06:52 UTC; every scenario is built relative to it.
const NOW_MS: i64 = 1_792_824_720_000;
const MINUTE_MS: i64 = 60_000;

const TRAINER: &str = "b81d44f0-5a3e-4d2b-9c61-0e7f2a9d3c15";
const APP: &str = "3f2a9c1e-77b0-4f6e-a2d8-5c9e1b4f7a02";
const IDLE: &str = "7c19e2aa-0d4b-4c8f-b3e5-9a6f2d1c8e47";

fn repo_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Compares `actual` with the committed file at `relative` to the repository root, or rewrites
/// the file when updating.
fn assert_matches_file(relative: &str, actual: &str) {
    let path = repo_dir().join(relative);
    if env::var_os(UPDATE_VAR).is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {}: {error}; rerun with {UPDATE_VAR}=1 to create it",
            path.display()
        )
    });
    assert!(
        expected == actual,
        "{} is out of date; rerun with {UPDATE_VAR}=1 and review the diff",
        path.display()
    );
}

fn pretty_json<T: Serialize>(value: &T) -> String {
    let mut json = serde_json::to_string_pretty(value).unwrap();
    json.push('\n');
    json
}

fn holder(session: &str, pid: u32) -> Holder {
    Holder {
        session: SessionKey(session.into()),
        pid,
        pid_start_time: 1_792_820_000,
    }
}

fn profile(cores: f32, ram_mb: u64, gpu_pct: f32, vram_mb: u64, est_secs: u64) -> JobProfile {
    JobProfile {
        cores,
        ram_mb,
        gpu_pct,
        vram_mb,
        est_secs,
    }
}

fn usage(cores: f32, ram_mb: u64, gpu_pct: f32, vram_mb: u64) -> ResourceUsage {
    ResourceUsage {
        cores,
        ram_mb,
        gpu_pct,
        vram_mb,
    }
}

/// An active `cpu_heavy` lease granted `granted_ago_ms` before [`NOW_MS`].
fn active_lease(id: i64, holder: Holder, label: &str, cwd: &str, granted_ago_ms: i64) -> Lease {
    let granted_at_ms = NOW_MS - granted_ago_ms;
    Lease {
        id: LeaseId(id),
        holder,
        kind: LeaseKind::Normal,
        state: LeaseState::Active,
        class: Some(JobClass::CpuHeavy),
        profile: profile(8.0, 4096, 0.0, 0, 300),
        label: label.into(),
        cwd: cwd.into(),
        requested_at_ms: granted_at_ms - 1_500,
        granted_at_ms: Some(granted_at_ms),
        ended_at_ms: None,
        expires_at_ms: NOW_MS + 45_000,
        end_cause: None,
        binding: None,
        isolation: None,
    }
}

/// A queued lease requested `waited_ms` before [`NOW_MS`].
fn queued_lease(id: i64, holder: Holder, label: &str, cwd: &str, waited_ms: i64) -> Lease {
    Lease {
        state: LeaseState::Queued,
        requested_at_ms: NOW_MS - waited_ms,
        granted_at_ms: None,
        ..active_lease(id, holder, label, cwd, 0)
    }
}

fn waiting(reasons: &[&str], wait_estimate_secs: Option<u64>) -> Verdict {
    Verdict {
        fits: false,
        reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
        wait_estimate_secs,
        benchmark: None,
    }
}

fn service() -> HealthResponse {
    HealthResponse {
        version: "0.2.0".into(),
        pid: 58_260,
        uptime_secs: 7_380,
        config_error: None,
    }
}

fn session(key: &str, cwd: Option<&str>, usage: ResourceUsage) -> UiSession {
    UiSession {
        session: SessionKey(key.into()),
        cwd: cwd.map(Into::into),
        usage,
    }
}

fn drive(name: &str, total_mb: u64, used_mb: u64, read: u64, write: u64) -> DriveUsage {
    DriveUsage {
        name: name.into(),
        total_mb,
        used_mb,
        read_bytes_per_sec: read,
        write_bytes_per_sec: write,
    }
}

fn meters(usage_pct: f32, reserved_cores: f32, reserved_mb: u64, gpus: Vec<GpuMeter>) -> Meters {
    Meters {
        headroom_pct: 15.0,
        cpu: CpuMeter {
            logical_cores: 16,
            usage_pct,
            history_pct: vec![48.5, 52.0, 61.25, 64.0, 58.75, 63.5, usage_pct],
            reserved_cores,
        },
        ram: RamMeter {
            total_mb: 32_768,
            used_mb: 22_938,
            reserved_mb,
        },
        gpus,
        drives: vec![
            drive("C:", 975_872, 626_688, 44_040_192, 8_388_608),
            drive("E:", 1_907_712, 1_258_291, 0, 0),
        ],
    }
}

fn rtx(busiest_pct: f32, reserved_pct: f32, reserved_vram_mb: u64) -> GpuMeter {
    GpuMeter {
        name: "NVIDIA GeForce RTX 2080 Ti".into(),
        busiest_pct,
        vram_used_mb: 6_656,
        vram_total_mb: Some(11_264),
        reserved_pct,
        reserved_vram_mb,
        unified_memory: false,
        process_load: Accuracy::Exact,
    }
}

/// Two running jobs (one bound, one shown by its busiest process), two waiting requests
/// (estimate known and unknown), an idle session, and a kill Claude has not heard about yet.
pub(crate) fn busy() -> UiState {
    let build = active_lease(
        41,
        holder(APP, 18_240),
        "cargo build --release",
        r"E:\work\app",
        186_000,
    );
    let train = Lease {
        class: Some(JobClass::GpuHeavy),
        profile: profile(2.0, 4096, 80.0, 4096, 1_800),
        ..active_lease(
            42,
            holder(TRAINER, 22_016),
            "python train.py --epochs 40",
            r"E:\work\ml-trainer",
            900_000,
        )
    };
    let npm = Lease {
        class: Some(JobClass::Light),
        profile: profile(1.0, 1024, 0.0, 0, 120),
        ..queued_lease(
            43,
            holder(TRAINER, 22_016),
            "npm test",
            r"E:\work\ml-trainer",
            42_000,
        )
    };
    let test = Lease {
        class: None,
        profile: profile(12.0, 8192, 0.0, 0, 240),
        ..queued_lease(
            44,
            holder(APP, 18_240),
            "cargo test --workspace",
            r"E:\work\app",
            18_000,
        )
    };
    UiState {
        generated_at_ms: NOW_MS,
        service: service(),
        first_session_at_ms: Some(NOW_MS - 3 * 24 * 60 * MINUTE_MS),
        sessions: vec![
            session(
                TRAINER,
                Some(r"E:\work\ml-trainer"),
                usage(2.5, 6_144, 78.0, 4_352),
            ),
            session(APP, Some(r"E:\work\app"), usage(7.75, 2_970, 0.0, 0)),
            session(IDLE, None, usage(0.25, 512, 0.0, 0)),
        ],
        active: vec![
            ActiveJob {
                lease: train,
                process: Some(JobProcess {
                    pid: 30_412,
                    name: "python.exe".into(),
                    exe: None,
                    bound: false,
                }),
                quiet_at_start: None,
            },
            ActiveJob {
                lease: build,
                process: Some(JobProcess {
                    pid: 18_244,
                    name: "cargo.exe".into(),
                    exe: Some(r"C:\Users\Mehran\.cargo\bin\cargo.exe".into()),
                    bound: true,
                }),
                quiet_at_start: None,
            },
        ],
        queue: vec![
            QueuedJob {
                lease: npm,
                position: 1,
                verdict: waiting(
                    &["CPU: 1 core requested, 0.4 free after headroom"],
                    Some(120),
                ),
            },
            QueuedJob {
                lease: test,
                position: 2,
                verdict: waiting(
                    &[
                        "CPU: 12 cores requested, 0.4 free after headroom",
                        "1 lease(s) queued ahead",
                    ],
                    None,
                ),
            },
        ],
        meters: meters(64.0, 10.0, 8_192, vec![rtx(78.0, 80.0, 4_096)]),
        recent: Vec::new(),
        notices: vec![Notice {
            lease_id: LeaseId(39),
            session: SessionKey(APP.into()),
            cause: EndCause::Killed,
            command: "cargo clippy --workspace".into(),
            tool_use_id: Some("toolu_01FixtureBusyNotice".into()),
            ended_at_ms: NOW_MS - 20_000,
            delivered_at_ms: None,
        }],
        ended: vec![
            EndedLease {
                lease_id: LeaseId(40),
                session: SessionKey(TRAINER.into()),
                command: "ruff check .".into(),
                cause: EndCause::Released,
                reason: "released by holder".into(),
                granted_at_ms: Some(NOW_MS - 14_000),
                ended_at_ms: NOW_MS - 2_000,
            },
            EndedLease {
                lease_id: LeaseId(39),
                session: SessionKey(APP.into()),
                command: "cargo clippy --workspace".into(),
                cause: EndCause::Killed,
                reason: "the user stopped this job in Allot".into(),
                granted_at_ms: Some(NOW_MS - 95_000),
                ended_at_ms: NOW_MS - 20_000,
            },
        ],
    }
}

/// A benchmark measuring background load before it runs alone, a job queued behind it, and a
/// second GPU.
fn benchmark() -> UiState {
    let bench = Lease {
        kind: LeaseKind::BenchmarkExclusive,
        profile: profile(16.0, 4096, 0.0, 0, 120),
        ..queued_lease(
            51,
            holder(APP, 18_240),
            "cargo bench --bench parser",
            r"E:\work\app",
            72_000,
        )
    };
    let after = queued_lease(
        52,
        holder(TRAINER, 22_016),
        "cargo build",
        r"E:\work\tools",
        30_000,
    );
    let integrated = GpuMeter {
        name: "Intel(R) UHD Graphics 770".into(),
        busiest_pct: 3.0,
        vram_used_mb: 0,
        vram_total_mb: None,
        reserved_pct: 0.0,
        reserved_vram_mb: 0,
        unified_memory: false,
        process_load: Accuracy::Exact,
    };
    UiState {
        generated_at_ms: NOW_MS,
        service: service(),
        first_session_at_ms: Some(NOW_MS - 3 * 24 * 60 * MINUTE_MS),
        sessions: vec![
            session(APP, Some(r"E:\work\app"), usage(0.5, 1_024, 0.0, 0)),
            session(TRAINER, Some(r"E:\work\tools"), usage(0.25, 768, 0.0, 0)),
        ],
        active: Vec::new(),
        queue: vec![
            QueuedJob {
                lease: bench,
                position: 1,
                verdict: Verdict {
                    benchmark: Some(BenchmarkProgress::Measuring {
                        measured_secs: 6,
                        needed_secs: 10,
                        cpu_avg_pct: 3.0,
                        gpu_avg_pct: 0.5,
                    }),
                    ..waiting(&["benchmark: measuring background load (6 of 10 s)"], None)
                },
            },
            QueuedJob {
                lease: after,
                position: 2,
                verdict: waiting(&["1 lease(s) queued ahead"], None),
            },
        ],
        meters: meters(3.5, 0.0, 0, vec![rtx(1.0, 0.0, 0), integrated]),
        recent: Vec::new(),
        notices: Vec::new(),
        ended: Vec::new(),
    }
}

fn recent_run(
    id: i64,
    session: &str,
    command: &str,
    ended_ago_ms: i64,
    end_cause: EndCause,
) -> RecentRun {
    let ended_at_ms = NOW_MS - ended_ago_ms;
    RecentRun {
        lease_id: Some(LeaseId(id)),
        session: Some(SessionKey(session.into())),
        command: command.into(),
        cwd: r"E:\work\app".into(),
        declared: profile(8.0, 4096, 0.0, 0, 300),
        started_at_ms: ended_at_ms - 292_000,
        ended_at_ms,
        end_cause: Some(end_cause),
        noise: None,
    }
}

/// No jobs: idle sessions and recently finished runs, including a killed one, a benchmark
/// that was granted without quiet, and one recorded before sessions and causes were.
fn idle() -> UiState {
    UiState {
        generated_at_ms: NOW_MS,
        service: service(),
        first_session_at_ms: Some(NOW_MS - 3 * 24 * 60 * MINUTE_MS),
        sessions: vec![
            session(APP, Some(r"E:\work\app"), usage(0.25, 640, 0.0, 0)),
            session(TRAINER, None, usage(0.0, 512, 0.0, 0)),
        ],
        active: Vec::new(),
        queue: Vec::new(),
        meters: meters(4.0, 0.0, 0, vec![rtx(1.0, 0.0, 0)]),
        recent: vec![
            recent_run(
                41,
                APP,
                "cargo build --release",
                4 * MINUTE_MS,
                EndCause::Released,
            ),
            recent_run(
                39,
                APP,
                "cargo clippy --workspace",
                9 * MINUTE_MS,
                EndCause::Killed,
            ),
            RecentRun {
                declared: profile(16.0, 4096, 0.0, 0, 120),
                noise: Some(NoiseReport {
                    avg_cpu_pct: 1.75,
                    peak_cpu_pct: 6.25,
                    avg_gpu_pct: 0.0,
                    peak_gpu_pct: 0.5,
                    quiet_at_start: false,
                }),
                ..recent_run(
                    37,
                    TRAINER,
                    "cargo bench --bench parser",
                    25 * MINUTE_MS,
                    EndCause::Released,
                )
            },
            RecentRun {
                lease_id: None,
                session: None,
                end_cause: None,
                ..recent_run(
                    12,
                    APP,
                    "cargo test",
                    3 * 60 * MINUTE_MS,
                    EndCause::Released,
                )
            },
        ],
        notices: vec![Notice {
            lease_id: LeaseId(39),
            session: SessionKey(APP.into()),
            cause: EndCause::Killed,
            command: "cargo clippy --workspace".into(),
            tool_use_id: None,
            ended_at_ms: NOW_MS - 9 * MINUTE_MS,
            delivered_at_ms: Some(NOW_MS - 8 * MINUTE_MS),
        }],
        ended: Vec::new(),
    }
}

/// A fresh install: no session has ever registered, no GPU, and no drive data yet.
fn first_run() -> UiState {
    let mut meters = meters(2.5, 0.0, 0, Vec::new());
    meters.cpu.history_pct = vec![2.5];
    meters.drives.clear();
    UiState {
        generated_at_ms: NOW_MS,
        service: HealthResponse {
            uptime_secs: 4,
            ..service()
        },
        first_session_at_ms: None,
        sessions: Vec::new(),
        active: Vec::new(),
        queue: Vec::new(),
        meters,
        recent: Vec::new(),
        notices: Vec::new(),
        ended: Vec::new(),
    }
}

/// A Linux desktop: a build in its own cgroup scope with caps, a training run followed as a
/// process tree where scopes were unavailable, a queued request, and drives by mount point.
fn linux() -> UiState {
    let build = Lease {
        isolation: Some(LeaseIsolation {
            kind: IsolationKind::Cgroup,
            scope: Some("allot-lease-61.scope".into()),
            limits: EnforcedLimits {
                cpu_cores: Some(8.0),
                memory_high_mb: Some(4096),
                memory_max_mb: None,
            },
        }),
        ..active_lease(
            61,
            holder(APP, 4_120),
            "cargo build --release",
            "/home/mehran/work/app",
            48_000,
        )
    };
    let train = Lease {
        class: Some(JobClass::GpuHeavy),
        profile: profile(2.0, 4096, 60.0, 2048, 1_800),
        isolation: Some(LeaseIsolation {
            kind: IsolationKind::Tree,
            scope: None,
            limits: EnforcedLimits::default(),
        }),
        ..active_lease(
            62,
            holder(TRAINER, 4_388),
            "python train.py --epochs 40",
            "/home/mehran/work/ml-trainer",
            600_000,
        )
    };
    let test = queued_lease(
        63,
        holder(APP, 4_120),
        "cargo test --workspace",
        "/home/mehran/work/app",
        12_000,
    );
    let mut meters = meters(71.5, 10.0, 8_192, Vec::new());
    meters.gpus = vec![GpuMeter {
        name: "NVIDIA GeForce GTX 970".into(),
        busiest_pct: 57.0,
        vram_used_mb: 2_304,
        vram_total_mb: Some(4_096),
        reserved_pct: 60.0,
        reserved_vram_mb: 2_048,
        unified_memory: false,
        process_load: Accuracy::Exact,
    }];
    meters.drives = vec![
        drive("/", 102_400, 61_440, 12_582_912, 33_554_432),
        drive("/home", 819_200, 409_600, 0, 4_194_304),
    ];
    UiState {
        generated_at_ms: NOW_MS,
        service: service(),
        first_session_at_ms: Some(NOW_MS - 2 * 60 * MINUTE_MS),
        sessions: vec![
            session(
                APP,
                Some("/home/mehran/work/app"),
                usage(7.5, 3_328, 0.0, 0),
            ),
            session(
                TRAINER,
                Some("/home/mehran/work/ml-trainer"),
                usage(1.75, 5_120, 55.0, 1_920),
            ),
        ],
        active: vec![
            ActiveJob {
                lease: train,
                process: Some(JobProcess {
                    pid: 4_502,
                    name: "python3".into(),
                    exe: Some("/usr/bin/python3.13".into()),
                    bound: true,
                }),
                quiet_at_start: None,
            },
            ActiveJob {
                lease: build,
                process: Some(JobProcess {
                    pid: 4_131,
                    name: "rustc".into(),
                    exe: Some("/home/mehran/.rustup/toolchains/stable/bin/rustc".into()),
                    bound: true,
                }),
                quiet_at_start: None,
            },
        ],
        queue: vec![QueuedJob {
            lease: test,
            position: 1,
            verdict: waiting(&["cores: 8.0 requested, 3.6 free"], Some(252)),
        }],
        meters,
        recent: Vec::new(),
        notices: Vec::new(),
        ended: Vec::new(),
    }
}

/// An Apple Silicon Mac: one GPU whose memory is RAM, with a training run's video memory
/// reserved out of it, estimated per-job GPU load, and a request waiting for shared memory.
fn macos() -> UiState {
    let train = Lease {
        class: Some(JobClass::GpuHeavy),
        profile: profile(2.0, 4096, 60.0, 8_192, 1_800),
        isolation: Some(LeaseIsolation {
            kind: IsolationKind::Tree,
            scope: None,
            limits: EnforcedLimits::default(),
        }),
        ..active_lease(
            71,
            holder(TRAINER, 912),
            "python train.py --device mps",
            "/Users/mehran/work/ml-trainer",
            300_000,
        )
    };
    let render = Lease {
        class: Some(JobClass::Mixed),
        profile: profile(6.0, 8192, 50.0, 16_384, 600),
        ..queued_lease(
            72,
            holder(APP, 1_204),
            "blender -b scene.blend -a",
            "/Users/mehran/work/app",
            30_000,
        )
    };
    let mut meters = meters(38.0, 2.0, 4_096, Vec::new());
    meters.cpu.logical_cores = 14;
    meters.ram = RamMeter {
        total_mb: 36_864,
        used_mb: 24_576,
        reserved_mb: 4_096,
    };
    meters.gpus = vec![GpuMeter {
        name: "Apple M3 Max".into(),
        busiest_pct: 64.0,
        vram_used_mb: 9_216,
        vram_total_mb: None,
        reserved_pct: 60.0,
        reserved_vram_mb: 8_192,
        unified_memory: true,
        process_load: Accuracy::Estimated,
    }];
    meters.drives = vec![
        drive("/", 999_424, 614_400, 3_145_728, 1_048_576),
        drive("/Volumes/Backup", 1_907_712, 1_048_576, 0, 0),
    ];
    UiState {
        generated_at_ms: NOW_MS,
        service: service(),
        first_session_at_ms: Some(NOW_MS - 24 * 60 * MINUTE_MS),
        sessions: vec![
            session(
                TRAINER,
                Some("/Users/mehran/work/ml-trainer"),
                usage(1.5, 3_584, 58.5, 0),
            ),
            session(
                APP,
                Some("/Users/mehran/work/app"),
                usage(0.25, 768, 0.0, 0),
            ),
        ],
        active: vec![ActiveJob {
            lease: train,
            process: Some(JobProcess {
                pid: 2_210,
                name: "python3.13".into(),
                exe: Some("/opt/homebrew/bin/python3.13".into()),
                bound: true,
            }),
            quiet_at_start: None,
        }],
        queue: vec![QueuedJob {
            lease: render,
            position: 1,
            verdict: waiting(
                &[
                    "RAM: 24576 MB requested (16384 MB of it video memory, which shares RAM), \
                     2560 MB free",
                ],
                Some(270),
            ),
        }],
        meters,
        recent: Vec::new(),
        notices: Vec::new(),
        ended: Vec::new(),
    }
}

fn scenarios() -> [(&'static str, UiState); 6] {
    [
        ("busy", busy()),
        ("benchmark", benchmark()),
        ("idle", idle()),
        ("first-run", first_run()),
        ("linux", linux()),
        ("macos", macos()),
    ]
}

/// The lease of a `cargo clippy` the user killed 20 s before [`NOW_MS`], bound to its shell.
fn killed_lease() -> Lease {
    Lease {
        state: LeaseState::Expired,
        ended_at_ms: Some(NOW_MS - 20_000),
        end_cause: Some(EndCause::Killed),
        binding: Some(LeaseBinding {
            tool_use_id: "toolu_01Kq7ZpX3mV9".into(),
            bound_at_ms: NOW_MS - 95_000,
            root: Some(BoundProcess {
                pid: 31_020,
                start_time: 1_792_824_625,
            }),
        }),
        ..active_lease(
            39,
            holder(APP, 18_240),
            "cargo clippy --workspace",
            r"E:\work\app",
            95_000,
        )
    }
}

fn killed(pid: u32, name: &str) -> KilledProcess {
    KilledProcess {
        pid,
        name: name.into(),
    }
}

/// The bound tree terminated with a nested session spared and one process that refused, and a
/// lease that had no bound tree.
fn kill_scenarios() -> [(&'static str, KillResponse); 2] {
    let tree = KillResponse {
        lease: killed_lease(),
        terminated: vec![
            killed(31_020, "bash.exe"),
            killed(31_024, "bash.exe"),
            killed(31_032, "cargo.exe"),
            killed(31_077, "rustc.exe"),
        ],
        failed: vec![UnkilledProcess {
            pid: 31_090,
            name: "link.exe".into(),
            reason: "Access is denied. (0x80070005)".into(),
        }],
        spared: vec![UnkilledProcess {
            pid: 31_101,
            name: "claude.exe".into(),
            reason: "protected".into(),
        }],
        untouched_reason: None,
    };
    let untouched = KillResponse {
        lease: Lease {
            binding: None,
            ..killed_lease()
        },
        terminated: Vec::new(),
        failed: Vec::new(),
        spared: Vec::new(),
        untouched_reason: Some("the lease has no bound process tree".into()),
    };
    [("tree", tree), ("untouched", untouched)]
}

/// A queued `cargo test` the user removed at [`NOW_MS`].
fn remove_scenario() -> RemoveResponse {
    let queued = queued_lease(
        44,
        holder(APP, 18_240),
        "cargo test --workspace",
        r"E:\work\app",
        18_000,
    );
    RemoveResponse {
        lease: Lease {
            state: LeaseState::Expired,
            ended_at_ms: Some(NOW_MS),
            end_cause: Some(EndCause::Removed),
            ..queued
        },
    }
}

fn root_schema<T: schemars::JsonSchema>() -> Schema {
    SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<T>()
}

/// Every root the tray decodes, with its committed schema file; `UiState` first.
fn contract_schemas() -> [(&'static str, Schema); 3] {
    [
        (SCHEMA_FILE, root_schema::<UiState>()),
        (KILL_SCHEMA_FILE, root_schema::<KillResponse>()),
        (REMOVE_SCHEMA_FILE, root_schema::<RemoveResponse>()),
    ]
}

#[test]
fn test_contract_schemas_match_committed_files() {
    for (file, schema) in contract_schemas() {
        assert_matches_file(file, &pretty_json(&schema));
    }
}

#[test]
fn test_csharp_types_match_committed_file() {
    let schemas = contract_schemas();
    let roots: Vec<&serde_json::Value> = schemas
        .iter()
        .map(|(_, schema)| schema.as_value())
        .collect();
    assert_matches_file(CSHARP_FILE, &csharp::emit(&roots));
}

#[test]
fn test_response_fixtures_match_committed_files() {
    for (name, response) in kill_scenarios() {
        assert_matches_file(
            &format!("contract/fixtures/kill-response-{name}.json"),
            &pretty_json(&response),
        );
    }
    assert_matches_file(
        "contract/fixtures/remove-response.json",
        &pretty_json(&remove_scenario()),
    );
}

#[test]
fn test_ui_state_fixtures_match_committed_files() {
    for (name, state) in scenarios() {
        assert_matches_file(
            &format!("contract/fixtures/ui-state-{name}.json"),
            &pretty_json(&state),
        );
    }
}

#[test]
fn test_ui_state_fixtures_decode_to_their_scenarios() {
    for (name, state) in scenarios() {
        let json = pretty_json(&state);
        let parsed: UiState = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed, state,
            "scenario {name} changed in a JSON round trip"
        );
    }
}
