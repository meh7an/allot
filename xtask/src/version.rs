//! The one command that moves the version. GNOME's and KDE's metadata and the Windows installer
//! are absent on purpose: the release workflows write the version into them at packaging.

//!
//! A pre-release such as `0.6.0-beta.1` sets the same places except the stable-only ones, and
//! leaves the changelog's Unreleased section in place: its notes come from there, and the
//! release after it promotes them as usual.

use std::{
    cmp::Ordering,
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
/// means the version runs to the end of the line. A `stable_only` pin keeps the last release
/// while the workspace is at a pre-release, because it tells users what to install.
#[derive(Debug)]
struct Pin {
    path: &'static str,
    line_start: &'static str,
    open: &'static str,
    close: &'static str,
    stable_only: bool,
}

const fn path_dependencies(path: &'static str) -> Pin {
    Pin {
        path,
        line_start: "allot-",
        open: "version = \"",
        close: "\"",
        stable_only: false,
    }
}

const fn install_snippet(path: &'static str) -> Pin {
    Pin {
        path,
        line_start: "version=",
        open: "version=",
        close: "",
        stable_only: true,
    }
}

const PINS: &[Pin] = &[
    Pin {
        path: WORKSPACE,
        line_start: "version = \"",
        open: "version = \"",
        close: "\"",
        stable_only: false,
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
        stable_only: false,
    },
    Pin {
        path: "apps/macos/project.yml",
        line_start: "MARKETING_VERSION:",
        open: "MARKETING_VERSION: \"",
        close: "\"",
        stable_only: false,
    },
    install_snippet("docs/install.md"),
];

/// Sets `new` in every pinned file and refreshes `Cargo.lock`, which the release builds read
/// with `--locked`. A release also promotes the changelog's Unreleased section; a pre-release
/// only needs that section to hold notes.
pub fn set(root: &Path, new: &str) -> Result<String, String> {
    let new_version = Version::parse(new)?;
    check(root)?;
    let old = workspace_version(&read(root, WORKSPACE)?)?;
    if new_version <= Version::parse(&old)? {
        return Err(format!("{new} is not newer than the current version {old}"));
    }
    let pre_release = new_version.is_pre_release();

    // Everything is worked out before the first write, so a refusal leaves the tree untouched.
    let changelog = read(root, CHANGELOG)?;
    let mut edits = Vec::new();
    if pre_release {
        unreleased_notes(&changelog)?;
    } else {
        edits.push((
            CHANGELOG,
            promote_changelog(&changelog, new, &today_utc()?)?,
        ));
    }
    let pins: Vec<&Pin> = PINS
        .iter()
        .filter(|pin| !(pre_release && pin.stable_only))
        .collect();
    for pin in &pins {
        edits.push((pin.path, rewrite(pin, &read(root, pin.path)?, new)?));
    }
    for (path, text) in &edits {
        fs::write(root.join(path), text).map_err(|error| format!("{path}: {error}"))?;
    }

    refresh_lock(root)?;
    check(root)?;
    let (changelog_note, publish_note) = if pre_release {
        (
            format!("left {CHANGELOG}'s [Unreleased] for the notes"),
            "Publish it as a pre-release: no package manager gets it.",
        )
    } else {
        (
            format!("promoted {CHANGELOG}"),
            "Publish it to start every channel.",
        )
    };
    Ok(format!(
        "Set {new} in {} files and {LOCK}, and {changelog_note}.\nReview the diff, commit it, \
         then start the draft release:\n  git tag v{new}\n  git push origin main v{new}\n\
         {publish_note}",
        pins.len()
    ))
}

/// Checks that every pinned file and `Cargo.lock` hold the workspace version, and that the
/// changelog has a dated, non-empty section and links for it. At a pre-release, stable-only pins
/// hold an older release and the notes are still under Unreleased.
pub fn check(root: &Path) -> Result<String, String> {
    let version = workspace_version(&read(root, WORKSPACE)?)?;
    let parsed = Version::parse(&version)?;
    let mut problems = Vec::new();
    for pin in PINS {
        let text = read(root, pin.path)?;
        let found = pinned_versions(pin, &text);
        if found.is_empty() {
            problems.push(format!("{}: no line holds the version", pin.path));
        }
        for other in found {
            if let Some(problem) = pin_problem(pin, other, &version, &parsed) {
                problems.push(format!("{}: {problem}", pin.path));
            }
        }
    }
    for name in stale_lock_packages(&read(root, LOCK)?, &version) {
        problems.push(format!("{LOCK}: {name} is not at {version}"));
    }
    let changelog = read(root, CHANGELOG)?;
    if parsed.is_pre_release() {
        problems.extend(pre_release_changelog_problems(&changelog, &version));
    } else {
        problems.extend(changelog_problems(&changelog, &version));
    }

    if problems.is_empty() {
        Ok(format!("Every place holds {version}."))
    } else {
        Err(problems.join("\n"))
    }
}

fn read(root: &Path, path: &str) -> Result<String, String> {
    fs::read_to_string(root.join(path)).map_err(|error| format!("{path}: {error}"))
}

fn pin_problem(pin: &Pin, found: &str, version: &str, parsed: &Version) -> Option<String> {
    if !(pin.stable_only && parsed.is_pre_release()) {
        return (found != version).then(|| format!("has {found}, the workspace has {version}"));
    }
    match Version::parse(found) {
        Ok(stable) if !stable.is_pre_release() && stable < *parsed => None,
        _ => Some(format!(
            "has {found}, but it holds the last release before the pre-release {version}"
        )),
    }
}

/// A version like `1.2.3` or `1.2.3-beta.1`, ordered as semver orders them: a pre-release comes
/// before its release, and numeric parts of a pre-release compare as numbers.
#[derive(Debug, PartialEq, Eq)]
struct Version {
    core: [u64; 3],
    pre: Vec<Identifier>,
}

/// One dot-separated part of a pre-release. Numbers sort before text, as semver says.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Identifier {
    Number(u64),
    Text(String),
}

