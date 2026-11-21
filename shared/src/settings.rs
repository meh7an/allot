//! The settings spec: every key of `config.toml` once, with the page and group it shows in, its
//! control, range, default, label and help. allotd serves it, the contract generates the
//! panels' types from it, and tests hold it to [`Config`] and the runbook's Configuration table.
//!
//! The labels and help come from `design/settings/DESIGN.md` section 4. Defaults are read from
//! [`Config::default`], so the spec cannot disagree with the daemon.

mod values;

use serde::{Deserialize, Serialize};

pub use self::values::{SettingValue, SizeRow};
use crate::{
    config::{Config, HeavyRule, MIN_HEARTBEAT_TTL_SECS, PRESET_EST_SECS, Sizing},
    types::{JobClass, JobProfile, MAX_CORES, QUICK_MAX_SECS},
};

/// Every allotd setting, page by page, in the order the Settings window shows them.
///
/// Panels add what holds no key themselves: the Panel page of their own settings, live
/// explanations, the command test, the service status, the config file row and Reset.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsSpec {
    /// The pages, in window order.
    pub pages: Vec<SettingsPageSpec>,
}

/// A page of the Settings window.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsPageSpec {
    /// Stable identifier, such as `admission`.
    pub id: String,
    /// The page's title.
    pub label: String,
    /// A sentence under the title, if the page has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<String>,
    /// The groups, in page order.
    pub groups: Vec<SettingsGroupSpec>,
}

/// A titled group of settings on a page.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsGroupSpec {
    /// Stable identifier, unique on its page, such as `learn`.
    pub id: String,
    /// The group's title.
    pub label: String,
    /// The settings, in group order.
    pub settings: Vec<SettingSpec>,
}

/// One setting: a key of `config.toml`, or a field of a table row or rule.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingSpec {
    /// The TOML key: a dotted path such as `admission.headroom_pct` for a setting on a page,
    /// or the key inside a row, such as `cores`, for a field of a table or rule.
    pub key: String,
    /// A noun phrase in sentence case; a switch's label describes the on state.
    pub label: String,
    /// One or two plain sentences. Panels show the key after them.
    pub help: String,
    /// How the value is edited, with its default and range.
    pub control: SettingControl,
    /// Whether the setting is rarely changed and shows last on its page, under Advanced.
    pub advanced: bool,
    /// Whether a change takes effect only once allotd restarts.
    pub needs_restart: bool,
    /// The operating systems whose panels show the setting; empty means every one.
    pub platforms: Vec<Platform>,
    /// A switch that must be on for this setting to act; panels disable it while it is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Requirement>,
}

/// How a setting is edited.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SettingControl {
    /// On or off.
    Switch {
        /// The value while the key is not in the file.
        default: bool,
    },
    /// A number in a unit, written to the file in that unit.
    Number {
        /// What the number counts, which picks its unit and display.
        quantity: Quantity,
        /// Whether only whole numbers are allowed.
        integer: bool,
        /// The value while the key is not in the file. In a table's fields, the value a new
        /// row starts with, or `None` when a new row must give one or `automatic` applies.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<f64>,
        /// The smallest value allowed, if any besides `min_key`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<Bound>,
        /// The largest value allowed, if any besides `max_key`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<Bound>,
        /// A setting this value must be at least, by key; in a table, a field of the same row.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_key: Option<String>,
        /// A setting this value must be at most, by key; in a table, a field of the same row.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_key: Option<String>,
        /// A switch in front of the number, when one value turns the feature off.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        off: Option<OffSwitch>,
        /// How the value is worked out while the key is left out, when it may be, shown
        /// beside an Automatic choice.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        automatic: Option<String>,
    },
    /// A line of text, never blank.
    Text {
        /// A regular expression the whole text must match, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
    },
    /// One of a fixed set of values.
    Choice {
        /// The values in display order.
        options: Vec<ChoiceOption>,
        /// The value a new item starts with, or `None` when one must be picked.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
    },
    /// A list of strings without blank entries; a list in the file replaces the default list.
    StringList {
        /// The list while the key is not in the file.
        default: Vec<String>,
    },
    /// Named rows of sizes, such as `[classes.cpu_heavy]`, edited one row at a time. A row
    /// in the file changes only the fields it names.
    SizeTable {
        /// The built-in rows with their defaults, which always exist.
        rows: Vec<SizeRowSpec>,
        /// The fields of every row.
        fields: Vec<SettingSpec>,
        /// How a row added by the user is named, or `None` when rows cannot be added,
        /// renamed or deleted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_row_name: Option<RowName>,
    },
    /// An ordered list of heavy-command rules, always written whole. A list in the file, even
    /// an empty one, replaces the default list; removing the key brings the default back.
    RuleList {
        /// The list while the key is not in the file.
        default: Vec<HeavyRule>,
        /// The fields of every rule.
        fields: Vec<SettingSpec>,
    },
}

