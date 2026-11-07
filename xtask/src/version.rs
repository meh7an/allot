//! The one command that moves the version. GNOME's and KDE's metadata and the Windows installer
//! are absent on purpose: the release workflows write the version into them at packaging.

use std::{
    env, fs,
    ops::Range,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const CHANGELOG: &str = "CHANGELOG.md";
const LOCK: &str = "Cargo.lock";
const COMPARE: &str = "https://github.com/meh7an/allot/compare/";
const UNRELEASED: &str = "## [Unreleased]\n";
const WORKSPACE: &str = "Cargo.toml";

/// Lines in one tracked file that hold the version, between `open` and `close`. An empty `close`
/// means the version runs to the end of the line.
#[derive(Debug)]
struct Pin {
    path: &'static str,
    line_start: &'static str,
    open: &'static str,
    close: &'static str,
}

const fn path_dependencies(path: &'static str) -> Pin {
    Pin {
        path,
        line_start: "allot-",
        open: "version = \"",
        close: "\"",
    }
}

const fn install_snippet(path: &'static str) -> Pin {
    Pin {
        path,
        line_start: "version=",
        open: "version=",
        close: "",
    }
}

const PINS: &[Pin] = &[
    Pin {
        path: WORKSPACE,
        line_start: "version = \"",
        open: "version = \"",
        close: "\"",
    },
    path_dependencies("cli/Cargo.toml"),
    path_dependencies("client/Cargo.toml"),
    path_dependencies("core/Cargo.toml"),
    path_dependencies("daemon/Cargo.toml"),
    path_dependencies("mcp/Cargo.toml"),
    path_dependencies("sampler/Cargo.toml"),
    Pin {
        path: "tray/src/Allot.Tray/Allot.Tray.csproj",
        line_start: "<Version>",
        open: "<Version>",
        close: "</Version>",
    },
    Pin {
        path: "apps/macos/project.yml",
        line_start: "MARKETING_VERSION:",
        open: "MARKETING_VERSION: \"",
        close: "\"",
    },
    install_snippet("README.md"),
    install_snippet("docs/install.md"),
];

/// Sets `new` in every pinned file, promotes the changelog's Unreleased section and refreshes
/// `Cargo.lock`, which the release builds read with `--locked`.
pub fn set(root: &Path, new: &str) -> Result<String, String> {
    let new_parts = parse(new)?;
    check(root)?;
    let old = workspace_version(&read(root, WORKSPACE)?)?;
    if new_parts <= parse(&old)? {
        return Err(format!("{new} is not newer than the current version {old}"));
    }

    // Everything is worked out before the first write, so a refusal leaves the tree untouched.
    let mut edits = vec![(
        CHANGELOG,
        promote_changelog(&read(root, CHANGELOG)?, &old, new, &today_utc()?)?,
    )];
    for pin in PINS {
        edits.push((pin.path, rewrite(pin, &read(root, pin.path)?, new)?));
    }
    for (path, text) in &edits {
        fs::write(root.join(path), text).map_err(|error| format!("{path}: {error}"))?;
    }

    refresh_lock(root)?;
    check(root)?;
    Ok(format!(
        "Set {new} in {} files and {LOCK}, and promoted {CHANGELOG}.\nReview the diff, commit it, \
         then start the draft release:\n  git tag v{new}\n  git push origin main v{new}",
        PINS.len()
    ))
}

/// Checks that every pinned file and `Cargo.lock` hold the workspace version, and that the
/// changelog has a dated, non-empty section and links for it.
pub fn check(root: &Path) -> Result<String, String> {
    let version = workspace_version(&read(root, WORKSPACE)?)?;
    let mut problems = Vec::new();
    for pin in PINS {
        let text = read(root, pin.path)?;
        let found = pinned_versions(pin, &text);
        if found.is_empty() {
            problems.push(format!("{}: no line holds the version", pin.path));
        }
        for other in found.into_iter().filter(|found| *found != version) {
            problems.push(format!(
                "{}: has {other}, the workspace has {version}",
                pin.path
            ));
        }
    }
    for name in stale_lock_packages(&read(root, LOCK)?, &version) {
        problems.push(format!("{LOCK}: {name} is not at {version}"));
    }
    problems.extend(changelog_problems(&read(root, CHANGELOG)?, &version));

    if problems.is_empty() {
        Ok(format!("Every place holds {version}."))
    } else {
        Err(problems.join("\n"))
    }
}

fn read(root: &Path, path: &str) -> Result<String, String> {
    fs::read_to_string(root.join(path)).map_err(|error| format!("{path}: {error}"))
}

fn parse(version: &str) -> Result<[u64; 3], String> {
    let invalid = || format!("{version} is not a version like 1.2.3");
    let parts: Vec<&str> = version.split('.').collect();
    let [major, minor, patch] = parts.as_slice() else {
        return Err(invalid());
    };
    let number = |part: &str| {
        let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        if !digits || (part.len() > 1 && part.starts_with('0')) {
            return Err(invalid());
        }
        part.parse().map_err(|_| invalid())
    };
    Ok([number(major)?, number(minor)?, number(patch)?])
}

fn workspace_version(manifest: &str) -> Result<String, String> {
    let pin = &PINS[0];
    match pinned_versions(pin, manifest).as_slice() {
        [version] => Ok((*version).to_string()),
        found => Err(format!(
            "{}: expected one version line, found {}",
            pin.path,
            found.len()
        )),
    }
}

fn pinned_range(pin: &Pin, line: &str) -> Option<Range<usize>> {
    if !line.trim_start().starts_with(pin.line_start) {
        return None;
    }
    let start = line.find(pin.open)? + pin.open.len();
    let rest = &line[start..];
    let length = if pin.close.is_empty() {
        rest.trim_end().len()
    } else {
        rest.find(pin.close)?
    };
    Some(start..start + length)
}

fn pinned_versions<'a>(pin: &Pin, text: &'a str) -> Vec<&'a str> {
    text.lines()
        .filter_map(|line| pinned_range(pin, line).map(|range| &line[range]))
        .collect()
}

