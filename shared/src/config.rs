use std::{collections::BTreeMap, fmt, fs, io, path::Path};

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{
    Error, Result,
    types::{JobClass, JobProfile, RangeIssue},
};

/// Daemon configuration, read from `config.toml`.
///
/// Every value has a default, so a user file only lists what it changes. Tables merge field
/// by field over the defaults; arrays such as `classifier.rules` replace the default list as
/// a whole. Unknown keys are rejected so that typos surface instead of being ignored.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// HTTP listener settings.
    pub daemon: DaemonConfig,
    /// How much of the machine the admission engine keeps free.
    pub admission: AdmissionConfig,
    /// How past runs change a request's estimate.
    pub estimates: EstimatesConfig,
    /// When the machine counts as quiet enough for a benchmark.
    pub benchmark: BenchmarkConfig,
    /// Lease lifetime settings.
    pub leases: LeaseConfig,
    /// How long ended leases and job history are kept.
    pub retention: RetentionConfig,
    /// PreToolUse hook behavior.
    pub hook: HookConfig,
    /// The profile and minimum cores behind each [`JobClass`].
    pub classes: ClassProfiles,
    /// Named profiles sized between the classes, chosen by name like a class.
    pub presets: Presets,
    /// Which shell commands count as heavy.
    pub classifier: ClassifierConfig,
    /// How leases use cgroups on Linux; ignored on other OSes.
    pub linux: LinuxConfig,
}

impl Config {
    /// Parses and validates a TOML document. Missing values take their defaults.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConfigParse`] for malformed TOML, unknown keys, or wrongly typed
    /// values, and [`Error::Config`] for values out of range.
    pub fn from_toml(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    /// Loads the file at `path`, or the defaults when the file does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConfigFile`] naming `path` when the file exists but cannot be read,
    /// or holds what [`Config::from_toml`] rejects.
    pub fn load(path: &Path) -> Result<Self> {
        let loaded = match fs::read_to_string(path) {
            Ok(text) => Self::from_toml(&text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => Err(error.into()),
        };
        loaded.map_err(|reason| Error::ConfigFile {
            path: path.to_owned(),
            reason: Box::new(reason),
        })
    }

    /// The `daemon.port` the file at `path` sets, when the file parses and the port is valid,
    /// whatever its other values are; `None` otherwise.
    pub fn port_in(path: &Path) -> Option<u16> {
        let text = fs::read_to_string(path).ok()?;
        let table: toml::Table = toml::from_str(&text).ok()?;
        let port = table.get("daemon")?.get("port")?.as_integer()?;
        u16::try_from(port).ok().filter(|port| *port != 0)
    }

    /// Checks every value against its allowed range.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] naming every invalid value, as [`Config::problems`] lists them.
    pub fn validate(&self) -> Result<()> {
        let problems = self.problems();
        if problems.is_empty() {
            return Ok(());
        }
        let text: Vec<String> = problems.iter().map(ToString::to_string).collect();
        Err(Error::Config(text.join("; ")))
    }

    /// Every value outside its allowed range, each with its key and what it must be, in file
    /// order; empty when the configuration is valid.
    pub fn problems(&self) -> Vec<FieldError> {
        let mut problems = Vec::new();
        let mut check = |valid: bool, key: &str, message: &str| {
            if !valid {
                problems.push(FieldError::new(key, message));
            }
        };
        check(self.daemon.port != 0, "daemon.port", "must not be 0");
        check(
            (0.0..100.0).contains(&self.admission.headroom_pct),
            "admission.headroom_pct",
            "must be at least 0 and below 100",
        );
        check(
            self.estimates.min_runs > 0,
            "estimates.min_runs",
            "must be positive",
        );
        check(
            is_pct(self.estimates.margin_pct),
            "estimates.margin_pct",
            "must be 0 to 100",
        );
        let benchmark = &self.benchmark;
        check(
            is_pct(benchmark.quiet_cpu_pct),
            "benchmark.quiet_cpu_pct",
            "must be 0 to 100",
        );
        check(
            is_pct(benchmark.quiet_gpu_pct),
            "benchmark.quiet_gpu_pct",
            "must be 0 to 100",
        );
        check(
            benchmark.quiet_secs > 0,
            "benchmark.quiet_secs",
            "must be positive",
        );
        check(
            benchmark.max_quiet_wait_secs >= benchmark.quiet_secs,
            "benchmark.max_quiet_wait_secs",
            "must be at least quiet_secs",
        );
        check(
            self.leases.heartbeat_ttl_secs >= MIN_HEARTBEAT_TTL_SECS,
            "leases.heartbeat_ttl_secs",
            "must be at least 10",
        );
        let retention = &self.retention;
        check(
            retention.lease_days > 0,
            "retention.lease_days",
            "must be positive",
        );
        check(
            retention.history_days > 0,
            "retention.history_days",
            "must be positive",
        );
        check(
            retention.keep_runs_per_job > 0,
            "retention.keep_runs_per_job",
            "must be positive",
        );
        check(
            retention.max_job_runs >= retention.keep_runs_per_job,
            "retention.max_job_runs",
            "must be at least keep_runs_per_job",
        );
        for class in JobClass::ALL {
            let row = format!("classes.{}", class.as_str());
            for issue in self.classes.get(class).range_issues() {
                check(false, &format!("{row}.{}", issue.field), issue.allowed);
            }
        }
        for (name, sizing) in &self.presets.0 {
            let row = format!("presets.{name}");
            let well_formed = !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
            check(
                well_formed,
                &row,
                "is not a valid name: use only a-z, 0-9 and _",
            );
            check(
                JobClass::ALL.iter().all(|class| class.as_str() != name),
                &row,
                "is the name of a class",
            );
            for issue in sizing.range_issues() {
                check(false, &format!("{row}.{}", issue.field), issue.allowed);
            }
        }
        for (index, rule) in self.classifier.rules.iter().enumerate() {
            let rule_key = format!("classifier.rules[{index}]");
            check(
                !rule.program.trim().is_empty(),
                &format!("{rule_key}.program"),
                "must not be blank",
            );
            check(
                rule.args.iter().all(|arg| !arg.trim().is_empty()),
                &format!("{rule_key}.args"),
                "must not contain blank entries",
            );
        }
        check(
            self.classifier
                .light_flags
                .iter()
                .all(|flag| !flag.trim().is_empty()),
            "classifier.light_flags",
            "must not contain blank entries",
        );
        problems
    }
}

/// A value of the configuration outside what it allows: its key and what it must be.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    /// The value's dotted key, such as `admission.headroom_pct`, `presets.big.min_cores` or
    /// `classifier.rules[2].program`. A preset's name is reported as `presets.<name>`.
    pub key: String,
    /// What the value must be, such as `must be at least 0 and below 100`.
    pub message: String,
}

impl FieldError {
    /// The error for `key`, saying what it must be.
    pub fn new(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.key, self.message)
    }
}