/// What a number counts.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Quantity {
    /// A share from 0 to 100.
    Percent,
    /// A number of job runs.
    Runs,
    /// A number of days.
    Days,
    /// A duration in seconds; panels may show it in minutes or hours.
    Seconds,
    /// Logical cores; fractions are allowed.
    Cores,
    /// Memory in megabytes; panels may show it in gigabytes.
    Megabytes,
    /// A TCP port, shown without digit grouping.
    Port,
}

impl Quantity {
    /// Whether values of this quantity are whole numbers.
    pub fn is_integer(self) -> bool {
        !matches!(self, Self::Percent | Self::Cores)
    }
}

/// An operating system a panel runs on.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Platform {
    /// The Windows tray.
    Windows,
    /// The macOS menu bar app.
    Macos,
    /// The GNOME extension and the KDE widget.
    Linux,
}

/// One end of a number's range.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bound {
    /// The limit.
    pub value: f64,
    /// Whether the limit itself is out of range.
    pub exclusive: bool,
}

/// A switch that turns a numeric feature off by writing one value.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OffSwitch {
    /// The value written while the switch is off.
    pub value: f64,
    /// The switch's label, describing the on state.
    pub label: String,
    /// The switch's help.
    pub help: String,
}

/// A switch another setting depends on.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    /// The switch's key.
    pub key: String,
    /// Shown under the disabled setting while the switch is off.
    pub note: String,
}

/// One value of a choice.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    /// The value written to the file.
    pub value: String,
    /// The value as the panel shows it.
    pub label: String,
}

/// A built-in row of a size table.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SizeRowSpec {
    /// The row's key in the file, such as `cpu_heavy`.
    pub name: String,
    /// The row as the panel shows it.
    pub label: String,
    /// The row's values while the file does not change them.
    pub default: JobProfile,
    /// The row's minimum cores while the file names none; `None` follows the field's
    /// automatic rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_min_cores: Option<f32>,
}

/// The name field of a row the user adds or renames.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowName {
    /// The field's label.
    pub label: String,
    /// The field's help.
    pub help: String,
    /// A regular expression the whole name must match. A name must also differ from every
    /// other row's and from every class's.
    pub pattern: String,
}

impl SettingsSpec {
    /// The spec of this build of allotd, with the defaults of [`Config::default`].
    pub fn new() -> Self {
        let config = Config::default();
        Self {
            pages: vec![
                admission(&config),
                sizes(&config),
                commands(&config),
                benchmarks(&config),
                leases(&config),
                service(&config),
            ],
        }
    }

    /// Every setting on every page, in window order.
    pub fn settings(&self) -> impl Iterator<Item = &SettingSpec> {
        self.pages
            .iter()
            .flat_map(|page| &page.groups)
            .flat_map(|group| &group.settings)
    }

    /// The setting on a page whose key is `key`.
    pub fn get(&self, key: &str) -> Option<&SettingSpec> {
        self.settings().find(|setting| setting.key == key)
    }
}

impl Default for SettingsSpec {
    fn default() -> Self {
        Self::new()
    }
}

/// The class as the panels show it, for example "CPU heavy".
pub fn class_label(class: JobClass) -> &'static str {
    match class {
        JobClass::Light => "Light",
        JobClass::CpuHeavy => "CPU heavy",
        JobClass::GpuHeavy => "GPU heavy",
        JobClass::Mixed => "Mixed",
    }
}

const LOWERING_KEY: &str = "estimates.lower";
const LOWERING_NOTE: &str = "Used only while lowering is on.";
const SCOPES_KEY: &str = "linux.scopes";
const SCOPES_NOTE: &str = "Needs systemd scopes.";

fn admission(config: &Config) -> SettingsPageSpec {
    let estimates = &config.estimates;
    page(
        "admission",
        "Admission",
        None,
        vec![
            group(
                "keep_free",
                "Keep free for your desktop",
                vec![setting(
                    "admission.headroom_pct",
                    "Headroom",
                    "Share of cores, memory, GPU and video memory that jobs never get, so your \
                     desktop stays responsive.",
                    number(
                        Quantity::Percent,
                        f64::from(config.admission.headroom_pct),
                        Limits::default().at_least(0.0).below(100.0),
                    ),
                )],
            ),
            group(
                "learn",
                "Learn from past runs",
                vec![
                    setting(
                        LOWERING_KEY,
                        "Lower requests that past runs show are too big",
                        "Once a command has used less than it asks for, reserve what it really \
                         uses. Requests always rise when runs used more. Benchmarks never lower.",
                        switch(estimates.lower),
                    ),
                    setting(
                        "estimates.min_runs",
                        "Runs before lowering",
                        "Clean past runs of the same command in the same folder needed before \
                         its request lowers.",
                        number(
                            Quantity::Runs,
                            f64::from(estimates.min_runs),
                            Limits::default().at_least(1.0),
                        ),
                    )
                    .requires(LOWERING_KEY, LOWERING_NOTE),
                    setting(
                        "estimates.margin_pct",
                        "Safety margin",
                        "Added on top of what past runs used, so a slightly heavier run still \
                         fits.",
                        number(
                            Quantity::Percent,
                            f64::from(estimates.margin_pct),
                            Limits::percent(),
                        ),
                    )
                    .requires(LOWERING_KEY, LOWERING_NOTE),
                ],
            ),
        ],
    )
}