fn rewrite(pin: &Pin, text: &str, new: &str) -> Result<String, String> {
    let mut rewritten = String::with_capacity(text.len());
    let mut found = false;
    for line in text.split_inclusive('\n') {
        if let Some(range) = pinned_range(pin, line) {
            rewritten.push_str(&line[..range.start]);
            rewritten.push_str(new);
            rewritten.push_str(&line[range.end..]);
            found = true;
        } else {
            rewritten.push_str(line);
        }
    }
    if found {
        Ok(rewritten)
    } else {
        Err(format!("{}: no line holds the version", pin.path))
    }
}

fn promote_changelog(text: &str, old: &str, new: &str, date: &str) -> Result<String, String> {
    if text.contains(&format!("## [{new}]")) {
        return Err(format!("{CHANGELOG} already has a section for {new}"));
    }
    let start = text
        .find(UNRELEASED)
        .map(|index| index + UNRELEASED.len())
        .ok_or_else(|| format!("{CHANGELOG} has no [Unreleased] section"))?;
    let end = text[start..]
        .find("\n## [")
        .map(|index| start + index + 1)
        .ok_or_else(|| format!("{CHANGELOG} has no section after [Unreleased]"))?;
    let body = &text[start..end];
    if body.trim().is_empty() {
        return Err(format!(
            "{CHANGELOG} has nothing under [Unreleased], so no release notes"
        ));
    }
    let old_link = format!("[Unreleased]: {COMPARE}v{old}...HEAD");
    if !text.contains(&old_link) {
        return Err(format!("{CHANGELOG} has no line {old_link}"));
    }
    let new_links =
        format!("[Unreleased]: {COMPARE}v{new}...HEAD\n[{new}]: {COMPARE}v{old}...v{new}");
    let (before, after) = (&text[..start], &text[end..]);
    Ok(format!("{before}\n## [{new}] - {date}\n{body}{after}").replace(&old_link, &new_links))
}

