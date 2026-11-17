//! History-based estimates: which past runs resemble a request, and what they used.
//!
//! A request's estimate is the declared profile raised to the p90 of its similar runs. Memory
//! uses each run's peak because it must fit at its peak. CPU and GPU use each run's average
//! while its session was busy: a build briefly touches every core, and reserving its one-second
//! peak would make it wait for an idle machine, while the average over the whole lease counts
//! the idle time around the command. Runs recorded before busy load was measured use their
//! whole-lease average. Raised values stop at what the machine can free, so a learned profile
//! never turns into a request that can never start.
//!
//! With enough runs, a value the runs stayed well under also lowers, to what they used plus a
//! margin ([`Lowering`]). RAM lowers to the highest peak instead of the p90: a clean build of
//! the same command can need several times the RAM of an incremental one, and running out of
//! memory crashes a job where too few cores only slow it. Run time never lowers.

use allot_shared::{
    api::JobSummary,
    config::EstimatesConfig,
    platform::{PATH_STYLE, PathStyle},
    types::{EndCause, JobProfile, JobRun, LOW_IMPACT_MAX_RAM_MB, LeaseKind},
};

use crate::{
    admission::{self, Room},
    classifier::{Classification, Classifier, Shell},
};

/// Most runs an estimate looks at, newest first.
pub const ESTIMATE_RUNS: usize = 20;

/// Fewest cores an estimate lowers to, so a lease always holds a share of a core.
pub const FLOOR_CORES: f32 = 0.25;

/// Least RAM an estimate lowers to, in MB.
pub const FLOOR_RAM_MB: u64 = 256;

/// What the similar runs of a request needed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Learned {
    /// p90 of peak RAM and VRAM, of busy cores and GPU load, and of run time.
    pub profile: JobProfile,
    /// The highest of each value over the same runs.
    pub max: JobProfile,
    /// Runs the values come from.
    pub runs: usize,
}

/// How far an estimate may lower a request below what it declares.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lowering {
    /// Clean runs needed before any value lowers.
    pub min_runs: usize,
    /// Added on top of what the runs used.
    pub margin_pct: f32,
    /// The minimum the request itself named. A class or preset's default minimum does not
    /// bound lowering: it is a guess for the class, while the runs measured this job.
    pub min_cores: Option<f32>,
}

impl Lowering {
    /// Lowering as `config` sets it for a request of `kind` that named `min_cores`, or `None`
    /// when the config turns it off or the request is a benchmark, whose measurements must
    /// stay comparable.
    pub fn for_request(
        config: &EstimatesConfig,
        kind: LeaseKind,
        min_cores: Option<f32>,
    ) -> Option<Self> {
        (config.lower && kind != LeaseKind::BenchmarkExclusive).then(|| Self {
            min_runs: usize::try_from(config.min_runs).unwrap_or(usize::MAX),
            margin_pct: config.margin_pct,
            min_cores,
        })
    }
}

/// A request's profile after history, with one reason per value history changed.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    /// The profile to reserve.
    pub profile: JobProfile,
    /// Where the values come from; empty without history.
    pub reasons: Vec<String>,
}

/// The key similar runs share: the normalized command, `|`, and the normalized working
/// directory.
///
/// The command is the first heavy part of `label` as the classifier sees it, so redirects,
/// pipes into `tail`, and a leading `cd` do not change it. A label with no heavy part is
/// used whole. Words are joined by single spaces and the program is lowercased on every OS;
/// the directory is spelled and compared as the OS does it ([`PATH_STYLE`]).
pub fn fingerprint(classifier: &Classifier, label: &str, cwd: &str) -> String {
    fingerprint_in(PATH_STYLE, classifier, label, cwd)
}

fn fingerprint_in(style: PathStyle, classifier: &Classifier, label: &str, cwd: &str) -> String {
    format!(
        "{}|{}",
        normalize_command(classifier, label),
        normalize_cwd(style, cwd)
    )
}

fn normalize_command(classifier: &Classifier, label: &str) -> String {
    let heavy = [Shell::Bash, Shell::PowerShell]
        .into_iter()
        .find_map(|shell| match classifier.classify(label, shell) {
            Classification::Heavy { command, .. } => Some(command),
            _ => None,
        });
    let command = heavy.as_deref().unwrap_or(label);
    let mut words = command.split_whitespace();
    let Some(program) = words.next() else {
        return String::new();
    };
    let mut normalized = program.to_lowercase();
    while let Some(word) = words.next() {
        match redirect(word) {
            Some(Redirect::Bare) => {
                words.next();
            }
            Some(Redirect::WithTarget) => {}
            None => {
                normalized.push(' ');
                normalized.push_str(word);
            }
        }
    }
    normalized
}

enum Redirect {
    /// `>`, `2>>`: the target is the next word.
    Bare,
    /// `2>&1`, `>log.txt`.
    WithTarget,
}

/// Whether `word` redirects output or input; where it goes does not change the job.
fn redirect(word: &str) -> Option<Redirect> {
    let operator = word.trim_start_matches(|c: char| c.is_ascii_digit() || c == '*' || c == '&');
    if !operator.starts_with(['>', '<']) {
        return None;
    }
    if operator.trim_start_matches(['>', '<']).is_empty() {
        Some(Redirect::Bare)
    } else {
        Some(Redirect::WithTarget)
    }
}