fn sizes(config: &Config) -> SettingsPageSpec {
    let classes = JobClass::ALL
        .iter()
        .map(|&class| {
            size_row(
                class.as_str(),
                class_label(class),
                config.classes.get(class),
            )
        })
        .collect();
    let presets = config
        .presets
        .0
        .iter()
        .map(|(name, sizing)| size_row(name, name, sizing))
        .collect();
    page(
        "sizes",
        "Sizes",
        Some(
            "Each heavy command reserves the size of its class. Presets are sizes a session can \
             ask for by name.",
        ),
        vec![
            group(
                "classes",
                "Classes",
                vec![setting(
                    "classes",
                    "Classes",
                    "The size each class reserves. The four classes are fixed.",
                    SettingControl::SizeTable {
                        rows: classes,
                        fields: size_fields(),
                        new_row_name: None,
                    },
                )],
            ),
            group(
                "presets",
                "Presets",
                vec![setting(
                    "presets",
                    "Presets",
                    "Sizes a session asks for by name. Built-in presets can be changed and \
                     reset, not renamed or deleted.",
                    SettingControl::SizeTable {
                        rows: presets,
                        fields: size_fields(),
                        new_row_name: Some(RowName {
                            label: "Name".into(),
                            help: "Lowercase letters, digits and _. Sessions ask for the preset \
                                   by this name."
                                .into(),
                            pattern: "^[a-z0-9_]+$".into(),
                        }),
                    },
                )],
            ),
        ],
    )
}

fn size_row(name: &str, label: &str, sizing: &Sizing) -> SizeRowSpec {
    SizeRowSpec {
        name: name.into(),
        label: label.into(),
        default: sizing.profile,
        default_min_cores: sizing.min_cores,
    }
}

/// The fields of a class or preset. Their defaults are what a new preset starts with.
fn size_fields() -> Vec<SettingSpec> {
    vec![
        setting(
            "cores",
            "Cores",
            "Cores a job of this size reserves. Fractions such as 0.5 are allowed.",
            field_number(
                Quantity::Cores,
                None,
                Limits::default()
                    .at_least(0.0)
                    .at_most(f64::from(MAX_CORES)),
            ),
        ),
        setting(
            "min_cores",
            "Minimum cores",
            "When the machine is busy, a job may start with fewer cores, down to this.",
            field_number(
                Quantity::Cores,
                None,
                Limits::default()
                    .above(0.0)
                    .at_most_key("cores")
                    .automatic("half the cores, at least 1"),
            ),
        ),
        setting(
            "ram_mb",
            "Memory",
            "Memory a job of this size reserves.",
            field_number(Quantity::Megabytes, None, Limits::default().at_least(0.0)),
        ),
        setting(
            "gpu_pct",
            "GPU",
            "Share of the GPU's engines a job reserves.",
            field_number(Quantity::Percent, Some(0.0), Limits::percent()),
        ),
        setting(
            "vram_mb",
            "Video memory",
            "GPU memory a job reserves.",
            field_number(
                Quantity::Megabytes,
                Some(0.0),
                Limits::default().at_least(0.0),
            ),
        ),
        setting(
            "est_secs",
            "Expected run time",
            format!(
                "How long a job of this size usually runs. Jobs of {QUICK_MAX_SECS} s or less \
                 may start while a benchmark waits."
            ),
            field_number(
                Quantity::Seconds,
                Some(whole(PRESET_EST_SECS)),
                Limits::default().at_least(0.0),
            ),
        ),
    ]
}

fn commands(config: &Config) -> SettingsPageSpec {
    let classifier = &config.classifier;
    page(
        "commands",
        "Commands",
        None,
        vec![
            group(
                "rules",
                "Heavy-command rules",
                vec![setting(
                    "classifier.rules",
                    "Heavy-command rules",
                    "Checked in order against every simple command in a command line. Your list \
                     replaces the built-in rules.",
                    SettingControl::RuleList {
                        default: classifier.rules.clone(),
                        fields: rule_fields(),
                    },
                )],
            ),
            group(
                "print_only",
                "Print-only flags",
                vec![setting(
                    "classifier.light_flags",
                    "Print-only flags",
                    "A command with any of these only prints and exits, so it never needs a \
                     lease, even when a rule matches.",
                    SettingControl::StringList {
                        default: classifier.light_flags.clone(),
                    },
                )],
            ),
        ],
    )
}