fn is_pct(value: f32) -> bool {
    (0.0..=100.0).contains(&value)
}

/// HTTP listener settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DaemonConfig {
    /// Port on `127.0.0.1`.
    pub port: u16,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self { port: 47_823 }
    }
}

/// How much of the machine the admission engine keeps free.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AdmissionConfig {
    /// Share of each resource (cores, RAM, GPU, VRAM) never handed out to leases, so the
    /// desktop and unleased work stay responsive.
    pub headroom_pct: f32,
}

impl Default for AdmissionConfig {
    fn default() -> Self {
        Self { headroom_pct: 15.0 }
    }
}

/// How past runs change a request's estimate. Raising to what similar runs used always
/// happens; lowering is what these keys control.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EstimatesConfig {
    /// Whether a request may reserve less than it declares once its runs used less.
    pub lower: bool,
    /// Clean runs needed before a value lowers.
    pub min_runs: u32,
    /// Added on top of what the runs used, so a run slightly heavier than the last ones
    /// still fits its reservation.
    pub margin_pct: f32,
}

impl Default for EstimatesConfig {
    fn default() -> Self {
        Self {
            lower: true,
            min_runs: 3,
            margin_pct: 10.0,
        }
    }
}

/// When the machine counts as quiet enough for a benchmark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BenchmarkConfig {
    /// Total CPU load at or below which the machine is quiet.
    pub quiet_cpu_pct: f32,
    /// GPU engine load at or below which the machine is quiet.
    pub quiet_gpu_pct: f32,
    /// The window the loads are averaged over once no other lease is active. The machine is
    /// quiet when both averages are at or below their limits.
    pub quiet_secs: u64,
    /// How long a drained machine may stay above the limits before a waiting benchmark is
    /// granted anyway, marked as not quiet. At least `quiet_secs`.
    pub max_quiet_wait_secs: u64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            quiet_cpu_pct: 10.0,
            quiet_gpu_pct: 10.0,
            quiet_secs: 10,
            max_quiet_wait_secs: 60,
        }
    }
}

/// The shortest lease timeout. allot-mcp sends a heartbeat every third of it, so a shorter one
/// leaves too little room for a heartbeat that arrives late on a busy machine.
pub const MIN_HEARTBEAT_TTL_SECS: u64 = 10;

