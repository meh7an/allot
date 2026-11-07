//! `allot doctor`: checks the install without changing anything, and says what to do about
//! each problem.

use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

use allot_client::BlockingClient;
use allot_shared::{
    api::HealthResponse,
    config::Config,
    paths::Paths,
    platform::{ALLOT_BINARIES, CLI_EXE},
};
use anstyle::Style;
use anyhow::bail;
use serde::Serialize;

use crate::{
    output::{BAD, GOOD, WAIT, paint, print_json},
    registration::{self, State},
};

/// The fix for anything `allot install` sets up: running it again.
const REINSTALL: &str =
    "run the one-line install again (README, Install), or `allot install` from a release folder";
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Set by the npm package's launcher (`npm/bin/allot.js`) on the allot it runs.
const NPM_LAUNCHER_ENV: &str = "ALLOT_NPM_LAUNCHER";

/// How one check came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Ok,
    /// Works, but not as it should, or could not be told.
    Warn,
    /// Allot cannot work this way.
    Fail,
}

impl Outcome {
    fn label(self) -> (&'static str, Style) {
        match self {
            Self::Ok => ("ok", GOOD),
            Self::Warn => ("warn", WAIT),
            Self::Fail => ("fail", BAD),
        }
    }
}

/// One line of the report; `--json` prints a list of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Check {
    name: &'static str,
    outcome: Outcome,
    detail: String,
    /// What to do; `None` when there is nothing to do.
    fix: Option<String>,
}

impl Check {
    fn ok(name: &'static str, detail: String) -> Self {
        Self {
            name,
            outcome: Outcome::Ok,
            detail,
            fix: None,
        }
    }

    fn problem(name: &'static str, outcome: Outcome, detail: String, fix: &str) -> Self {
        Self {
            name,
            outcome,
            detail,
            fix: Some(fix.to_owned()),
        }
    }
}

/// Checks the binaries, `PATH`, the config, allotd, and what `allot install` registers, prints
/// the report as text or JSON, and fails when a check failed.
///
/// # Errors
///
/// Fails when the home directory is unknown, or after the report when a check failed.
pub fn doctor(json: bool) -> anyhow::Result<()> {
    let paths = Paths::from_env()?;
    let mut checks = vec![
        check_binaries(&paths),
        check_path(&paths, env::var_os(NPM_LAUNCHER_ENV)),
    ];
    checks.extend(check_config_and_daemon(&paths));
    // Warnings, not failures: `--no-claude` and `--no-autostart` leave these out on purpose.
    checks.extend(
        registration::inspect(&paths).map(|inspection| match inspection.state {
            State::Done => Check::ok(inspection.name, inspection.detail),
            State::Missing | State::Unknown => {
                Check::problem(inspection.name, Outcome::Warn, inspection.detail, REINSTALL)
            }
        }),
    );
    if json {
        print_json(&checks)?;
    } else {
        anstream::print!("{}", format_report(&checks));
    }
    let failed = checks
        .iter()
        .filter(|check| check.outcome == Outcome::Fail)
        .count();
    if failed > 0 {
        bail!("{failed} check(s) failed");
    }
    Ok(())
}

fn format_report(checks: &[Check]) -> String {
    let width = checks
        .iter()
        .map(|check| check.name.len())
        .max()
        .unwrap_or(0);
    let mut text = String::new();
    for check in checks {
        let (label, style) = check.outcome.label();
        text.push_str(&format!(
            "{}  {:<width$}  {}\n",
            paint(style, &format!("{label:<4}")),
            check.name,
            check.detail
        ));
        if let Some(fix) = &check.fix {
            text.push_str(&format!("      {:<width$}  fix: {fix}\n", ""));
        }
    }
    let problems = checks
        .iter()
        .filter(|check| check.outcome != Outcome::Ok)
        .count();
    if problems == 0 {
        text.push_str("\nall checks passed\n");
    }
    text
}