/// The directory as `style` compares it, without a trailing separator.
fn normalize_cwd(style: PathStyle, cwd: &str) -> String {
    let cwd = cwd.trim();
    match style {
        PathStyle::Windows => normalize_windows_cwd(cwd),
        PathStyle::Macos | PathStyle::Linux => {
            let trimmed = cwd.trim_end_matches('/');
            let path = if trimmed.is_empty() && cwd.starts_with('/') {
                "/"
            } else {
                trimmed
            };
            if style.ignores_case() {
                path.to_lowercase()
            } else {
                path.to_owned()
            }
        }
    }
}

/// Lowercase with backslashes and no trailing separator; Git Bash paths such as `/e/work`
/// become `e:\work`.
fn normalize_windows_cwd(cwd: &str) -> String {
    let mut path = cwd.replace('/', "\\").to_lowercase();
    let bytes = path.as_bytes();
    let msys_drive = bytes.first() == Some(&b'\\')
        && bytes.get(1).is_some_and(u8::is_ascii_alphabetic)
        && matches!(bytes.get(2), None | Some(b'\\'));
    if msys_drive {
        let drive = char::from(bytes[1]);
        path = format!("{drive}:{}", &path[2..]);
    }
    while path.ends_with('\\') && !path.ends_with(":\\") && path.len() > 1 {
        path.pop();
    }
    if path.len() == 2 && path.ends_with(':') {
        path.push('\\');
    }
    path
}

/// What up to [`ESTIMATE_RUNS`] of `runs` (newest first) needed, skipping runs that
/// overlapped another lease of their session and runs the user killed, which stopped short of
/// what the job needs. `None` when no run is left.
pub fn learned(runs: &[JobRun]) -> Option<Learned> {
    let clean: Vec<&JobRun> = runs
        .iter()
        .filter(|run| !run.overlapped && run.end_cause != Some(EndCause::Killed))
        .take(ESTIMATE_RUNS)
        .collect();
    if clean.is_empty() {
        return None;
    }
    Some(Learned {
        profile: profile_of(&clean, p90),
        max: profile_of(&clean, max),
        runs: clean.len(),
    })
}

/// `pick` applied to each value of the non-empty `runs`.
fn profile_of(runs: &[&JobRun], pick: fn(Vec<f64>) -> f64) -> JobProfile {
    let pick_of = |value: fn(&JobRun) -> f64| pick(runs.iter().map(|run| value(run)).collect());
    JobProfile {
        cores: pick_of(|run| f64::from(run.busy.map_or(run.avg.cores, |busy| busy.cores))) as f32,
        ram_mb: pick_of(|run| run.peak.ram_mb as f64).round() as u64,
        gpu_pct: (pick_of(|run| f64::from(run.busy.map_or(run.avg.gpu_pct, |busy| busy.gpu_pct)))
            as f32)
            .min(100.0),
        vram_mb: pick_of(|run| run.peak.vram_mb as f64).round() as u64,
        est_secs: pick_of(|run| {
            admission::ms_to_secs_ceil(run.ended_at_ms - run.started_at_ms) as f64
        }) as u64,
    }
}

/// Nearest-rank 90th percentile of a non-empty list.
fn p90(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let rank = (values.len() * 9).div_ceil(10).max(1);
    values[rank - 1]
}

/// Largest value of a non-empty list.
fn max(values: Vec<f64>) -> f64 {
    values.into_iter().fold(f64::MIN, f64::max)
}