fn changelog_problems(text: &str, version: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let heading = format!("## [{version}] - ");
    match text.split_once(&heading) {
        None => problems.push(format!("{CHANGELOG}: no dated section for {version}")),
        Some((_, rest)) => {
            let section = rest.split("\n## [").next().unwrap_or_default();
            let body = section.split_once('\n').map_or("", |(_date, body)| body);
            if body.trim().is_empty() {
                problems.push(format!("{CHANGELOG}: the section for {version} is empty"));
            }
        }
    }
    let unreleased = format!("[Unreleased]: {COMPARE}v{version}...HEAD");
    if !text.contains(&unreleased) {
        problems.push(format!("{CHANGELOG}: no line {unreleased}"));
    }
    if !text.contains(&format!("\n[{version}]: ")) {
        problems.push(format!("{CHANGELOG}: no link for {version}"));
    }
    problems
}

fn stale_lock_packages(lock: &str, version: &str) -> Vec<String> {
    let expected = format!("version = \"{version}\"");
    let mut stale = Vec::new();
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        let name = line
            .strip_prefix("name = \"")
            .and_then(|rest| rest.strip_suffix('"'));
        if let Some(name) = name.filter(|name| name.starts_with("allot-") || *name == "xtask")
            && lines.next() != Some(expected.as_str())
        {
            stale.push(name.to_string());
        }
    }
    stale
}

fn refresh_lock(root: &Path) -> Result<(), String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .args(["update", "--workspace"])
        .current_dir(root)
        .status()
        .map_err(|error| format!("cargo update: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo update: {status}"))
    }
}

fn today_utc() -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    Ok(date_from_days(now.as_secs() / 86_400))
}