/// Lease lifetime settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LeaseConfig {
    /// A lease without a heartbeat for this long expires; at least
    /// [`MIN_HEARTBEAT_TTL_SECS`].
    pub heartbeat_ttl_secs: u64,
    /// An active lease ends once no command runs under it and its holder's session has been
    /// idle for this long, for holders that forget `release_lease`; 0 turns this off.
    pub idle_release_secs: u64,
}

impl Default for LeaseConfig {
    fn default() -> Self {
        Self {
            heartbeat_ttl_secs: 60,
            idle_release_secs: 180,
        }
    }
}

/// How long ended leases and job history are kept. The daemon prunes once an hour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RetentionConfig {
    /// Released and expired leases, with their audit trail, are deleted this many days after
    /// they ended.
    pub lease_days: u64,
    /// Job runs are deleted this many days after they ended, except the newest
    /// `keep_runs_per_job` of each job.
    pub history_days: u64,
    /// Runs of each job kept regardless of age, so estimates survive long breaks. The
    /// estimator reads the newest 20.
    pub keep_runs_per_job: u64,
    /// Hard cap on stored job runs; the oldest beyond it are deleted whatever their job.
    pub max_job_runs: u64,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            lease_days: 30,
            history_days: 90,
            keep_runs_per_job: 20,
            max_job_runs: 5_000,
        }
    }
}

/// PreToolUse hook behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HookConfig {
    /// Allow heavy commands when the daemon is unreachable. Off means every heavy command is
    /// denied until the daemon is back.
    pub fail_open: bool,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self { fail_open: true }
    }
}

/// How leases use cgroups on Linux; ignored on other OSes.
///
/// Each active lease runs in a systemd scope of its own, which gives exact usage and a kill
/// that ends every process at once. Caps on what a lease may use are opt-in, because a job
/// that outgrows its declared profile is slowed down by them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LinuxConfig {
    /// Run each lease in its own cgroup scope when the systemd user manager allows it. Off
    /// follows the process tree instead, as without systemd.
    pub scopes: bool,
    /// Cap a lease's CPU time at its reserved cores.
    pub enforce_cpu: bool,
    /// Throttle a lease above its reserved RAM (`MemoryHigh`); it is never killed for memory.
    pub enforce_memory: bool,
}

impl Default for LinuxConfig {
    fn default() -> Self {
        Self {
            scopes: true,
            enforce_cpu: false,
            enforce_memory: false,
        }
    }
}

/// A class or preset: the profile a lease of it asks for, and the fewest cores it may start
/// with. The minimum is a key of the same table, such as `min_cores` in `[classes.cpu_heavy]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Sizing {
    /// What a lease of it asks for.
    #[serde(flatten)]
    pub profile: JobProfile,
    /// The fewest cores a lease of it may start with; `None` takes
    /// [`JobProfile::default_min_cores`], so the default follows a changed `cores`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_cores: Option<f32>,
}

impl Sizing {
    /// `profile` with the default minimum.
    pub const fn of(profile: JobProfile) -> Self {
        Self {
            profile,
            min_cores: None,
        }
    }

    /// The fewest cores a lease of it may start with: the configured minimum or the default.
    pub fn minimum(&self) -> f32 {
        self.min_cores
            .unwrap_or_else(|| self.profile.default_min_cores())
    }

    /// Every value of the profile or its minimum out of range; empty when all are in range.
    pub fn range_issues(&self) -> Vec<RangeIssue> {
        let mut issues = self.profile.range_issues();
        issues.extend(
            self.min_cores
                .and_then(|min_cores| self.profile.min_cores_reason(min_cores)),
        );
        issues
    }
}

/// The profile and minimum cores behind each [`JobClass`].
///
/// A partial table such as `[classes.gpu_heavy]` with only `vram_mb` keeps that class's own
/// defaults for the other fields, not zeros.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassProfiles {
    /// Sizing of [`JobClass::Light`].
    pub light: Sizing,
    /// Sizing of [`JobClass::CpuHeavy`].
    pub cpu_heavy: Sizing,
    /// Sizing of [`JobClass::GpuHeavy`].
    pub gpu_heavy: Sizing,
    /// Sizing of [`JobClass::Mixed`].
    pub mixed: Sizing,
}

impl ClassProfiles {
    /// Returns the sizing configured for `class`.
    pub fn get(&self, class: JobClass) -> &Sizing {
        match class {
            JobClass::Light => &self.light,
            JobClass::CpuHeavy => &self.cpu_heavy,
            JobClass::GpuHeavy => &self.gpu_heavy,
            JobClass::Mixed => &self.mixed,
        }
    }
}