fn rule_fields() -> Vec<SettingSpec> {
    let classes = JobClass::ALL
        .iter()
        .map(|&class| ChoiceOption {
            value: class.as_str().into(),
            label: class_label(class).into(),
        })
        .collect();
    vec![
        setting(
            "program",
            "Program",
            "The program's name without folder or extension, such as cargo or python*. * \
             matches any characters, and case is ignored.",
            SettingControl::Text { pattern: None },
        ),
        setting(
            "args",
            "Arguments",
            "Any one of these makes the command heavy; leave empty to match any arguments. A \
             pattern starting with - matches that flag anywhere; others match the first argument \
             that is not a flag.",
            SettingControl::StringList {
                default: Vec::new(),
            },
        ),
        setting(
            "class",
            "Lease class",
            "The size a matching command reserves.",
            SettingControl::Choice {
                options: classes,
                default: None,
            },
        ),
    ]
}

fn benchmarks(config: &Config) -> SettingsPageSpec {
    let benchmark = &config.benchmark;
    page(
        "benchmarks",
        "Benchmarks",
        None,
        vec![group(
            "quiet",
            "Quiet machine",
            vec![
                setting(
                    "benchmark.quiet_cpu_pct",
                    "CPU load limit",
                    "A benchmark waits until total CPU load stays at or below this.",
                    number(
                        Quantity::Percent,
                        f64::from(benchmark.quiet_cpu_pct),
                        Limits::percent(),
                    ),
                ),
                setting(
                    "benchmark.quiet_gpu_pct",
                    "GPU load limit",
                    "A benchmark waits until GPU load stays at or below this.",
                    number(
                        Quantity::Percent,
                        f64::from(benchmark.quiet_gpu_pct),
                        Limits::percent(),
                    ),
                ),
                setting(
                    "benchmark.quiet_secs",
                    "Quiet for",
                    "How long both loads must stay under their limits, once no other lease is \
                     active.",
                    number(
                        Quantity::Seconds,
                        whole(benchmark.quiet_secs),
                        Limits::default().above(0.0),
                    ),
                ),
                setting(
                    "benchmark.max_quiet_wait_secs",
                    "Longest wait",
                    "After this, a waiting benchmark starts anyway, marked not quiet.",
                    number(
                        Quantity::Seconds,
                        whole(benchmark.max_quiet_wait_secs),
                        Limits::default().at_least_key("benchmark.quiet_secs"),
                    ),
                ),
            ],
        )],
    )
}

fn leases(config: &Config) -> SettingsPageSpec {
    let retention = &config.retention;
    page(
        "leases",
        "Leases",
        None,
        vec![
            group(
                "leases",
                "Leases",
                vec![
                    setting(
                        "leases.heartbeat_ttl_secs",
                        "Lease timeout",
                        "A lease ends when its session stops checking in for this long.",
                        number(
                            Quantity::Seconds,
                            whole(config.leases.heartbeat_ttl_secs),
                            Limits::default().at_least(MIN_HEARTBEAT_TTL_SECS as f64),
                        ),
                    ),
                    setting(
                        "leases.idle_release_secs",
                        "Idle for",
                        "How long the session must be idle first.",
                        number(
                            Quantity::Seconds,
                            whole(config.leases.idle_release_secs),
                            Limits::default().above(0.0).off(
                                0.0,
                                "End leases left idle",
                                "Ends a lease once nothing runs under it and its session has \
                                 been idle, for sessions that forget to release.",
                            ),
                        ),
                    ),
                ],
            ),
            group(
                "history",
                "History",
                vec![
                    setting(
                        "retention.lease_days",
                        "Keep ended leases for",
                        "Ended leases and their audit trail are deleted after this. Cleanup runs \
                         every hour.",
                        number(
                            Quantity::Days,
                            whole(retention.lease_days),
                            Limits::default().at_least(1.0),
                        ),
                    )
                    .advanced(),
                    setting(
                        "retention.history_days",
                        "Keep job runs for",
                        "Job runs are deleted after this, except each command's newest runs \
                         below.",
                        number(
                            Quantity::Days,
                            whole(retention.history_days),
                            Limits::default().at_least(1.0),
                        ),
                    )
                    .advanced(),
                    setting(
                        "retention.keep_runs_per_job",
                        "Always keep per command",
                        "The newest runs of each command stay whatever their age, so estimates \
                         survive long breaks. Estimates read the newest 20.",
                        number(
                            Quantity::Runs,
                            whole(retention.keep_runs_per_job),
                            Limits::default().at_least(1.0),
                        ),
                    )
                    .advanced(),
                    setting(
                        "retention.max_job_runs",
                        "Most runs kept",
                        "Beyond this, the oldest runs are deleted, whatever the command.",
                        number(
                            Quantity::Runs,
                            whole(retention.max_job_runs),
                            Limits::default().at_least_key("retention.keep_runs_per_job"),
                        ),
                    )
                    .advanced(),
                ],
            ),
        ],
    )
}