impl Version {
    fn parse(version: &str) -> Result<Self, String> {
        let invalid = || format!("{version} is not a version like 1.2.3 or 1.2.3-beta.1");
        let (core, pre) = match version.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (version, None),
        };
        let parts: Vec<&str> = core.split('.').collect();
        let [major, minor, patch] = parts.as_slice() else {
            return Err(invalid());
        };
        let core = [major, minor, patch].map(|part| number(part));
        let [Some(major), Some(minor), Some(patch)] = core else {
            return Err(invalid());
        };
        let pre = match pre {
            None => Vec::new(),
            Some(pre) => pre
                .split('.')
                .map(|part| identifier(part).ok_or_else(invalid))
                .collect::<Result<_, _>>()?,
        };
        Ok(Self {
            core: [major, minor, patch],
            pre,
        })
    }

    fn is_pre_release(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn number(part: &str) -> Option<u64> {
    let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits || (part.len() > 1 && part.starts_with('0')) {
        return None;
    }
    part.parse().ok()
}

fn identifier(part: &str) -> Option<Identifier> {
    let allowed = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'-';
    if part.is_empty() || !part.bytes().all(allowed) {
        return None;
    }
    if part.bytes().all(|byte| byte.is_ascii_digit()) {
        return number(part).map(Identifier::Number);
    }
    Some(Identifier::Text(part.to_string()))
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

/// The byte range of the notes under `## [Unreleased]`, refused when there are none.
fn unreleased_notes(text: &str) -> Result<Range<usize>, String> {
    let start = text
        .find(UNRELEASED)
        .map(|index| index + UNRELEASED.len())
        .ok_or_else(|| format!("{CHANGELOG} has no [Unreleased] section"))?;
    let end = text[start..]
        .find("\n## [")
        .map(|index| start + index + 1)
        .ok_or_else(|| format!("{CHANGELOG} has no section after [Unreleased]"))?;
    if text[start..end].trim().is_empty() {
        return Err(format!(
            "{CHANGELOG} has nothing under [Unreleased], so no release notes"
        ));
    }
    Ok(start..end)
}

/// The release the `[Unreleased]` link compares from: the last release, which a pre-release
/// leaves in place.
fn unreleased_since(text: &str) -> Option<&str> {
    let prefix = format!("[Unreleased]: {COMPARE}v");
    text.lines()
        .find_map(|line| line.strip_prefix(&prefix)?.strip_suffix("...HEAD"))
}

fn promote_changelog(text: &str, new: &str, date: &str) -> Result<String, String> {
    if text.contains(&format!("## [{new}]")) {
        return Err(format!("{CHANGELOG} already has a section for {new}"));
    }
    let notes = unreleased_notes(text)?;
    let old = unreleased_since(text)
        .ok_or_else(|| format!("{CHANGELOG} has no line [Unreleased]: {COMPARE}v<x.y.z>...HEAD"))?;
    let old_link = format!("[Unreleased]: {COMPARE}v{old}...HEAD");
    let new_links =
        format!("[Unreleased]: {COMPARE}v{new}...HEAD\n[{new}]: {COMPARE}v{old}...v{new}");
    let (before, body, after) = (
        &text[..notes.start],
        &text[notes.clone()],
        &text[notes.end..],
    );
    Ok(format!("{before}\n## [{new}] - {date}\n{body}{after}").replace(&old_link, &new_links))
}

fn pre_release_changelog_problems(text: &str, version: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if let Err(problem) = unreleased_notes(text) {
        problems.push(problem);
    }
    if text.contains(&format!("## [{version}]")) {
        problems.push(format!(
            "{CHANGELOG}: has a section for the pre-release {version}; its notes stay under \
             [Unreleased]"
        ));
    }
    if unreleased_since(text).is_none() {
        problems.push(format!(
            "{CHANGELOG}: no line [Unreleased]: {COMPARE}v<x.y.z>...HEAD"
        ));
    }
    problems
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
        let rewritten = rewrite(pin_for("docs/install.md"), text, "0.4.0").unwrap();
        assert_eq!(rewritten, "version=0.4.0\r\nallotd 0.3.0 (pid 1)\r\n");
    }

    #[test]
    fn test_rewrite_no_matching_line_is_an_error() {
        assert!(rewrite(pin_for("docs/install.md"), "nothing here\n", "0.4.0").is_err());
    }

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn test_parse_malformed_versions_are_refused() {
        assert_eq!(version("0.10.2").core, [0, 10, 2]);
        assert!(!version("0.10.2").is_pre_release());
        for bad in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "v1.2.3",
            "01.2.3",
            "1.2.+3",
            "1..3",
            "1.2.3-",
            "1.2.3-beta..1",
            "1.2.3-beta.01",
            "1.2.3-beta_1",
            "1.2.3+build",
            "1.2.3-beta+build",
        ] {
            assert!(Version::parse(bad).is_err(), "{bad} should not parse");
        }
    }

    #[test]
    fn test_parse_pre_release_keeps_its_parts() {
        let beta = version("0.6.0-beta.1");
        assert_eq!(beta.core, [0, 6, 0]);
        assert_eq!(
            beta.pre,
            [Identifier::Text("beta".to_string()), Identifier::Number(1)]
        );
        assert!(beta.is_pre_release());
    }

    #[test]
    fn test_parse_compares_numerically() {
        assert!(version("0.10.0") > version("0.9.9"));
    }

    #[test]
    fn test_parse_orders_pre_releases_as_semver() {
        let ordered = [
            "0.5.0",
            "0.6.0-alpha",
            "0.6.0-alpha.1",
            "0.6.0-beta",
            "0.6.0-beta.2",
            "0.6.0-beta.10",
            "0.6.0-rc.1",
            "0.6.0",
        ];
        for pair in ordered.windows(2) {
            assert!(version(pair[0]) < version(pair[1]), "{pair:?}");
        }
    }

    #[test]
    fn test_pin_problem_stable_only_pin_keeps_the_last_release() {
        let snippet = pin_for("docs/install.md");
        let beta = version("0.6.0-beta.1");
        assert_eq!(pin_problem(snippet, "0.5.0", "0.6.0-beta.1", &beta), None);
        for wrong in ["0.6.0-beta.1", "0.6.0", "0.7.0"] {
            assert!(
                pin_problem(snippet, wrong, "0.6.0-beta.1", &beta).is_some(),
                "{wrong}"
            );
        }
        let release = version("0.6.0");
        assert!(pin_problem(snippet, "0.5.0", "0.6.0", &release).is_some());
        assert_eq!(pin_problem(snippet, "0.6.0", "0.6.0", &release), None);
    }

    #[test]
    fn test_pin_problem_other_pins_follow_a_pre_release() {
        let tray = pin_for("tray/src/Allot.Tray/Allot.Tray.csproj");
        let beta = version("0.6.0-beta.1");
        assert_eq!(
            pin_problem(tray, "0.6.0-beta.1", "0.6.0-beta.1", &beta),
            None
        );
        assert!(pin_problem(tray, "0.5.0", "0.6.0-beta.1", &beta).is_some());
    }

    #[test]
    fn test_pre_release_changelog_problems_unreleased_notes_pass() {
        assert!(pre_release_changelog_problems(CHANGELOG_TEXT, "0.4.0-beta.1").is_empty());
    }

    #[test]
    fn test_pre_release_changelog_problems_empty_or_promoted_is_reported() {
        let empty = CHANGELOG_TEXT.replace("### Added\n\n- A thing.\n\n", "");
        assert_eq!(
            pre_release_changelog_problems(&empty, "0.4.0-beta.1").len(),
            1
        );
        let promoted = promote_changelog(CHANGELOG_TEXT, "0.4.0-beta.1", "2026-12-01").unwrap();
        assert!(!pre_release_changelog_problems(&promoted, "0.4.0-beta.1").is_empty());
    }

    #[test]
    fn test_promote_changelog_moves_notes_and_rewrites_links() {
        let promoted = promote_changelog(CHANGELOG_TEXT, "0.4.0", "2026-12-01").unwrap();
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
        let error = promote_changelog(&text, "0.4.0", "2026-12-01").unwrap_err();
        assert!(error.contains("nothing under [Unreleased]"), "{error}");
    }

    #[test]
    fn test_promote_changelog_existing_section_is_refused() {
        assert!(promote_changelog(CHANGELOG_TEXT, "0.3.0", "2026-12-01").is_err());
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