fn check_binaries(paths: &Paths) -> Check {
    let bin = paths.bin_dir();
    let missing: Vec<&str> = ALLOT_BINARIES
        .into_iter()
        .filter(|name| !bin.join(name).is_file())
        .collect();
    if missing.is_empty() {
        return Check::ok("binaries", format!("{} has all three", bin.display()));
    }
    Check::problem(
        "binaries",
        Outcome::Fail,
        format!("{} lacks {}", bin.display(), missing.join(", ")),
        REINSTALL,
    )
}

/// `npm_launcher` is [`NPM_LAUNCHER_ENV`]'s value.
fn check_path(paths: &Paths, npm_launcher: Option<OsString>) -> Check {
    // npm's `allot` is a launcher for the binary in its own package, so on PATH it is never this
    // install's file, and on Windows it is a .cmd that the lookup below cannot see.
    if let Some(launcher) = npm_launcher.filter(|value| !value.is_empty()) {
        return Check::ok(
            "path",
            format!(
                "allot runs through npm's launcher {}",
                Path::new(&launcher).display()
            ),
        );
    }
    let installed = paths.bin_dir().join(CLI_EXE);
    let path = env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = env::split_paths(&path).collect();
    match first_on_path(&dirs, CLI_EXE) {
        Some(found) if is_this_install(&found, &installed) => {
            Check::ok("path", format!("allot runs {}", found.display()))
        }
        Some(found) => Check::problem(
            "path",
            Outcome::Warn,
            format!("allot on PATH is {}, not this install", found.display()),
            &format!(
                "remove or rename {}, or put {} first on PATH",
                found.display(),
                paths.bin_dir().display()
            ),
        ),
        None => Check::problem(
            "path",
            Outcome::Warn,
            "allot is not on PATH".to_owned(),
            &format!("add {} to PATH", paths.bin_dir().display()),
        ),
    }
}

/// The first `name` in `dirs`, as a shell would find it.
fn first_on_path(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter()
        .map(|dir| dir.join(name))
        .find(|file| file.is_file())
}

/// Whether `found` is `installed`, directly or through a link such as `~/.local/bin/allot`.
fn is_this_install(found: &Path, installed: &Path) -> bool {
    let canonical = |path: &Path| path.canonicalize().ok();
    found == installed || canonical(found).is_some_and(|found| Some(found) == canonical(installed))
}

/// The config file, then allotd: whether it runs, its version, and whether it took the config.
fn check_config_and_daemon(paths: &Paths) -> Vec<Check> {
    let config = match Config::load(&paths.config_file()) {
        Ok(_) => Check::ok(
            "config",
            format!("{} is valid or absent", paths.config_file().display()),
        ),
        Err(error) => Check::problem(
            "config",
            Outcome::Fail,
            error.to_string(),
            "fix the file; allotd keeps the previous config until it is valid",
        ),
    };
    let client = match BlockingClient::discover(false) {
        Ok(client) => client,
        Err(error) => {
            let daemon = Check::problem(
                "allotd",
                Outcome::Fail,
                format!("{error:#}"),
                "fix the config file, then `allot daemon start`",
            );
            return vec![config, daemon];
        }
    };
    let daemon = match client.health() {
        Ok(health) => check_running(&health, client.base_url()),
        Err(error) if error.is_unreachable() => Check::problem(
            "allotd",
            Outcome::Warn,
            format!("not running at {}", client.base_url()),
            "`allot daemon start`; sessions also start it on their first heavy command",
        ),
        Err(error) => Check::problem(
            "allotd",
            Outcome::Fail,
            format!("{error:#}"),
            "`allot daemon stop`, then `allot daemon start`",
        ),
    };
    vec![config, daemon]
}

