//! What the Settings window's live explanations answer, as `POST /settings/preview` and
//! `POST /classify` send it: how many jobs of each size fit on this machine, and how the hook
//! treats a command line. `allot_core::explain` works the numbers out.

use serde::{Deserialize, Serialize};

use crate::types::{JobClass, JobProfile};

/// The shell a command line is written for. Quoting and escapes differ between them.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Shell {
    /// bash and other POSIX shells: `\` escapes.
    #[default]
    Bash,
    /// PowerShell, also used for `cmd /c` scripts: `` ` `` escapes and `\` is literal.
    #[serde(rename = "powershell")]
    PowerShell,
}

/// How many jobs of one size start, one after another, in a given room.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Fits {
    /// Jobs that get every core they ask for.
    pub full: u32,
    /// Jobs after those that start shrunk, each counted at its minimum cores.
    pub shrunk: u32,
    /// Whether more would still fit after the count's limit of 99 jobs.
    pub more: bool,
}

/// What fits of one class or preset.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SizeFit {
    /// The class's key, such as `cpu_heavy`, or the preset's name.
    pub name: String,
    /// The fewest cores a job of it starts with.
    pub min_cores: f32,
    /// Beside the active leases and the load right now.
    pub now: Fits,
    /// With every lease ended; load outside leases stays.
    pub idle: Fits,
}

/// What the headroom keeps free of the whole machine.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KeptFree {
    /// Logical cores kept free.
    pub cores: f64,
    /// The machine's logical cores.
    pub total_cores: u32,
    /// Memory kept free.
    pub ram_mb: f64,
    /// The machine's memory.
    pub total_ram_mb: u64,
    /// Share of the primary GPU kept free, or `None` without a GPU.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_pct: Option<f64>,
    /// Video memory kept free, or `None` when there is no GPU, its size is unknown, or it
    /// shares RAM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_mb: Option<f64>,
    /// The primary GPU's video memory, under the same conditions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_vram_mb: Option<u64>,
}

/// What fits on this machine with proposed settings.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitPreview {
    /// One per class, in [`JobClass::ALL`] order.
    pub classes: Vec<SizeFit>,
    /// One per preset, by name.
    pub presets: Vec<SizeFit>,
    /// What the proposed headroom keeps free.
    pub kept_free: KeptFree,
    /// The machine's CPU load, as the 10 s average the counts use.
    pub cpu_busy_pct: f32,
}

/// A class with the size the proposed settings give it.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClassSize {
    /// The lease class.
    pub class: JobClass,
    /// What a lease of it asks for.
    pub profile: JobProfile,
    /// The fewest cores a lease of it starts with: its own minimum, or the automatic one.
    pub min_cores: f32,
}

/// How the rules treat one simple command.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CommandKind {
    /// A rule matches, so it needs a lease.
    Heavy,
    /// A rule matches, but a light flag makes it only print and exit.
    PrintOnly,
    /// No rule matches.
    NotHeavy,
}

/// One simple command of a tested line.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    /// Its words joined by spaces, wrappers such as `sudo` included.
    pub command: String,
    /// How the proposed rules treat it.
    pub kind: CommandKind,
    /// Position of the first matching rule, from 0, for a heavy command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<u32>,
    /// The first light flag among its arguments, for a print-only command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
    /// The lease a heavy command needs on its own; `None` for the others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<ClassSize>,
}

/// How the hook treats a command line under proposed rules.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandTest {
    /// The one lease the hook asks for: the class of every heavy command combined. `None`
    /// when no command is heavy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<ClassSize>,
    /// Every simple command of the line, in order.
    pub commands: Vec<CommandResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_json_names_are_lowercase() {
        assert_eq!(serde_json::to_string(&Shell::Bash).unwrap(), "\"bash\"");
        assert_eq!(
            serde_json::to_string(&Shell::PowerShell).unwrap(),
            "\"powershell\""
        );
    }
}