fn service(config: &Config) -> SettingsPageSpec {
    let linux = &config.linux;
    page(
        "service",
        "Service",
        None,
        vec![
            group(
                "connection",
                "Connection",
                vec![
                    setting(
                        "hook.fail_open",
                        "Let heavy commands run while the service is down",
                        "When off, every heavy command is blocked until the service answers \
                         again.",
                        switch(config.hook.fail_open),
                    ),
                    setting(
                        "daemon.port",
                        "Port",
                        "The service listens on this port on 127.0.0.1. Change it only if \
                         another app needs the port.",
                        number(
                            Quantity::Port,
                            f64::from(config.daemon.port),
                            Limits::default().at_least(1.0).at_most(f64::from(u16::MAX)),
                        ),
                    )
                    .advanced()
                    .needs_restart(),
                ],
            ),
            group(
                "linux",
                "Linux",
                vec![
                    setting(
                        SCOPES_KEY,
                        "Run each lease in its own systemd scope",
                        "Gives exact CPU and memory use, and a kill that ends every process at \
                         once. Needs cgroup v2 and a systemd user session.",
                        switch(linux.scopes),
                    )
                    .on(Platform::Linux),
                    setting(
                        "linux.enforce_cpu",
                        "Cap jobs at their reserved cores",
                        "A job that outgrows its lease is slowed down, not stopped.",
                        switch(linux.enforce_cpu),
                    )
                    .on(Platform::Linux)
                    .requires(SCOPES_KEY, SCOPES_NOTE),
                    setting(
                        "linux.enforce_memory",
                        "Slow down jobs above their reserved memory",
                        "Memory above the reservation is throttled. A job is never stopped for \
                         memory.",
                        switch(linux.enforce_memory),
                    )
                    .on(Platform::Linux)
                    .requires(SCOPES_KEY, SCOPES_NOTE),
                ],
            ),
        ],
    )
}

fn page(
    id: &str,
    label: &str,
    intro: Option<&str>,
    groups: Vec<SettingsGroupSpec>,
) -> SettingsPageSpec {
    SettingsPageSpec {
        id: id.into(),
        label: label.into(),
        intro: intro.map(Into::into),
        groups,
    }
}

fn group(id: &str, label: &str, settings: Vec<SettingSpec>) -> SettingsGroupSpec {
    SettingsGroupSpec {
        id: id.into(),
        label: label.into(),
        settings,
    }
}

fn setting(
    key: &str,
    label: &str,
    help: impl Into<String>,
    control: SettingControl,
) -> SettingSpec {
    SettingSpec {
        key: key.into(),
        label: label.into(),
        help: help.into(),
        control,
        advanced: false,
        needs_restart: false,
        platforms: Vec::new(),
        requires: None,
    }
}

impl SettingSpec {
    fn advanced(self) -> Self {
        Self {
            advanced: true,
            ..self
        }
    }

    fn needs_restart(self) -> Self {
        Self {
            needs_restart: true,
            ..self
        }
    }

    fn on(mut self, platform: Platform) -> Self {
        self.platforms.push(platform);
        self
    }

    fn requires(self, key: &str, note: &str) -> Self {
        Self {
            requires: Some(Requirement {
                key: key.into(),
                note: note.into(),
            }),
            ..self
        }
    }
}

fn switch(default: bool) -> SettingControl {
    SettingControl::Switch { default }
}

fn number(quantity: Quantity, default: f64, limits: Limits) -> SettingControl {
    field_number(quantity, Some(default), limits)
}

fn field_number(quantity: Quantity, default: Option<f64>, limits: Limits) -> SettingControl {
    let Limits {
        min,
        max,
        min_key,
        max_key,
        off,
        automatic,
    } = limits;
    SettingControl::Number {
        quantity,
        integer: quantity.is_integer(),
        default,
        min,
        max,
        min_key,
        max_key,
        off,
        automatic,
    }
}

/// A config integer as a spec number; every config integer is far below 2^53, so it is exact.
fn whole(value: u64) -> f64 {
    value as f64
}

/// The optional parts of a number's control, built up one call at a time.
#[derive(Default)]
struct Limits {
    min: Option<Bound>,
    max: Option<Bound>,
    min_key: Option<String>,
    max_key: Option<String>,
    off: Option<OffSwitch>,
    automatic: Option<String>,
}

impl Limits {
    fn percent() -> Self {
        Self::default().at_least(0.0).at_most(100.0)
    }