fn check_running(health: &HealthResponse, url: &str) -> Check {
    if let Some(error) = &health.config_error {
        return Check::problem(
            "allotd",
            Outcome::Fail,
            format!("allotd {} rejected the config: {error}", health.version),
            "fix the config file; allotd reloads it by itself",
        );
    }
    if health.version != VERSION {
        return Check::problem(
            "allotd",
            Outcome::Warn,
            format!(
                "allotd {} runs at {url}, but this allot is {VERSION}",
                health.version
            ),
            "`allot daemon stop`, then `allot daemon start`, to run the installed build",
        );
    }
    Check::ok(
        "allotd",
        format!(
            "allotd {} running at {url} (pid {})",
            health.version, health.pid
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, process};

    use super::*;
    use crate::output::plain;

    fn health(version: &str, config_error: Option<&str>) -> HealthResponse {
        HealthResponse {
            version: version.into(),
            pid: 4_242,
            uptime_secs: 5,
            config_error: config_error.map(Into::into),
        }
    }

    #[test]
    fn test_check_running_flags_another_version_and_a_rejected_config() {
        let url = "http://127.0.0.1:47800";
        assert_eq!(
            check_running(&health(VERSION, None), url).outcome,
            Outcome::Ok
        );
        let old = check_running(&health("0.0.1", None), url);
        assert_eq!(old.outcome, Outcome::Warn);
        assert!(old.detail.contains("allotd 0.0.1 runs"), "{}", old.detail);
        let rejected = check_running(&health(VERSION, Some("bad cores")), url);
        assert_eq!(rejected.outcome, Outcome::Fail);
        assert!(
            rejected.detail.ends_with("bad cores"),
            "{}",
            rejected.detail
        );
    }

    #[test]
    fn test_first_on_path_takes_the_earliest_directory() {
        let root = env::temp_dir().join(format!("allot-doctor-{}", process::id()));
        let (first, second) = (root.join("a"), root.join("b"));
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::write(second.join(CLI_EXE), "").unwrap();
        let dirs = [root.join("missing"), first.clone(), second.clone()];
        assert_eq!(first_on_path(&dirs, CLI_EXE), Some(second.join(CLI_EXE)));
        fs::write(first.join(CLI_EXE), "").unwrap();
        assert_eq!(first_on_path(&dirs, CLI_EXE), Some(first.join(CLI_EXE)));
        assert!(is_this_install(
            &first.join(CLI_EXE),
            &first.join(".").join(CLI_EXE)
        ));
        assert!(!is_this_install(
            &first.join(CLI_EXE),
            &second.join(CLI_EXE)
        ));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn test_check_path_accepts_the_npm_launcher() {
        let paths = Paths::under(env::temp_dir().join("allot-doctor-npm"));
        let launcher = Path::new("npm").join("bin").join("allot.js");
        let check = check_path(&paths, Some(launcher.clone().into_os_string()));
        assert_eq!(check.outcome, Outcome::Ok);
        assert!(
            check.detail.ends_with(&launcher.display().to_string()),
            "{}",
            check.detail
        );
        assert_ne!(
            check_path(&paths, Some(OsString::new())).detail,
            check.detail
        );
    }

    #[test]
    fn test_format_report_puts_each_fix_under_its_check() {
        let checks = [
            Check::ok("binaries", "all three".into()),
            Check::problem("path", Outcome::Warn, "not on PATH".into(), "add it"),
        ];
        let text = plain(format_report(&checks));
        assert_eq!(
            text,
            "ok    binaries  all three\nwarn  path      not on PATH\n                fix: add \
             it\n"
        );
        let healthy = plain(format_report(&checks[..1]));
        assert!(healthy.ends_with("\nall checks passed\n"), "{healthy}");
    }

    #[test]
    fn test_check_json_names_the_outcome_in_snake_case() {
        let check = Check::problem("path", Outcome::Warn, "x".into(), "y");
        assert_eq!(
            serde_json::to_value(&check).unwrap(),
            serde_json::json!({ "name": "path", "outcome": "warn", "detail": "x", "fix": "y" })
        );
    }
}
