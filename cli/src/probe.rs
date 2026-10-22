use std::{
    env,
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use rh_core::ancestry::{ProcessNode, ancestry};
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::sample;

const ENV_VALUE_PREFIXES: [&str; 3] = ["CLAUDE", "ANTHROPIC", "MCP"];
const SECRET_MARKERS: [&str; 5] = ["TOKEN", "KEY", "SECRET", "PASSWORD", "AUTH"];

/// Arguments for `rh probe`.
#[derive(Debug, clap::Args)]
pub struct ProbeArgs {
    /// File the report is appended to.
    #[arg(long, required_unless_present = "sample")]
    out: Option<PathBuf>,
    /// Also record stdin. Only for hooks: an MCP server's stdin never closes.
    #[arg(long)]
    stdin: bool,
    /// Label written into the report header, for example `hook` or `mcp`.
    #[arg(long, default_value = "probe")]
    label: String,
    /// Print one machine sample per second instead of writing a launch report.
    #[arg(long, conflicts_with_all = ["out", "stdin"])]
    sample: bool,
    /// How many seconds `--sample` runs.
    #[arg(
        long,
        default_value_t = 10,
        requires = "sample",
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    secs: u64,
}

/// Appends a report on how this process was launched to `args.out`, or with `--sample`
/// prints live machine samples.
///
/// # Errors
///
/// Fails when the process table, stdin, the output file, or the sampler cannot be read or
/// written.
pub fn run(args: &ProbeArgs) -> anyhow::Result<()> {
    if args.sample {
        return sample::run(args.secs);
    }
    let out = args
        .out
        .as_deref()
        .context("--out is required without --sample")?;
    let report = build_report(args)?;
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(out)
        .and_then(|mut file| file.write_all(report.as_bytes()))
        .with_context(|| format!("writing {}", out.display()))
}

fn build_report(args: &ProbeArgs) -> anyhow::Result<String> {
    let chain = current_ancestry()?;
    let unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    let cwd = env::current_dir()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default();

    let mut report = String::new();
    writeln!(report, "=== rh probe [{}] unix={unix_secs}", args.label)?;
    writeln!(report, "cwd: {cwd}")?;
    writeln!(
        report,
        "-- ancestry (nearest first): pid ppid start name exe"
    )?;
    for node in &chain {
        writeln!(
            report,
            "  {} {} {} {} {}",
            node.pid,
            node.parent_pid
                .map_or_else(|| "-".to_owned(), |pid| pid.to_string()),
            node.start_time,
            node.name,
            node.exe.as_deref().unwrap_or("-"),
        )?;
    }

    let mut vars: Vec<(String, String)> = env::vars_os()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect();
    vars.sort();
    writeln!(
        report,
        "-- env (values for {ENV_VALUE_PREFIXES:?} prefixes)"
    )?;
    for (name, value) in &vars {
        if has_value_prefix(name) {
            writeln!(report, "  {name}={}", redact(name, value))?;
        }
    }
    let names: Vec<&str> = vars.iter().map(|(name, _)| name.as_str()).collect();
    writeln!(report, "-- env names: {}", names.join(" "))?;

    if args.stdin {
        let mut raw = Vec::new();
        io::stdin().read_to_end(&mut raw).context("reading stdin")?;
        writeln!(report, "-- stdin\n{}", String::from_utf8_lossy(&raw))?;
    }
    writeln!(report)?;
    Ok(report)
}

/// Returns this process followed by its ancestors, nearest first.
pub(crate) fn current_ancestry() -> anyhow::Result<Vec<ProcessNode>> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    let own_pid = sysinfo::get_current_pid().map_err(anyhow::Error::msg)?;
    Ok(ancestry(own_pid.as_u32(), |pid| {
        system.process(Pid::from_u32(pid)).map(to_node)
    }))
}

fn to_node(process: &Process) -> ProcessNode {
    ProcessNode {
        pid: process.pid().as_u32(),
        parent_pid: process.parent().map(Pid::as_u32),
        name: process.name().to_string_lossy().into_owned(),
        exe: process.exe().map(|path| path.display().to_string()),
        start_time: process.start_time(),
    }
}

fn has_value_prefix(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    ENV_VALUE_PREFIXES
        .iter()
        .any(|prefix| upper.starts_with(prefix))
}

fn redact<'a>(name: &str, value: &'a str) -> &'a str {
    let upper = name.to_ascii_uppercase();
    if SECRET_MARKERS.iter().any(|marker| upper.contains(marker)) {
        "<redacted>"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_ancestry_starts_at_own_process() {
        let chain = current_ancestry().unwrap();
        assert_eq!(chain[0].pid, std::process::id());
        assert!(
            chain.len() >= 2,
            "test runner should have a parent: {chain:?}"
        );
    }

    #[test]
    fn test_redact_secret_names_hides_value() {
        assert_eq!(redact("CLAUDE_CODE_OAUTH_TOKEN", "abc"), "<redacted>");
        assert_eq!(redact("anthropic_api_key", "abc"), "<redacted>");
    }

    #[test]
    fn test_redact_plain_names_keeps_value() {
        assert_eq!(redact("CLAUDE_CODE_ENTRYPOINT", "cli"), "cli");
    }

    #[test]
    fn test_has_value_prefix_is_case_insensitive() {
        assert!(has_value_prefix("ClaudeCode"));
        assert!(!has_value_prefix("PATH"));
    }
}