    fn at_least(self, value: f64) -> Self {
        self.with_min(value, false)
    }

    fn above(self, value: f64) -> Self {
        self.with_min(value, true)
    }

    fn at_most(self, value: f64) -> Self {
        self.with_max(value, false)
    }

    fn below(self, value: f64) -> Self {
        self.with_max(value, true)
    }

    fn with_min(self, value: f64, exclusive: bool) -> Self {
        Self {
            min: Some(Bound { value, exclusive }),
            ..self
        }
    }

    fn with_max(self, value: f64, exclusive: bool) -> Self {
        Self {
            max: Some(Bound { value, exclusive }),
            ..self
        }
    }

    fn at_least_key(self, key: &str) -> Self {
        Self {
            min_key: Some(key.into()),
            ..self
        }
    }

    fn at_most_key(self, key: &str) -> Self {
        Self {
            max_key: Some(key.into()),
            ..self
        }
    }

    fn off(self, value: f64, label: &str, help: &str) -> Self {
        Self {
            off: Some(OffSwitch {
                value,
                label: label.into(),
                help: help.into(),
            }),
            ..self
        }
    }

    fn automatic(self, rule: &str) -> Self {
        Self {
            automatic: Some(rule.into()),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::PathBuf};

    use super::*;
    use crate::Error;

    fn spec() -> SettingsSpec {
        SettingsSpec::new()
    }

    /// Every leaf key under `prefix` in `value`; the rows of an array of tables share `[]`.
    fn leaf_keys(prefix: &str, value: &toml::Value, keys: &mut BTreeSet<String>) {
        match value {
            toml::Value::Table(table) => {
                for (key, value) in table {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    leaf_keys(&path, value, keys);
                }
            }
            toml::Value::Array(items) if items.first().is_some_and(toml::Value::is_table) => {
                for item in items {
                    leaf_keys(&format!("{prefix}[]"), item, keys);
                }
            }
            _ => {
                keys.insert(prefix.to_owned());
            }
        }
    }

    /// Every leaf key of the default config, with `min_cores` set on every class and preset,
    /// since the file leaves it out while unset.
    fn config_keys() -> BTreeSet<String> {
        let mut config = Config::default();
        let classes = &mut config.classes;
        let sizings = [
            &mut classes.light,
            &mut classes.cpu_heavy,
            &mut classes.gpu_heavy,
            &mut classes.mixed,
        ]
        .into_iter()
        .chain(config.presets.0.values_mut());
        for sizing in sizings {
            sizing.min_cores = Some(sizing.minimum());
        }
        let mut keys = BTreeSet::new();
        leaf_keys("", &toml::Value::try_from(&config).unwrap(), &mut keys);
        keys
    }

    /// Every leaf key the spec describes, spelled as [`config_keys`] spells them.
    fn spec_keys(spec: &SettingsSpec) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        for setting in spec.settings() {
            let key = &setting.key;
            match &setting.control {
                SettingControl::SizeTable { rows, fields, .. } => {
                    for row in rows {
                        for field in fields {
                            keys.insert(format!("{key}.{}.{}", row.name, field.key));
                        }
                    }
                }
                SettingControl::RuleList { fields, .. } => {
                    for field in fields {
                        keys.insert(format!("{key}[].{}", field.key));
                    }
                }
                _ => {
                    keys.insert(key.clone());
                }
            }
        }
        keys
    }

    #[test]
    fn test_settings_spec_covers_every_config_key_and_no_other() {
        assert_eq!(spec_keys(&spec()), config_keys());
    }

    #[test]
    fn test_settings_spec_covers_learned_estimates_and_minimum_cores() {
        let keys = spec_keys(&spec());
        let expected = [
            "estimates.lower",
            "estimates.min_runs",
            "estimates.margin_pct",
            "classes.cpu_heavy.min_cores",
            "classes.light.min_cores",
            "presets.low_impact.min_cores",
            "presets.quick.min_cores",
        ];
        for key in expected {
            assert!(keys.contains(key), "{key} has no spec entry");
        }
    }

    #[test]
    fn test_settings_spec_lists_every_builtin_class_and_preset() {
        let spec = spec();
        let rows = |key| match &spec.get(key).unwrap().control {
            SettingControl::SizeTable { rows, .. } => {
                rows.iter().map(|row| row.name.clone()).collect::<Vec<_>>()
            }
            other => panic!("{key} is not a size table: {other:?}"),
        };
        let classes: Vec<String> = JobClass::ALL
            .iter()
            .map(|class| class.as_str().into())
            .collect();
        assert_eq!(rows("classes"), classes);
        let config = Config::default();
        let presets: Vec<String> = config.presets.names().map(Into::into).collect();
        assert_eq!(rows("presets"), presets);
    }