impl Default for ClassProfiles {
    fn default() -> Self {
        Self {
            light: Sizing::of(JobProfile {
                cores: 1.0,
                ram_mb: 512,
                gpu_pct: 0.0,
                vram_mb: 0,
                est_secs: 30,
            }),
            cpu_heavy: Sizing::of(JobProfile {
                cores: 8.0,
                ram_mb: 4096,
                gpu_pct: 0.0,
                vram_mb: 0,
                est_secs: 300,
            }),
            gpu_heavy: Sizing::of(JobProfile {
                cores: 2.0,
                ram_mb: 4096,
                gpu_pct: 80.0,
                vram_mb: 4096,
                est_secs: 600,
            }),
            mixed: Sizing::of(JobProfile {
                cores: 6.0,
                ram_mb: 8192,
                gpu_pct: 50.0,
                vram_mb: 2048,
                est_secs: 600,
            }),
        }
    }
}

impl<'de> Deserialize<'de> for ClassProfiles {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let overrides = ClassOverrides::deserialize(deserializer)?;
        let defaults = Self::default();
        Ok(Self {
            light: overrides.light.apply(defaults.light),
            cpu_heavy: overrides.cpu_heavy.apply(defaults.cpu_heavy),
            gpu_heavy: overrides.gpu_heavy.apply(defaults.gpu_heavy),
            mixed: overrides.mixed.apply(defaults.mixed),
        })
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ClassOverrides {
    light: ProfileOverride,
    cpu_heavy: ProfileOverride,
    gpu_heavy: ProfileOverride,
    mixed: ProfileOverride,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileOverride {
    cores: Option<f32>,
    ram_mb: Option<u64>,
    gpu_pct: Option<f32>,
    vram_mb: Option<u64>,
    est_secs: Option<u64>,
    min_cores: Option<f32>,
}

impl ProfileOverride {
    fn apply(self, base: Sizing) -> Sizing {
        let profile = base.profile;
        Sizing {
            profile: JobProfile {
                cores: self.cores.unwrap_or(profile.cores),
                ram_mb: self.ram_mb.unwrap_or(profile.ram_mb),
                gpu_pct: self.gpu_pct.unwrap_or(profile.gpu_pct),
                vram_mb: self.vram_mb.unwrap_or(profile.vram_mb),
                est_secs: self.est_secs.unwrap_or(profile.est_secs),
            },
            min_cores: self.min_cores.or(base.min_cores),
        }
    }
}

/// Named profiles between the classes, by name, each with its minimum cores.
///
/// A `[presets.<name>]` table changes that preset field by field, like a class, or adds a new
/// one, which needs at least `cores` and `ram_mb` (GPU and VRAM default to 0, the run time
/// to 300 s). The default presets stay unless overridden.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Presets(pub BTreeMap<String, Sizing>);

/// Run time of a new preset that gives none.
pub(crate) const PRESET_EST_SECS: u64 = 300;

impl Presets {
    /// The sizing of preset `name`.
    pub fn get(&self, name: &str) -> Option<&Sizing> {
        self.0.get(name)
    }

    /// The preset names, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

impl Default for Presets {
    fn default() -> Self {
        let preset = |cores, ram_mb, gpu_pct, vram_mb, est_secs| {
            Sizing::of(JobProfile {
                cores,
                ram_mb,
                gpu_pct,
                vram_mb,
                est_secs,
            })
        };
        Self(BTreeMap::from([
            ("cpu_small".to_owned(), preset(2.0, 1024, 0.0, 0, 60)),
            ("cpu_medium".to_owned(), preset(4.0, 2048, 0.0, 0, 180)),
            ("gpu_small".to_owned(), preset(1.0, 2048, 30.0, 2048, 300)),
            ("quick".to_owned(), preset(4.0, 2048, 0.0, 0, 10)),
            ("low_impact".to_owned(), preset(0.25, 256, 0.0, 0, 60)),
        ]))
    }
}

impl<'de> Deserialize<'de> for Presets {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let overrides = BTreeMap::<String, ProfileOverride>::deserialize(deserializer)?;
        let mut presets = Self::default();
        for (name, value) in overrides {
            let profile = match presets.0.get(&name) {
                Some(&base) => value.apply(base),
                None => {
                    let (Some(cores), Some(ram_mb)) = (value.cores, value.ram_mb) else {
                        return Err(de::Error::custom(format!(
                            "new preset `{name}` needs both cores and ram_mb"
                        )));
                    };
                    value.apply(Sizing::of(JobProfile {
                        cores,
                        ram_mb,
                        gpu_pct: 0.0,
                        vram_mb: 0,
                        est_secs: PRESET_EST_SECS,
                    }))
                }
            };
            presets.0.insert(name, profile);
        }
        Ok(presets)
    }
}

/// Which shell commands count as heavy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClassifierConfig {
    /// Rules checked against every simple command in a command line, in order.
    pub rules: Vec<HeavyRule>,
    /// A simple command containing any of these arguments only prints and exits, so it is
    /// light even when a rule matches it, for example `cargo build --help`.
    pub light_flags: Vec<String>,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            rules: vec![
                HeavyRule::new(
                    "cargo",
                    &["build", "test", "run", "bench"],
                    JobClass::CpuHeavy,
                ),
                HeavyRule::new("pnpm", &["build*", "test*"], JobClass::CpuHeavy),
                HeavyRule::new("msbuild", &[], JobClass::CpuHeavy),
                HeavyRule::new("cmake", &["--build"], JobClass::CpuHeavy),
                HeavyRule::new("python*", &["train*.py"], JobClass::GpuHeavy),
                HeavyRule::new("py", &["train*.py"], JobClass::GpuHeavy),
            ],
            light_flags: ["--help", "-h", "--version", "-V", "/?"]
                .map(String::from)
                .to_vec(),
        }
    }
}