/// Raises `declared` to `learned` value by value, each capped at what `ceiling` (the room
/// with every active lease ended) can hold. With `lowering` and at least its `min_runs` runs,
/// a value the runs stayed under also lowers to what they used plus the margin, never below
/// the floors ([`FLOOR_CORES`], [`FLOOR_RAM_MB`]) or the minimum the request named. GPU load
/// and VRAM lower only when some run measured them, since a platform that cannot attribute
/// them to a session reports zero. Run time only raises.
pub fn estimate(
    declared: &JobProfile,
    learned: Option<&Learned>,
    ceiling: &Room,
    lowering: Option<&Lowering>,
) -> Estimate {
    let Some(learned) = learned else {
        return Estimate {
            profile: *declared,
            reasons: Vec::new(),
        };
    };
    let runs = learned.runs;
    let lowers = lowering.filter(|lowering| runs >= lowering.min_runs);
    let margin_pct = lowers.map_or(0.0, |lowering| lowering.margin_pct);
    let (used, most) = (learned.profile, learned.max);
    let lower_to = |used: f64, source, bound, per_unit| {
        lowers.map(|_| Lower {
            used,
            source,
            bound,
            per_unit,
        })
    };
    let cores_bound = match lowers.and_then(|lowering| lowering.min_cores) {
        Some(named) if named > FLOOR_CORES => Bound {
            value: f64::from(named),
            is: "the minimum the request named",
        },
        _ => Bound::floor(f64::from(FLOOR_CORES)),
    };

    let cores = Value {
        declared: f64::from(declared.cores),
        p90: f64::from(used.cores),
        cap: Some(ceiling.cores),
        lower: lower_to(f64::from(used.cores), P90, cores_bound, 10.0),
    }
    .change(margin_pct);
    let gpu = Value {
        declared: f64::from(declared.gpu_pct),
        p90: f64::from(used.gpu_pct),
        cap: ceiling.gpu_pct,
        lower: lower_to(f64::from(used.gpu_pct), P90, Bound::floor(0.0), 1.0)
            .filter(|_| most.gpu_pct > 0.0),
    }
    .change(margin_pct);
    let vram = Value {
        declared: declared.vram_mb as f64,
        p90: used.vram_mb as f64,
        cap: ceiling.vram_mb,
        lower: lower_to(used.vram_mb as f64, P90, Bound::floor(0.0), 1.0)
            .filter(|_| most.vram_mb > 0),
    }
    .change(margin_pct);
    let est = Value {
        declared: declared.est_secs as f64,
        p90: used.est_secs as f64,
        cap: None,
        lower: None,
    }
    .change(margin_pct);

    let mut profile = *declared;
    if let Some(cores) = cores {
        profile.cores = cores.value() as f32;
    }
    if let Some(gpu) = gpu {
        profile.gpu_pct = gpu.value() as f32;
    }
    if let Some(vram) = vram {
        profile.vram_mb = vram.value().round() as u64;
    }
    if let Some(est) = est {
        profile.est_secs = est.value() as u64;
    }
    // A low-impact lease runs beside a benchmark. A job that only looks that small because its
    // work runs outside its session (in containers, for one) must not, so lowering never makes
    // a request low impact; RAM is the value held back, since a megabyte costs nothing.
    let at_low_impact_ram = JobProfile {
        ram_mb: LOW_IMPACT_MAX_RAM_MB,
        ..profile
    };
    let ram_bound = if !declared.is_low_impact() && at_low_impact_ram.is_low_impact() {
        Bound {
            value: (LOW_IMPACT_MAX_RAM_MB + 1) as f64,
            is: "the least that keeps it out of low impact",
        }
    } else {
        Bound::floor(FLOOR_RAM_MB as f64)
    };
    let ram = Value {
        declared: declared.ram_mb as f64,
        p90: used.ram_mb as f64,
        cap: Some(ceiling.ram_mb),
        lower: lower_to(most.ram_mb as f64, HIGHEST_PEAK, ram_bound, 1.0),
    }
    .change(margin_pct);
    if let Some(ram) = ram {
        profile.ram_mb = ram.value().round() as u64;
    }

    let show_mb = |value: f64| format!("{value:.0} MB");
    let changes: [(&str, Option<Change>, Show); 5] = [
        ("cores", cores, &show_cores),
        ("RAM", ram, &show_mb),
        ("GPU", gpu, &|value| format!("{value:.0}%")),
        ("VRAM", vram, &show_mb),
        ("run time", est, &|value| format!("{value:.0}s")),
    ];
    let mut reasons: Vec<String> = changes
        .into_iter()
        .filter_map(|(name, change, show)| Some(change?.reason(name, runs, margin_pct, show)))
        .collect();
    if reasons.is_empty() {
        let stayed = format!("{runs} past run(s) stayed within the declared profile");
        reasons.push(match lowering {
            Some(lowering) if runs < lowering.min_runs => {
                format!("{stayed}; lowering waits for {} runs", lowering.min_runs)
            }
            _ => stayed,
        });
    }
    Estimate { profile, reasons }
}

/// Cores with one decimal, or two when the second is not zero, so the 0.25 floor reads as
/// itself.
fn show_cores(cores: f64) -> String {
    let two = format!("{cores:.2}");
    match two.strip_suffix('0') {
        Some(one) => one.to_owned(),
        None => two,
    }
}

/// One summary per fingerprint in `runs` (newest first), in the order each was last run.
pub fn summarize(runs: &[JobRun]) -> Vec<JobSummary> {
    let mut groups: Vec<(&str, Vec<JobRun>)> = Vec::new();
    for run in runs {
        match groups
            .iter_mut()
            .find(|(fingerprint, _)| *fingerprint == run.fingerprint)
        {
            Some((_, group)) => group.push(run.clone()),
            None => groups.push((run.fingerprint.as_str(), vec![run.clone()])),
        }
    }
    groups
        .into_iter()
        .map(|(fingerprint, group)| {
            let learned = learned(&group);
            let newest = &group[0];
            JobSummary {
                fingerprint: fingerprint.to_owned(),
                command: newest.command.clone(),
                cwd: newest.cwd.clone(),
                runs: u32::try_from(group.len()).unwrap_or(u32::MAX),
                estimate_runs: learned
                    .map_or(0, |learned| u32::try_from(learned.runs).unwrap_or(u32::MAX)),
                last_ended_at_ms: newest.ended_at_ms,
                learned: learned.map(|learned| learned.profile),
            }
        })
        .collect()
}

/// Names of what a lowered value comes from, for its reason.
const P90: &str = "p90";
const HIGHEST_PEAK: &str = "the highest peak";

/// How a value reads in a reason.
type Show<'a> = &'a dyn Fn(f64) -> String;

/// Slack below a rounding step, so float error such as 1.1000000000000001 does not round a
/// lowered value up by a whole step.
const ROUNDING_SLACK: f64 = 1e-6;

/// One value of a profile as history sees it.
#[derive(Debug, Clone, Copy)]
struct Value {
    declared: f64,
    /// p90 of the runs, which raising aims for.
    p90: f64,
    /// Most raising may reach.
    cap: Option<f64>,
    /// Where lowering aims, or `None` when this value does not lower.
    lower: Option<Lower>,
}