    #[test]
    fn test_settings_spec_keys_are_unique_and_references_resolve() {
        let spec = spec();
        let mut seen = BTreeSet::new();
        for setting in spec.settings() {
            assert!(seen.insert(&setting.key), "{} appears twice", setting.key);
            if let Some(requirement) = &setting.requires {
                let target = spec.get(&requirement.key).map(|target| &target.control);
                let is_switch = matches!(target, Some(SettingControl::Switch { .. }));
                assert!(is_switch, "{} requires a missing switch", setting.key);
            }
            for (_, other) in number_references(&setting.control) {
                let target = spec.get(other).map(|target| &target.control);
                let is_number = matches!(target, Some(SettingControl::Number { .. }));
                assert!(
                    is_number,
                    "{} refers to a missing number {other}",
                    setting.key
                );
            }
            let fields = match &setting.control {
                SettingControl::SizeTable { fields, .. }
                | SettingControl::RuleList { fields, .. } => fields.as_slice(),
                _ => &[],
            };
            for field in fields {
                for (_, other) in number_references(&field.control) {
                    let found = fields.iter().any(|sibling| sibling.key == other);
                    assert!(
                        found,
                        "{}.{} refers to a missing field",
                        setting.key, field.key
                    );
                }
            }
        }
    }

    /// The `min_key` and `max_key` of a number, with `true` for a minimum.
    fn number_references(control: &SettingControl) -> Vec<(bool, &str)> {
        let SettingControl::Number {
            min_key, max_key, ..
        } = control
        else {
            return Vec::new();
        };
        let minimum = min_key.as_deref().map(|key| (true, key));
        let maximum = max_key.as_deref().map(|key| (false, key));
        minimum.into_iter().chain(maximum).collect()
    }

    /// The TOML spelling of `value` for a number that is `integer` or not.
    fn toml_number(value: f64, integer: bool) -> toml::Value {
        if integer {
            toml::Value::Integer(value as i64)
        } else {
            toml::Value::Float(value)
        }
    }

    /// Sets the dotted `path` in `document`, creating the tables on the way.
    fn insert(document: &mut toml::Table, path: &str, value: toml::Value) {
        let (tables, leaf) = path.rsplit_once('.').unwrap_or(("", path));
        let mut table = document;
        for name in tables.split('.').filter(|name| !name.is_empty()) {
            table = table
                .entry(name)
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .unwrap();
        }
        table.insert(leaf.to_owned(), value);
    }

    #[test]
    fn test_settings_spec_defaults_written_out_rebuild_the_default_config() {
        let mut document = toml::Table::new();
        for setting in spec().settings() {
            let key = &setting.key;
            match &setting.control {
                SettingControl::Switch { default } => {
                    insert(&mut document, key, toml::Value::Boolean(*default));
                }
                SettingControl::Number {
                    integer, default, ..
                } => insert(&mut document, key, toml_number(default.unwrap(), *integer)),
                SettingControl::StringList { default } => {
                    insert(&mut document, key, toml::Value::try_from(default).unwrap());
                }
                SettingControl::SizeTable { rows, .. } => {
                    for row in rows {
                        let path = format!("{key}.{}", row.name);
                        insert(
                            &mut document,
                            &path,
                            toml::Value::try_from(row.default).unwrap(),
                        );
                        if let Some(min_cores) = row.default_min_cores {
                            let value = toml::Value::Float(f64::from(min_cores));
                            insert(&mut document, &format!("{path}.min_cores"), value);
                        }
                    }
                }
                SettingControl::RuleList { default, .. } => {
                    insert(&mut document, key, toml::Value::try_from(default).unwrap());
                }
                other => panic!("{key}: no TOML spelling for {other:?}"),
            }
        }
        let text = toml::to_string(&document).unwrap();
        assert_eq!(
            Config::from_toml(&text).unwrap(),
            Config::default(),
            "{text}"
        );
    }