/// Marks a simple command as heavy when its program and arguments match.
///
/// Matching ignores case, and `*` matches any run of characters.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeavyRule {
    /// Program name without directory or extension, for example `cargo` or `python*`.
    pub program: String,
    /// Any one of these makes the command heavy; empty matches any arguments. A pattern that
    /// starts with `-` matches that flag anywhere in the command; any other pattern matches
    /// the first positional argument (the first one not starting with `-` or `+`).
    #[serde(default)]
    pub args: Vec<String>,
    /// Class the matched command needs a lease for.
    pub class: JobClass,
}

impl HeavyRule {
    fn new(program: &str, args: &[&str], class: JobClass) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|&arg| arg.into()).collect(),
            class,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;
    use crate::mutate::mutations;

    fn config_error(text: &str) -> String {
        match Config::from_toml(text) {
            Err(Error::Config(message)) => message,
            other => panic!("expected Error::Config, got {other:?}"),
        }
    }

    #[test]
    fn test_config_empty_document_yields_defaults() {
        assert_eq!(Config::from_toml("").unwrap(), Config::default());
    }

    #[test]
    fn test_config_default_passes_validation() {
        Config::default().validate().unwrap();
    }

    #[test]
    fn test_config_default_toml_round_trip_preserves_every_value() {
        let text = toml::to_string(&Config::default()).unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), Config::default());
    }

    #[test]
    fn test_config_presets_merge_over_defaults_and_add_new_ones() {
        let config = Config::from_toml(
            "[presets.cpu_small]\ncores = 3.0\n\n[presets.render_preview]\ncores = 2.0\n\
             ram_mb = 3000\ngpu_pct = 40.0\n",
        )
        .unwrap();
        let defaults = Presets::default();
        let small = config.presets.get("cpu_small").unwrap();
        assert_eq!(small.profile.cores, 3.0);
        assert_eq!(
            small.profile.est_secs,
            defaults.get("cpu_small").unwrap().profile.est_secs
        );
        let preview = config.presets.get("render_preview").unwrap().profile;
        assert_eq!((preview.gpu_pct, preview.vram_mb), (40.0, 0));
        assert_eq!(preview.est_secs, PRESET_EST_SECS);
        assert_eq!(config.presets.get("quick"), defaults.get("quick"));
    }

    #[test]
    fn test_config_presets_reject_bad_names_and_incomplete_new_ones() {
        assert!(config_error("[presets.cpu_heavy]\ncores = 1.0\nram_mb = 1\n").contains("class"));
        assert!(config_error("[presets.Big]\ncores = 1.0\nram_mb = 1\n").contains("a-z"));
        assert!(config_error("[presets.quick]\ngpu_pct = 140.0\n").contains("presets.quick"));
        let incomplete = Config::from_toml("[presets.huge]\ncores = 12.0\n").unwrap_err();
        assert!(incomplete.to_string().contains("ram_mb"), "{incomplete}");
    }

    #[test]
    fn test_default_presets_are_quick_and_low_impact_where_named() {
        let presets = Presets::default();
        let profile = |name| presets.get(name).unwrap().profile;
        assert!(profile("quick").is_quick());
        assert!(profile("low_impact").is_low_impact());
        assert!(!profile("cpu_small").is_quick());
        assert!(!profile("cpu_small").is_low_impact());
    }

    #[test]
    fn test_config_partial_table_merges_over_defaults() {
        let config = Config::from_toml(
            "[leases]\nheartbeat_ttl_secs = 90\n\n[classes.gpu_heavy]\nvram_mb = 8192\n",
        )
        .unwrap();
        let defaults = Config::default();
        assert_eq!(config.leases.heartbeat_ttl_secs, 90);
        assert_eq!(
            config.leases.idle_release_secs,
            defaults.leases.idle_release_secs
        );
        assert_eq!(config.classes.gpu_heavy.profile.vram_mb, 8192);
        assert_eq!(
            config.classes.gpu_heavy.profile.cores,
            defaults.classes.gpu_heavy.profile.cores
        );
        assert_eq!(config.classes.cpu_heavy, defaults.classes.cpu_heavy);
        assert_eq!(config.daemon, defaults.daemon);
        assert_eq!(config.classifier, defaults.classifier);
    }

    #[test]
    fn test_config_default_minimums_are_half_with_at_least_one_core() {
        let config = Config::default();
        let class = |class| config.classes.get(class).minimum();
        assert_eq!(class(JobClass::Light), 1.0);
        assert_eq!(class(JobClass::CpuHeavy), 4.0);
        assert_eq!(class(JobClass::GpuHeavy), 1.0);
        assert_eq!(class(JobClass::Mixed), 3.0);
        let preset = |name| config.presets.get(name).unwrap().minimum();
        assert_eq!(preset("cpu_small"), 1.0);
        assert_eq!(preset("cpu_medium"), 2.0);
        assert_eq!(preset("gpu_small"), 1.0);
        assert_eq!(preset("quick"), 2.0);
        assert_eq!(preset("low_impact"), 0.25);
    }

    #[test]
    fn test_config_min_cores_overrides_and_default_follows_changed_cores() {
        let config = Config::from_toml(
            "[classes.cpu_heavy]\nmin_cores = 6.0\n\n[classes.mixed]\ncores = 10.0\n\n\
             [presets.quick]\nmin_cores = 1.5\n\n[presets.wide]\ncores = 12.0\nram_mb = 1024\n",
        )
        .unwrap();
        assert_eq!(config.classes.cpu_heavy.minimum(), 6.0);
        assert_eq!(config.classes.cpu_heavy.profile.cores, 8.0);
        assert_eq!(config.classes.mixed.minimum(), 5.0);
        assert_eq!(config.presets.get("quick").unwrap().minimum(), 1.5);
        assert_eq!(config.presets.get("wide").unwrap().minimum(), 6.0);
    }

    #[test]
    fn test_config_min_cores_out_of_range_names_the_table_and_key() {
        for (table, keys) in [
            ("classes.cpu_heavy", "min_cores = 0.0"),
            ("classes.light", "min_cores = 1.5"),
            ("classes.mixed", "cores = 2.0\nmin_cores = 3.0"),
            ("presets.cpu_small", "min_cores = -1.0"),
            ("presets.cpu_small", "min_cores = nan"),
        ] {
            let message = config_error(&format!("[{table}]\n{keys}\n"));
            let named = message.contains(table) && message.contains("min_cores");
            assert!(named, "{message}");
        }
    }

    #[test]
    fn test_config_min_cores_round_trips_and_stays_out_when_unset() {
        let text = toml::to_string(&Config::default()).unwrap();
        assert!(!text.contains("min_cores"), "{text}");
        let config = Config::from_toml("[classes.gpu_heavy]\nmin_cores = 2.0\n").unwrap();
        let text = toml::to_string(&config).unwrap();
        assert_eq!(Config::from_toml(&text).unwrap(), config);
        assert_eq!(config.classes.gpu_heavy.min_cores, Some(2.0));
    }

    #[test]
    fn test_config_linux_table_defaults_to_scopes_without_caps_and_merges() {
        let defaults = LinuxConfig::default();
        assert!(defaults.scopes);
        assert!(!defaults.enforce_cpu);
        assert!(!defaults.enforce_memory);

        let config = Config::from_toml("[linux]\nenforce_memory = true\n").unwrap();
        assert_eq!(
            config.linux,
            LinuxConfig {
                enforce_memory: true,
                ..defaults
            }
        );
        let result = Config::from_toml("[linux]\nenforce_io = true\n");
        assert!(matches!(result, Err(Error::ConfigParse(_))));
    }

    #[test]
    fn test_config_estimates_default_and_override_field_by_field() {
        let defaults = EstimatesConfig::default();
        assert_eq!(
            (defaults.lower, defaults.min_runs, defaults.margin_pct),
            (true, 3, 10.0)
        );
        assert_eq!(Config::from_toml("").unwrap().estimates, defaults);
        let config = Config::from_toml("[estimates]\nmin_runs = 5\n").unwrap();
        assert_eq!(
            config.estimates,
            EstimatesConfig {
                min_runs: 5,
                ..defaults
            }
        );
        let config = Config::from_toml("[estimates]\nlower = false\nmargin_pct = 0\n").unwrap();
        assert!(!config.estimates.lower);
        assert_eq!(config.estimates.margin_pct, 0.0);
        let result = Config::from_toml("[estimates]\nlower_ram = false\n");
        assert!(matches!(result, Err(Error::ConfigParse(_))));
    }

    #[test]
    fn test_config_integer_percentage_parses_as_float() {
        let config = Config::from_toml("[admission]\nheadroom_pct = 20\n").unwrap();
        assert_eq!(config.admission.headroom_pct, 20.0);
    }

    #[test]
    fn test_config_user_rules_replace_default_rules() {
        let config = Config::from_toml(
            "[[classifier.rules]]\nprogram = \"python\"\nargs = [\"train.py\"]\nclass = \"gpu_heavy\"\n",
        )
        .unwrap();
        assert_eq!(
            config.classifier.rules,
            vec![HeavyRule::new("python", &["train.py"], JobClass::GpuHeavy)]
        );
    }

    #[test]
    fn test_config_rule_without_args_matches_any_arguments() {
        let config =
            Config::from_toml("[[classifier.rules]]\nprogram = \"ninja\"\nclass = \"cpu_heavy\"\n")
                .unwrap();
        assert!(config.classifier.rules[0].args.is_empty());
    }

    #[test]
    fn test_config_user_rules_keep_default_light_flags() {
        let config =
            Config::from_toml("[[classifier.rules]]\nprogram = \"ninja\"\nclass = \"cpu_heavy\"\n")
                .unwrap();
        let defaults = ClassifierConfig::default();
        assert_eq!(config.classifier.light_flags, defaults.light_flags);
        assert!(defaults.light_flags.contains(&"--help".to_owned()));
    }

    #[test]
    fn test_config_blank_light_flag_is_rejected() {
        let message = config_error("[classifier]\nlight_flags = [\"--help\", \"\"]\n");
        assert!(message.contains("classifier.light_flags"));
    }

    #[test]
    fn test_config_unknown_key_is_rejected() {
        let result = Config::from_toml("[leases]\nheartbeat_tll_secs = 90\n");
        assert!(matches!(result, Err(Error::ConfigParse(_))));
        let result = Config::from_toml("[classes.mixed]\nvram = 1024\n");
        assert!(matches!(result, Err(Error::ConfigParse(_))));
        let result = Config::from_toml("[classes.huge]\ncores = 32\n");
        assert!(matches!(result, Err(Error::ConfigParse(_))));
    }

    #[test]
    fn test_config_wrong_type_is_rejected() {
        let result = Config::from_toml("[daemon]\nport = \"eighty\"\n");
        assert!(matches!(result, Err(Error::ConfigParse(_))));
    }

    #[test]
    fn test_config_out_of_range_values_name_the_key() {
        assert!(config_error("[daemon]\nport = 0\n").contains("daemon.port"));
        assert!(config_error("[admission]\nheadroom_pct = 100\n").contains("headroom_pct"));
        assert!(config_error("[estimates]\nmin_runs = 0\n").contains("estimates.min_runs"));
        assert!(config_error("[estimates]\nmargin_pct = -5\n").contains("estimates.margin_pct"));
        assert!(config_error("[benchmark]\nquiet_cpu_pct = 120\n").contains("quiet_cpu_pct"));
        assert!(config_error("[benchmark]\nquiet_secs = 0\n").contains("quiet_secs"));
        let short_wait = "[benchmark]\nquiet_secs = 30\nmax_quiet_wait_secs = 20\n";
        assert!(config_error(short_wait).contains("max_quiet_wait_secs"));
        assert!(config_error("[leases]\nheartbeat_ttl_secs = 0\n").contains("heartbeat_ttl"));
        assert!(config_error("[classes.mixed]\ngpu_pct = 150\n").contains("classes.mixed"));
        assert!(config_error("[retention]\nlease_days = 0\n").contains("retention.lease_days"));
        assert!(config_error("[retention]\nhistory_days = 0\n").contains("history_days"));
        assert!(config_error("[retention]\nkeep_runs_per_job = 0\n").contains("keep_runs"));
        let small_cap = "[retention]\nkeep_runs_per_job = 50\nmax_job_runs = 10\n";
        assert!(config_error(small_cap).contains("retention.max_job_runs"));
    }

    #[test]
    fn test_config_load_invalid_file_names_the_file_and_the_key() {
        let path =
            env::temp_dir().join(format!("allot-shared-test-bad-{}.toml", std::process::id()));
        fs::write(&path, "[leases]\nheartbeat_ttl_secs = 0\n").unwrap();
        let loaded = Config::load(&path);
        fs::remove_file(&path).unwrap();
        let message = loaded.unwrap_err().to_string();
        assert!(
            message.starts_with(&path.display().to_string()),
            "{message}"
        );
        assert!(message.contains("leases.heartbeat_ttl_secs"), "{message}");
    }

    #[test]
    fn test_config_malformed_documents_fail_without_panicking() {
        let text = toml::to_string(&Config::default()).unwrap();
        for variant in mutations(&text, 0x5eed, 2_000) {
            match Config::from_toml(&variant) {
                Ok(config) => config.validate().unwrap(),
                Err(error) => assert!(!error.to_string().is_empty()),
            }
        }
    }

    #[test]
    fn test_config_wrongly_typed_value_names_its_key() {
        let text = toml::to_string(&Config::default()).unwrap();
        let mut checked = 0;
        for (index, line) in text.lines().enumerate() {
            let Some((key, _)) = line.split_once(" = ") else {
                continue;
            };
            for wrong in ["\"text\"", "[1, 2]", "{ x = 1 }", "-1", "true", "1.5", "0"] {
                let mut lines: Vec<&str> = text.lines().collect();
                let replaced = format!("{key} = {wrong}");
                lines[index] = &replaced;
                // A replacement of the right type and range is simply accepted.
                if let Err(error) = Config::from_toml(&lines.join("\n")) {
                    let message = error.to_string();
                    assert!(message.contains(key.trim()), "{key} = {wrong}: {message}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 50, "only {checked} rejections were checked");
    }

    #[test]
    fn test_config_problems_lists_every_invalid_value_with_its_key() {
        let config: Config = toml::from_str(
            "[daemon]\nport = 0\n\n[classes.mixed]\ncores = -1.0\ngpu_pct = 101.0\n\n\
             [presets.light]\ncores = 1.0\nram_mb = 1\n\n\
             [[classifier.rules]]\nprogram = \"cargo\"\nargs = [\"\"]\nclass = \"cpu_heavy\"\n",
        )
        .unwrap();
        let problems: Vec<String> = config.problems().iter().map(ToString::to_string).collect();
        assert_eq!(
            problems,
            [
                "daemon.port must not be 0",
                "classes.mixed.cores must be between 0 and 4096",
                "classes.mixed.gpu_pct must be between 0 and 100",
                "presets.light is the name of a class",
                "classifier.rules[0].args must not contain blank entries",
            ]
        );
        assert_eq!(
            config.validate().unwrap_err().to_string(),
            format!("invalid configuration: {}", problems.join("; "))
        );
    }

    #[test]
    fn test_config_blank_rule_program_is_rejected() {
        let message =
            config_error("[[classifier.rules]]\nprogram = \" \"\nclass = \"cpu_heavy\"\n");
        assert!(message.contains("classifier.rules[0]"));
    }

    #[test]
    fn test_config_load_missing_file_yields_defaults() {
        let path = env::temp_dir()
            .join("allot-shared-test-missing")
            .join("config.toml");
        assert_eq!(Config::load(&path).unwrap(), Config::default());
    }

    #[test]
    fn test_config_port_in_reads_the_port_of_an_otherwise_invalid_file() {
        let path = env::temp_dir().join(format!("allot-shared-port-{}.toml", std::process::id()));
        let port_of = |text: &str| {
            fs::write(&path, text).unwrap();
            Config::port_in(&path)
        };
        let invalid = "[daemon]\nport = 48000\n[leases]\nheartbeat_ttl_secs = 0\n";
        assert_eq!(port_of(invalid), Some(48_000));
        assert_eq!(port_of("[daemon]\nport = 0\n"), None);
        assert_eq!(port_of("[daemon]\nport = 70000\n"), None);
        assert_eq!(port_of("[daemon\nport = 48000\n"), None);
        assert_eq!(port_of("[leases]\nheartbeat_ttl_secs = 30\n"), None);
        fs::remove_file(&path).unwrap();
        assert_eq!(Config::port_in(&path), None);
    }

    #[test]
    fn test_config_load_existing_file_parses_it() {
        let path = env::temp_dir().join(format!("allot-shared-test-{}.toml", std::process::id()));
        fs::write(&path, "[hook]\nfail_open = false\n").unwrap();
        let loaded = Config::load(&path);
        fs::remove_file(&path).unwrap();
        assert!(!loaded.unwrap().hook.fail_open);
    }
}