/// Howard Hinnant's `civil_from_days`, for one date without a date crate. Eras are 400 years.
fn date_from_days(days: u64) -> String {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANGELOG_TEXT: &str = "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- A thing.\n\n\
        ## [0.3.0] - 2026-11-04\n\nOld notes.\n\n\
        [Unreleased]: https://github.com/meh7an/allot/compare/v0.3.0...HEAD\n\
        [0.3.0]: https://github.com/meh7an/allot/compare/v0.2.0...v0.3.0\n";

    fn pin_for(path: &str) -> &'static Pin {
        PINS.iter()
            .find(|pin| pin.path == path)
            .expect("the path is pinned")
    }

    #[test]
    fn test_rewrite_workspace_manifest_leaves_rust_version() {
        let text = "[workspace.package]\nversion = \"0.3.0\"\nrust-version = \"1.99\"\n";
        let rewritten = rewrite(pin_for(WORKSPACE), text, "0.4.0").unwrap();
        assert_eq!(
            rewritten,
            "[workspace.package]\nversion = \"0.4.0\"\nrust-version = \"1.99\"\n"
        );
    }

    #[test]
    fn test_rewrite_path_dependencies_leaves_other_lines() {
        let text = "name = \"allot-cli\"\nallot-core = { version = \"0.3.0\", path = \"../core\" }\n\
            anyhow = \"1.0.104\"\n";
        let rewritten = rewrite(pin_for("cli/Cargo.toml"), text, "0.4.0").unwrap();
        assert_eq!(
            rewritten,
            "name = \"allot-cli\"\nallot-core = { version = \"0.4.0\", path = \"../core\" }\n\
            anyhow = \"1.0.104\"\n"
        );
    }

    #[test]
    fn test_rewrite_tray_project_leaves_package_versions() {
        let pin = pin_for("tray/src/Allot.Tray/Allot.Tray.csproj");
        let text = "    <Version>0.3.0</Version>\n    <PackageReference Version=\"2.3.12\" />\n";
        let rewritten = rewrite(pin, text, "0.4.0").unwrap();
        assert_eq!(
            rewritten,
            "    <Version>0.4.0</Version>\n    <PackageReference Version=\"2.3.12\" />\n"
        );
    }

    #[test]
    fn test_rewrite_mac_project_leaves_build_number() {
        let text = "    MARKETING_VERSION: \"0.3.0\"\n    CURRENT_PROJECT_VERSION: \"1\"\n";
        let rewritten = rewrite(pin_for("apps/macos/project.yml"), text, "0.4.0").unwrap();
        assert_eq!(
            rewritten,
            "    MARKETING_VERSION: \"0.4.0\"\n    CURRENT_PROJECT_VERSION: \"1\"\n"
        );
    }

    #[test]
    fn test_rewrite_install_snippet_keeps_line_ending_and_sample_output() {
        let text = "version=0.3.0\r\nallotd 0.3.0 (pid 1)\r\n";
        let rewritten = rewrite(pin_for("README.md"), text, "0.4.0").unwrap();
        assert_eq!(rewritten, "version=0.4.0\r\nallotd 0.3.0 (pid 1)\r\n");
    }

    #[test]
    fn test_rewrite_no_matching_line_is_an_error() {
        assert!(rewrite(pin_for("README.md"), "nothing here\n", "0.4.0").is_err());
    }

    #[test]
    fn test_parse_only_plain_versions_pass() {
        assert_eq!(parse("0.10.2"), Ok([0, 10, 2]));
        for bad in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "v1.2.3",
            "1.2.3-rc1",
            "01.2.3",
            "1.2.+3",
            "1..3",
        ] {
            assert!(parse(bad).is_err(), "{bad} should not parse");
        }
    }

    #[test]
    fn test_parse_compares_numerically() {
        assert!(parse("0.10.0").unwrap() > parse("0.9.9").unwrap());
    }

    #[test]
    fn test_promote_changelog_moves_notes_and_rewrites_links() {
        let promoted = promote_changelog(CHANGELOG_TEXT, "0.3.0", "0.4.0", "2026-12-01").unwrap();
        assert_eq!(
            promoted,
            "# Changelog\n\n## [Unreleased]\n\n## [0.4.0] - 2026-12-01\n\n### Added\n\n- A thing.\n\n\
            ## [0.3.0] - 2026-11-04\n\nOld notes.\n\n\
            [Unreleased]: https://github.com/meh7an/allot/compare/v0.4.0...HEAD\n\
            [0.4.0]: https://github.com/meh7an/allot/compare/v0.3.0...v0.4.0\n\
            [0.3.0]: https://github.com/meh7an/allot/compare/v0.2.0...v0.3.0\n"
        );
        assert!(changelog_problems(&promoted, "0.4.0").is_empty());
    }

    #[test]
    fn test_promote_changelog_empty_unreleased_is_refused() {
        let text = CHANGELOG_TEXT.replace("### Added\n\n- A thing.\n\n", "");
        let error = promote_changelog(&text, "0.3.0", "0.4.0", "2026-12-01").unwrap_err();
        assert!(error.contains("nothing under [Unreleased]"), "{error}");
    }

    #[test]
    fn test_promote_changelog_existing_section_is_refused() {
        assert!(promote_changelog(CHANGELOG_TEXT, "0.2.0", "0.3.0", "2026-12-01").is_err());
    }

    #[test]
    fn test_changelog_problems_missing_section_is_reported() {
        let problems = changelog_problems(CHANGELOG_TEXT, "0.4.0");
        assert_eq!(problems.len(), 3, "{problems:?}");
    }

    #[test]
    fn test_changelog_problems_empty_section_is_reported() {
        let text = CHANGELOG_TEXT.replace("Old notes.\n\n", "");
        let text = text.replace(
            "\n\n[Unreleased]",
            "\n## [0.2.0] - 2026-01-01\n\n[Unreleased]",
        );
        let problems = changelog_problems(&text, "0.3.0");
        assert_eq!(
            problems,
            [format!("{CHANGELOG}: the section for 0.3.0 is empty")]
        );
    }

    #[test]
    fn test_stale_lock_packages_only_workspace_members_count() {
        let lock = "[[package]]\nname = \"allot-cli\"\nversion = \"0.3.0\"\n\n\
            [[package]]\nname = \"anyhow\"\nversion = \"1.0.104\"\n\n\
            [[package]]\nname = \"xtask\"\nversion = \"0.2.0\"\n";
        assert_eq!(stale_lock_packages(lock, "0.3.0"), ["xtask"]);
    }

    #[test]
    fn test_date_from_days_known_dates() {
        assert_eq!(date_from_days(0), "1970-01-01");
        assert_eq!(date_from_days(19_782), "2024-02-29");
        assert_eq!(date_from_days(20_762), "2026-11-05");
    }

    // Runs in CI's `check` job, so a bump by hand that misses a place fails on the next push.
    #[test]
    fn test_check_this_repository_agrees() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        check(&root).unwrap();
    }
}