/// Where a value lowers to.
#[derive(Debug, Clone, Copy)]
struct Lower {
    /// What the runs used, before the margin.
    used: f64,
    /// Which statistic `used` is.
    source: &'static str,
    bound: Bound,
    /// Lowered values round up to a multiple of `1 / per_unit`.
    per_unit: f64,
}

/// The least a value lowers to, and what it is.
#[derive(Debug, Clone, Copy)]
struct Bound {
    value: f64,
    is: &'static str,
}

impl Bound {
    fn floor(value: f64) -> Self {
        Self {
            value,
            is: "the floor",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Change {
    Raised(Raised),
    Lowered(Lowered),
}

#[derive(Debug, Clone, Copy)]
struct Raised {
    value: f64,
    declared: f64,
    p90: f64,
    capped: bool,
}

#[derive(Debug, Clone, Copy)]
struct Lowered {
    value: f64,
    declared: f64,
    used: f64,
    source: &'static str,
    /// What stopped the value, when a bound did rather than the runs.
    bounded_by: Option<&'static str>,
}

impl Value {
    /// How history changes this value: raised to the p90 above it, else lowered to what the
    /// runs used plus `margin_pct` when that is below it.
    fn change(&self, margin_pct: f32) -> Option<Change> {
        if let Some(raised) = raise(self.declared, self.p90, self.cap) {
            return Some(Change::Raised(raised));
        }
        let lower = self.lower?;
        let wanted = round_up(
            lower.used * (1.0 + f64::from(margin_pct) / 100.0),
            lower.per_unit,
        );
        let value = wanted.max(lower.bound.value);
        (value < self.declared).then_some(Change::Lowered(Lowered {
            value,
            declared: self.declared,
            used: lower.used,
            source: lower.source,
            bounded_by: (wanted < lower.bound.value).then_some(lower.bound.is),
        }))
    }
}

impl Change {
    fn value(&self) -> f64 {
        match self {
            Self::Raised(raised) => raised.value,
            Self::Lowered(lowered) => lowered.value,
        }
    }

    fn reason(&self, name: &str, runs: usize, margin_pct: f32, show: Show) -> String {
        match self {
            Self::Raised(raised) => {
                let (value, declared, p90) =
                    (show(raised.value), show(raised.declared), show(raised.p90));
                if raised.capped {
                    format!(
                        "{name} {value}, the most the machine can free \
                         (p90 of {runs} past run(s): {p90}, declared {declared})"
                    )
                } else {
                    format!("{name} {value} from p90 of {runs} past run(s) (declared {declared})")
                }
            }
            Self::Lowered(lowered) => {
                let (value, declared, used) = (
                    show(lowered.value),
                    show(lowered.declared),
                    show(lowered.used),
                );
                let source = lowered.source;
                match lowered.bounded_by {
                    Some(bound) => format!(
                        "{name} {value}, {bound} \
                         ({source} of {runs} past run(s): {used}, declared {declared})"
                    ),
                    None => format!(
                        "{name} {value} from {source} of {runs} past run(s) plus {margin_pct}% \
                         (declared {declared})"
                    ),
                }
            }
        }
    }
}

/// `value` rounded up to a multiple of `1 / per_unit`.
fn round_up(value: f64, per_unit: f64) -> f64 {
    (value * per_unit - ROUNDING_SLACK).ceil() / per_unit
}

/// The raised value when `p90` is above `declared`; it stops at `cap` but never drops below
/// `declared`.
fn raise(declared: f64, p90: f64, cap: Option<f64>) -> Option<Raised> {
    if p90 <= declared {
        return None;
    }
    let (value, capped) = match cap {
        Some(cap) if p90 > cap => (cap.max(declared), true),
        _ => (p90, false),
    };
    Some(Raised {
        value,
        declared,
        p90,
        capped,
    })
}

#[cfg(test)]
mod tests {
    use allot_shared::{
        config::ClassifierConfig,
        types::{BusyUsage, JobProfile, ResourceUsage},
    };

    use super::*;

    fn classifier() -> Classifier {
        Classifier::new(&ClassifierConfig::default())
    }

    fn declared() -> JobProfile {
        JobProfile {
            cores: 8.0,
            ram_mb: 4096,
            gpu_pct: 0.0,
            vram_mb: 0,
            est_secs: 300,
        }
    }

    fn roomy() -> Room {
        Room {
            cores: 13.6,
            ram_mb: 27_000.0,
            gpu_pct: Some(85.0),
            vram_mb: Some(9_000.0),
            unified_memory: false,
            reserved_cores: 0.0,
            reservations: 0,
            background_cores: 0.0,
            starting_cores: admission::Tally::default(),
            overrun_cores: admission::Tally::default(),
            overrun_ram_mb: admission::Tally::default(),
        }
    }

    /// A run that averaged `avg_cores`, peaked at `peak_ram_mb`, and took `secs`.
    fn run(avg_cores: f32, peak_ram_mb: u64, secs: i64) -> JobRun {
        JobRun {
            lease_id: None,
            session: None,
            end_cause: None,
            fingerprint: "cargo build|e:\\app".into(),
            command: "cargo build".into(),
            cwd: "E:\\app".into(),
            declared: declared(),
            peak: ResourceUsage {
                cores: 16.0,
                ram_mb: peak_ram_mb,
                gpu_pct: 0.0,
                vram_mb: 0,
            },
            avg: ResourceUsage {
                cores: avg_cores,
                ram_mb: peak_ram_mb / 2,
                gpu_pct: 0.0,
                vram_mb: 0,
            },
            busy: None,
            started_at_ms: 1_000,
            ended_at_ms: 1_000 + secs * 1_000,
            noise: None,
            overlapped: false,
        }
    }

    #[test]
    fn test_fingerprint_ignores_wrappers_redirects_and_path_spelling() {
        let classifier = classifier();
        let plain = windows_fingerprint(&classifier, "cargo build -p allotd", r"E:\Work\App");
        assert_eq!(plain, r"cargo build -p allotd|e:\work\app");
        for (label, cwd) in [
            (
                "cd sub && cargo build -p allotd 2>&1 | tail -5",
                "E:/Work/App/",
            ),
            ("Cargo  build   -p allotd", "/e/work/app"),
            ("cargo build -p allotd; echo done", r"e:\work\app\"),
            (
                "cargo build -p allotd > build.log 2> err.log",
                r"e:\work\app",
            ),
            ("cargo build -p allotd *>&1", r"e:\work\app"),
        ] {
            assert_eq!(
                windows_fingerprint(&classifier, label, cwd),
                plain,
                "{label} in {cwd}"
            );
        }
    }

    #[test]
    fn test_fingerprint_keeps_arguments_and_directories_apart() {
        let classifier = classifier();
        let base = windows_fingerprint(&classifier, "cargo build", r"E:\app");
        assert_ne!(
            windows_fingerprint(&classifier, "cargo build --release", r"E:\app"),
            base
        );
        assert_ne!(
            windows_fingerprint(&classifier, "cargo build", r"E:\other"),
            base
        );
        assert_eq!(windows_fingerprint(&classifier, "  ", ""), "|");
    }

    #[test]
    fn test_fingerprint_of_a_light_label_uses_the_whole_label() {
        let classifier = classifier();
        assert_eq!(
            windows_fingerprint(&classifier, "Render   scene 4", r"C:\"),
            r"render scene 4|c:\"
        );
        assert_eq!(windows_fingerprint(&classifier, "x", "/c"), r"x|c:\");
    }

    fn windows_fingerprint(classifier: &Classifier, label: &str, cwd: &str) -> String {
        fingerprint_in(PathStyle::Windows, classifier, label, cwd)
    }

    #[test]
    fn test_fingerprint_uses_the_path_style_of_this_os() {
        let classifier = classifier();
        assert_eq!(
            fingerprint(&classifier, "cargo build", "/Work/App"),
            fingerprint_in(PATH_STYLE, &classifier, "cargo build", "/Work/App")
        );
    }

    #[test]
    fn test_normalize_cwd_per_path_style_table() {
        let cases = [
            (PathStyle::Windows, r"E:\Work\App\", r"e:\work\app"),
            (PathStyle::Windows, "/e/Work/App", r"e:\work\app"),
            (PathStyle::Windows, "C:/", r"c:\"),
            (PathStyle::Macos, "/Users/Mehran/App/", "/users/mehran/app"),
            (PathStyle::Macos, " /Users/Mehran/App ", "/users/mehran/app"),
            (PathStyle::Macos, "//", "/"),
            (PathStyle::Linux, "/home/Mehran/App/", "/home/Mehran/App"),
            (PathStyle::Linux, "/home/mehran/app", "/home/mehran/app"),
            (PathStyle::Linux, r"/srv/a\b", r"/srv/a\b"),
            (PathStyle::Linux, "/", "/"),
            (PathStyle::Linux, "", ""),
        ];
        for (style, cwd, expected) in cases {
            assert_eq!(normalize_cwd(style, cwd), expected, "{style:?} {cwd:?}");
        }
    }

    #[test]
    fn test_fingerprint_on_linux_keeps_directories_that_differ_in_case_apart() {
        let classifier = classifier();
        let linux = |cwd| fingerprint_in(PathStyle::Linux, &classifier, "Cargo build", cwd);
        assert_eq!(linux("/home/me/App/"), "cargo build|/home/me/App");
        assert_ne!(linux("/home/me/App"), linux("/home/me/app"));
        let macos = |cwd| fingerprint_in(PathStyle::Macos, &classifier, "cargo build", cwd);
        assert_eq!(macos("/Users/me/App"), macos("/users/me/app/"));
    }

    #[test]
    fn test_learned_takes_p90_of_average_cores_peak_ram_and_run_time() {
        // Ten runs: 1..=10 average cores, 1000..=10000 MB peak, 10..=100 s.
        let runs: Vec<JobRun> = (1..=10)
            .map(|step| run(step as f32, step * 1_000, step as i64 * 10))
            .collect();
        let learned = learned(&runs).unwrap();
        assert_eq!(learned.runs, 10);
        assert_eq!(learned.profile.cores, 9.0);
        assert_eq!(learned.profile.ram_mb, 9_000);
        assert_eq!(learned.profile.est_secs, 90);
        assert_eq!(learned.profile.gpu_pct, 0.0);
    }

    #[test]
    fn test_learned_from_one_run_is_that_run() {
        let learned = learned(&[run(5.5, 2_500, 42)]).unwrap();
        assert_eq!(learned.runs, 1);
        assert_eq!(learned.profile.cores, 5.5);
        assert_eq!(learned.profile.ram_mb, 2_500);
        assert_eq!(learned.profile.est_secs, 42);
    }

    /// A run like lease 35: a short burst at `busy_cores` inside a lease that averaged
    /// `avg_cores`.
    fn burst_run(avg_cores: f32, busy_cores: f32) -> JobRun {
        JobRun {
            busy: Some(BusyUsage {
                cores: busy_cores,
                gpu_pct: 0.0,
            }),
            ..run(avg_cores, 900, 20)
        }
    }

    #[test]
    fn test_learned_uses_busy_load_and_falls_back_to_the_average_for_older_runs() {
        let learned = learned(&[burst_run(0.6, 1.7)]).unwrap();
        assert_eq!(learned.profile.cores, 1.7);

        let mut gpu = burst_run(0.6, 1.7);
        gpu.avg.gpu_pct = 20.0;
        gpu.busy = Some(BusyUsage {
            cores: 1.7,
            gpu_pct: 75.0,
        });
        assert_eq!(super::learned(&[gpu]).unwrap().profile.gpu_pct, 75.0);

        // Older runs without busy load count with their average: one new run in ten stays
        // under the p90, two lift it.
        let mut mixed = vec![burst_run(0.6, 5.0)];
        mixed.extend((0..9).map(|_| run(0.6, 900, 20)));
        assert_eq!(super::learned(&mixed).unwrap().profile.cores, 0.6);
        mixed[1] = burst_run(0.6, 5.0);
        assert_eq!(super::learned(&mixed).unwrap().profile.cores, 5.0);
    }

    #[test]
    fn test_estimate_from_short_bursts_in_long_leases_reflects_the_burst() {
        // Lease 35's shape: 0.6 cores over the lease, 1.7 over the build. The averages alone
        // stay below a 1-core declaration; the busy load raises it.
        let small = JobProfile {
            cores: 1.0,
            ..declared()
        };
        let runs = [burst_run(0.6, 1.7), burst_run(0.6, 1.6)];
        let estimate = estimate(&small, learned(&runs).as_ref(), &roomy(), None);
        assert_eq!(estimate.profile.cores, 1.7);
        assert_eq!(
            estimate.reasons,
            ["cores 1.7 from p90 of 2 past run(s) (declared 1.0)"]
        );

        // A wide build held the same way: 3.1 cores over the lease, 12.4 while it compiled.
        let wide = learned(&[burst_run(3.1, 12.4)]).unwrap();
        let estimate = super::estimate(&declared(), Some(&wide), &roomy(), None);
        assert_eq!(estimate.profile.cores, 12.4);
    }

    #[test]
    fn test_learned_skips_overlapped_runs_and_keeps_the_newest_twenty() {
        let mut runs = vec![JobRun {
            overlapped: true,
            ..run(15.0, 30_000, 900)
        }];
        runs.extend((0..ESTIMATE_RUNS).map(|_| run(2.0, 1_000, 10)));
        runs.push(run(15.0, 30_000, 900));
        let learned = learned(&runs).unwrap();
        assert_eq!(learned.runs, ESTIMATE_RUNS);
        assert_eq!(learned.profile.cores, 2.0);

        let only_overlapped = [JobRun {
            overlapped: true,
            ..run(1.0, 1, 1)
        }];
        assert_eq!(super::learned(&only_overlapped), None);
        assert_eq!(super::learned(&[]), None);
    }

    #[test]
    fn test_learned_skips_killed_runs() {
        let killed = JobRun {
            end_cause: Some(EndCause::Killed),
            ..run(1.0, 500, 5)
        };
        let released = JobRun {
            end_cause: Some(EndCause::Released),
            ..run(6.0, 2_000, 60)
        };
        let learned = learned(&[killed.clone(), released]).unwrap();
        assert_eq!(learned.runs, 1);
        assert_eq!(learned.profile.est_secs, 60);
        assert_eq!(super::learned(&[killed]), None);
    }

    #[test]
    fn test_summarize_groups_by_fingerprint_in_order_of_the_last_run() {
        let other = |overlapped| JobRun {
            fingerprint: r"pnpm test|e:\web".into(),
            command: "pnpm test".into(),
            overlapped,
            ..run(3.0, 900, 30)
        };
        let runs = [
            other(true),
            run(6.0, 2_000, 50),
            run(4.0, 1_000, 40),
            other(true),
        ];
        let summaries = summarize(&runs);
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].command, "pnpm test");
        assert_eq!((summaries[0].runs, summaries[0].estimate_runs), (2, 0));
        assert_eq!(summaries[0].learned, None);
        assert_eq!((summaries[1].runs, summaries[1].estimate_runs), (2, 2));
        assert_eq!(summaries[1].learned.map(|profile| profile.cores), Some(6.0));
        assert!(summarize(&[]).is_empty());
    }

    #[test]
    fn test_estimate_without_history_is_the_declared_profile() {
        let estimate = estimate(&declared(), None, &roomy(), None);
        assert_eq!(estimate.profile, declared());
        assert!(estimate.reasons.is_empty());
    }

    #[test]
    fn test_estimate_raises_values_history_exceeds_and_names_the_source() {
        let learned = learned(&[run(11.2, 6_000, 95), run(10.0, 5_000, 80)]).unwrap();
        let estimate = estimate(&declared(), Some(&learned), &roomy(), None);
        assert_eq!(estimate.profile.cores, 11.2);
        assert_eq!(estimate.profile.ram_mb, 6_000);
        // Faster runs never lower the declared run time.
        assert_eq!(estimate.profile.est_secs, 300);
        assert_eq!(
            estimate.reasons,
            [
                "cores 11.2 from p90 of 2 past run(s) (declared 8.0)",
                "RAM 6000 MB from p90 of 2 past run(s) (declared 4096 MB)",
            ]
        );
    }

    fn lowering() -> Lowering {
        Lowering {
            min_runs: 3,
            margin_pct: 10.0,
            min_cores: None,
        }
    }

    fn lowered(runs: &[JobRun], lowering: Lowering) -> Estimate {
        estimate(
            &declared(),
            learned(runs).as_ref(),
            &roomy(),
            Some(&lowering),
        )
    }

    #[test]
    fn test_estimate_lowers_after_enough_runs_to_p90_plus_the_margin() {
        let runs = [
            run(2.0, 1_800, 30),
            run(3.0, 2_000, 40),
            run(2.5, 1_500, 20),
        ];
        let estimate = lowered(&runs, lowering());
        assert!((estimate.profile.cores - 3.3).abs() < 1e-4);
        assert_eq!(estimate.profile.ram_mb, 2_200);
        // Faster runs never lower the declared run time.
        assert_eq!(estimate.profile.est_secs, 300);
        assert_eq!(
            estimate.reasons,
            [
                "cores 3.3 from p90 of 3 past run(s) plus 10% (declared 8.0)",
                "RAM 2200 MB from the highest peak of 3 past run(s) plus 10% (declared 4096 MB)",
            ]
        );

        let wide = lowered(
            &runs,
            Lowering {
                margin_pct: 50.0,
                ..lowering()
            },
        );
        assert!((wide.profile.cores - 4.5).abs() < 1e-4);
        assert_eq!(wide.profile.ram_mb, 3_000);
    }

    #[test]
    fn test_estimate_waits_for_the_minimum_number_of_runs() {
        let runs = [run(2.0, 1_000, 30), run(3.0, 2_000, 40)];
        let estimate = lowered(&runs, lowering());
        assert_eq!(estimate.profile, declared());
        assert_eq!(
            estimate.reasons,
            ["2 past run(s) stayed within the declared profile; lowering waits for 3 runs"]
        );

        let estimate = lowered(
            &runs,
            Lowering {
                min_runs: 2,
                ..lowering()
            },
        );
        assert!((estimate.profile.cores - 3.3).abs() < 1e-4);
    }

    #[test]
    fn test_estimate_without_lowering_never_lowers_the_declared_profile() {
        let runs = [run(1.0, 500, 5), run(1.0, 500, 5), run(1.0, 500, 5)];
        let estimate = estimate(&declared(), learned(&runs).as_ref(), &roomy(), None);
        assert_eq!(estimate.profile, declared());
        assert_eq!(
            estimate.reasons,
            ["3 past run(s) stayed within the declared profile"]
        );
    }

    #[test]
    fn test_estimate_lowering_stops_at_the_floor() {
        let runs = [run(2.0, 100, 5), run(2.0, 120, 5), run(0.05, 90, 5)];
        let estimate = lowered(&runs, lowering());
        assert_eq!(estimate.profile.ram_mb, FLOOR_RAM_MB);
        assert_eq!(
            estimate.reasons[1],
            "RAM 256 MB, the floor (the highest peak of 3 past run(s): 120 MB, declared 4096 MB)"
        );

        let idle = [run(0.05, 1_000, 5), run(0.05, 1_000, 5), run(0.0, 900, 5)];
        let estimate = lowered(&idle, lowering());
        assert_eq!(estimate.profile.cores, FLOOR_CORES);
        assert_eq!(
            estimate.reasons[0],
            "cores 0.25, the floor (p90 of 3 past run(s): 0.05, declared 8.0)"
        );
    }

    #[test]
    fn test_estimate_lowering_never_makes_a_request_low_impact() {
        // Like `pnpm build` whose work runs outside its session: it measures next to nothing.
        let runs = [run(0.01, 100, 60), run(0.01, 120, 60), run(0.0, 90, 60)];
        let estimate = lowered(&runs, lowering());
        assert_eq!(estimate.profile.cores, FLOOR_CORES);
        assert_eq!(estimate.profile.ram_mb, LOW_IMPACT_MAX_RAM_MB + 1);
        assert!(!estimate.profile.is_low_impact());
        assert_eq!(
            estimate.reasons[1],
            "RAM 257 MB, the least that keeps it out of low impact \
             (the highest peak of 3 past run(s): 120 MB, declared 4096 MB)"
        );

        // A request that is low impact as declared stays so.
        let small = JobProfile {
            cores: 0.25,
            ram_mb: 256,
            ..declared()
        };
        let estimate =
            super::estimate(&small, learned(&runs).as_ref(), &roomy(), Some(&lowering()));
        assert_eq!(estimate.profile, small);
    }

    #[test]
    fn test_estimate_lowers_ram_to_the_highest_peak_not_the_p90() {
        // Nine incremental builds and one clean build that peaked three times as high.
        let mut runs: Vec<JobRun> = (0..9).map(|_| run(2.0, 1_000, 10)).collect();
        runs.push(run(2.0, 3_000, 60));
        let learned = learned(&runs).unwrap();
        assert_eq!(learned.profile.ram_mb, 1_000);
        assert_eq!(learned.max.ram_mb, 3_000);
        assert_eq!(lowered(&runs, lowering()).profile.ram_mb, 3_300);
    }

    #[test]
    fn test_estimate_named_min_cores_bounds_lowering() {
        let runs = vec![run(0.5, 1_000, 10); 3];
        let named = |min_cores| {
            lowered(
                &runs,
                Lowering {
                    min_cores: Some(min_cores),
                    ..lowering()
                },
            )
        };
        let estimate = named(2.0);
        assert_eq!(estimate.profile.cores, 2.0);
        assert_eq!(
            estimate.reasons[0],
            "cores 2.0, the minimum the request named (p90 of 3 past run(s): 0.5, declared 8.0)"
        );
        // A minimum the lowered value already clears changes nothing.
        assert!((named(0.5).profile.cores - 0.6).abs() < 1e-4);
        // A minimum under the floor leaves the floor in place.
        let idle = lowered(
            &vec![run(0.05, 1_000, 10); 3],
            Lowering {
                min_cores: Some(0.1),
                ..lowering()
            },
        );
        assert_eq!(idle.profile.cores, FLOOR_CORES);
    }

    #[test]
    fn test_estimate_lowers_gpu_and_vram_only_when_a_run_measured_them() {
        let gpu_declared = JobProfile {
            gpu_pct: 60.0,
            vram_mb: 4_000,
            ..declared()
        };
        let unmeasured = vec![run(2.0, 1_000, 10); 3];
        let estimate = estimate(
            &gpu_declared,
            learned(&unmeasured).as_ref(),
            &roomy(),
            Some(&lowering()),
        );
        assert_eq!(
            (estimate.profile.gpu_pct, estimate.profile.vram_mb),
            (60.0, 4_000)
        );

        let measured: Vec<JobRun> = (0..3)
            .map(|_| {
                let mut gpu_run = run(2.0, 1_000, 10);
                gpu_run.avg.gpu_pct = 20.0;
                gpu_run.peak.vram_mb = 1_000;
                gpu_run
            })
            .collect();
        let estimate = super::estimate(
            &gpu_declared,
            learned(&measured).as_ref(),
            &roomy(),
            Some(&lowering()),
        );
        assert_eq!(estimate.profile.gpu_pct, 22.0);
        assert_eq!(estimate.profile.vram_mb, 1_100);
        let gpu_reason = "GPU 22% from p90 of 3 past run(s) plus 10% (declared 60%)";
        assert!(estimate.reasons.iter().any(|reason| reason == gpu_reason));
    }

    #[test]
    fn test_estimate_raises_and_lowers_in_one_request() {
        let runs = [
            run(11.2, 1_000, 95),
            run(10.0, 1_000, 80),
            run(10.5, 1_000, 90),
        ];
        let estimate = lowered(&runs, lowering());
        assert_eq!(estimate.profile.cores, 11.2);
        assert_eq!(estimate.profile.ram_mb, 1_100);
        assert_eq!(
            estimate.reasons,
            [
                "cores 11.2 from p90 of 3 past run(s) (declared 8.0)",
                "RAM 1100 MB from the highest peak of 3 past run(s) plus 10% (declared 4096 MB)",
            ]
        );
    }

    #[test]
    fn test_lowering_for_request_is_off_for_benchmarks_and_when_configured_off() {
        let config = EstimatesConfig::default();
        assert_eq!(
            Lowering::for_request(&config, LeaseKind::Normal, Some(2.0)),
            Some(Lowering {
                min_cores: Some(2.0),
                ..lowering()
            })
        );
        assert_eq!(
            Lowering::for_request(&config, LeaseKind::BenchmarkExclusive, None),
            None
        );
        let off = EstimatesConfig {
            lower: false,
            ..config
        };
        assert_eq!(Lowering::for_request(&off, LeaseKind::Normal, None), None);
    }

    #[test]
    fn test_estimate_caps_at_what_the_machine_can_free() {
        let learned = learned(&[run(15.3, 2_000, 60)]).unwrap();
        let estimate = estimate(&declared(), Some(&learned), &roomy(), None);
        assert!((estimate.profile.cores - 13.6).abs() < 1e-4);
        assert_eq!(
            estimate.reasons,
            [
                "cores 13.6, the most the machine can free (p90 of 1 past run(s): 15.3, declared 8.0)"
            ]
        );

        let cramped = Room {
            cores: 6.0,
            ..roomy()
        };
        let estimate = super::estimate(&declared(), Some(&learned), &cramped, None);
        assert_eq!(estimate.profile.cores, 8.0);
    }

    #[test]
    fn test_estimate_gpu_without_an_adapter_is_not_capped() {
        let mut gpu_run = run(1.0, 1_000, 10);
        gpu_run.avg.gpu_pct = 70.0;
        gpu_run.peak.vram_mb = 3_000;
        let learned = learned(&[gpu_run]).unwrap();
        let no_gpu = Room {
            gpu_pct: None,
            vram_mb: None,
            ..roomy()
        };
        let estimate = estimate(&declared(), Some(&learned), &no_gpu, None);
        assert_eq!(estimate.profile.gpu_pct, 70.0);
        assert_eq!(estimate.profile.vram_mb, 3_000);
    }
}