    /// A number setting checked against `Config::validate`: where it is written, its control,
    /// and the default of each setting a `min_key` or `max_key` names.
    struct NumberCase<'a> {
        path: String,
        control: &'a SettingControl,
        reference: Option<f64>,
    }

    fn number_cases(spec: &SettingsSpec) -> Vec<NumberCase<'_>> {
        let scalar_default = |key: &str| match spec.get(key).map(|setting| &setting.control) {
            Some(SettingControl::Number { default, .. }) => *default,
            _ => None,
        };
        let mut cases = Vec::new();
        for setting in spec.settings() {
            match &setting.control {
                control @ SettingControl::Number { .. } => {
                    let reference = number_references(control)
                        .first()
                        .and_then(|(_, key)| scalar_default(key));
                    cases.push(NumberCase {
                        path: setting.key.clone(),
                        control,
                        reference,
                    });
                }
                SettingControl::SizeTable { rows, fields, .. } => {
                    let row = &rows[0];
                    for field in fields {
                        // The only field another one refers to is `cores`.
                        let reference = (!number_references(&field.control).is_empty())
                            .then_some(f64::from(row.default.cores));
                        cases.push(NumberCase {
                            path: format!("{}.{}.{}", setting.key, row.name, field.key),
                            control: &field.control,
                            reference,
                        });
                    }
                }
                _ => {}
            }
        }
        cases
    }

    /// Values just inside and just outside each limit of `case`, with whether each is valid.
    fn probes(case: &NumberCase) -> Vec<(f64, bool)> {
        let SettingControl::Number { min, max, off, .. } = case.control else {
            unreachable!("number_cases holds numbers only");
        };
        let mut probes = Vec::new();
        if let Some(Bound { value, exclusive }) = *min {
            probes.push((if exclusive { value + 1.0 } else { value }, true));
            probes.push((if exclusive { value } else { value - 1.0 }, false));
        }
        if let Some(Bound { value, exclusive }) = *max {
            probes.push((if exclusive { value - 1.0 } else { value }, true));
            probes.push((if exclusive { value } else { value + 1.0 }, false));
        }
        for (is_minimum, _) in number_references(case.control) {
            let reference = case.reference.unwrap();
            let outside = if is_minimum {
                reference - 1.0
            } else {
                reference + 1.0
            };
            probes.push((reference, true));
            probes.push((outside, false));
        }
        // The off value is valid although it is outside the range.
        let off = off.as_ref().map(|off| off.value);
        probes.retain(|&(value, valid)| valid || Some(value) != off);
        probes
    }

    #[test]
    fn test_settings_spec_number_limits_match_config_validation() {
        let spec = spec();
        let mut checked = 0;
        for case in number_cases(&spec) {
            let SettingControl::Number { integer, .. } = case.control else {
                unreachable!("number_cases holds numbers only");
            };
            let (table, leaf) = case.path.rsplit_once('.').unwrap();
            for (value, valid) in probes(&case) {
                let value = toml_number(value, *integer);
                let text = format!("[{table}]\n{leaf} = {value}\n");
                match Config::from_toml(&text) {
                    Ok(_) => assert!(valid, "accepted out of range:\n{text}"),
                    Err(error @ (Error::Config(_) | Error::ConfigParse(_))) => {
                        let message = error.to_string();
                        assert!(!valid, "rejected in range:\n{text}{message}");
                        assert!(message.contains(leaf), "{message}");
                    }
                    Err(other) => panic!("{text}: {other}"),
                }
                checked += 1;
            }
        }
        assert!(checked > 40, "only {checked} values were checked");
    }

    fn runbook_section() -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/runbook.md");
        let text = fs::read_to_string(&path).unwrap();
        let (_, after) = text
            .split_once("\n## Configuration\n")
            .expect("the runbook has a Configuration section");
        after.split("\n## ").next().unwrap().to_owned()
    }

    /// The keys named in the first column of the Configuration table. A later code span of a
    /// cell without a dot shares the first span's table: `benchmark.quiet_cpu_pct`,
    /// `quiet_gpu_pct`. Table rows such as `[classes.<class>]` give their table's key.
    fn runbook_keys(section: &str) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        for line in section.lines().filter(|line| line.starts_with("| `")) {
            let cell = line.split('|').nth(1).unwrap();
            let mut table = "";
            for (index, span) in cell.split('`').skip(1).step_by(2).enumerate() {
                let span = span.trim_matches(|char| char == '[' || char == ']');
                let span = span.split(".<").next().unwrap();
                if index == 0 {
                    table = span.rsplit_once('.').map_or("", |(table, _)| table);
                    keys.insert(span.to_owned());
                } else if span.contains('.') {
                    keys.insert(span.to_owned());
                } else {
                    keys.insert(format!("{table}.{span}"));
                }
            }
        }
        keys
    }

    #[test]
    fn test_runbook_configuration_table_lists_every_key_and_no_other() {
        let spec = spec();
        let section = runbook_section();
        let documented = runbook_keys(&section);
        let described: BTreeSet<String> = spec.settings().map(|s| s.key.clone()).collect();
        let missing: Vec<_> = described.difference(&documented).collect();
        let unknown: Vec<_> = documented.difference(&described).collect();
        assert!(
            missing.is_empty() && unknown.is_empty(),
            "docs/runbook.md Configuration table: missing {missing:?}, unknown {unknown:?}"
        );
        for setting in spec.settings() {
            if let SettingControl::SizeTable { fields, .. } = &setting.control {
                for field in fields {
                    let named = section.contains(&format!("`{}`", field.key));
                    assert!(named, "docs/runbook.md never names `{}`", field.key);
                }
            }
        }
    }

    #[test]
    fn test_settings_spec_round_trips_through_json() {
        let spec = spec();
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(serde_json::from_str::<SettingsSpec>(&json).unwrap(), spec);
    }
}
