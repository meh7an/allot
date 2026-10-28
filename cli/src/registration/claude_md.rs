//! The rule for the global CLAUDE.md lives in `~/.claude/allot.md`, which CLAUDE.md imports
//! with one line, the way it imports other tools' rules. An upgrade rewrites only the imported
//! file; a copy of the rule pasted by hand before, or the import of the rule from before the
//! rename to Allot, is replaced by the import.

/// The section the imported file holds.
pub(super) const RULE: &str = include_str!("../../../skill/claude-md-rule.md");
/// The imported file's name, beside CLAUDE.md.
pub(super) const FILE_NAME: &str = "allot.md";
/// The rule file CLAUDE.md imported before the rename. Only Windows had installs then, but
/// looking for its import is harmless anywhere.
pub(super) const OLD_FILE_NAME: &str = "resources-helper.md";
const IMPORT: &str = "@allot.md";
/// The words every version of the rule starts with, current and from before the rename.
const OPENINGS: [&str; 2] = [
    "coordinated by the Allot daemon",
    "coordinated by the resources-helper daemon",
];

/// The rule's text as earlier releases asked to paste it, or imported it before the rename,
/// without its heading. A pasted copy of any of these, or of the current rule, is replaced by
/// the import.
const PASTED: [&str; 3] = [
    "This PC is shared by several Claude sessions, coordinated by the resources-helper daemon.

- Before any heavy command (builds, test suites, benchmarks, training, rendering, or anything
  that keeps several cores or the GPU busy for more than ~30 s), follow the `resource-guard`
  skill: `acquire_lease`, run only while the lease is active, then `release_lease` right away.
- A tool call \"Blocked by resources-helper\" means: do what the message says, then retry the
  same command. Never reword, wrap, or script a command to get past the hook.
- Timing or performance numbers need a `benchmark_exclusive` lease; tests and builds do not.
- When a lease is queued, tell the user in one line what is ahead and the estimated wait; ask
  before waiting longer than 10 minutes or when the wait is unknown.
- Lease handling never needs the user's approval; it is not a state-changing command.",
    "This PC is shared by several Claude sessions, coordinated by the resources-helper daemon.

- Before any heavy command (builds, test suites, benchmarks, training, rendering, or anything
  that keeps several cores or the GPU busy for more than ~30 s), follow the `resource-guard`
  skill: `acquire_lease`, run only while the lease is active, then `release_lease` right away.
- A tool call \"Blocked by resources-helper\" means: do what the message says, then retry the
  same command. Never reword, wrap, or script a command to get past the hook.
- Timing or performance numbers need a `benchmark_exclusive` lease; tests and builds do not.
- When a lease is queued, tell the user in one line what is ahead and the estimated wait, then
  keep waiting; never ask whether to queue or wait. If it ends without starting (removed from
  the queue), don't run the command; say in one line that it was cancelled.
- Lease handling never needs the user's approval; it is not a state-changing command.",
    "This PC is shared by several Claude sessions, coordinated by the resources-helper daemon.

- Before any heavy command (builds, test suites, benchmarks, training, rendering, or anything
  that keeps several cores or the GPU busy for more than ~30 s), follow the `resource-guard`
  skill: `acquire_lease`, run only while the lease is active, then `release_lease` right away.
- A tool call \"Blocked by resources-helper\" means: do what the message says, then retry the
  same command. Never reword, wrap, or script a command to get past the hook.
- Timing or performance numbers need a `benchmark_exclusive` lease; tests and builds do not.
- When a lease is queued, tell the user in one line what is ahead and the estimated wait, then
  keep waiting; never ask whether to queue or wait. If it ends without starting (removed from
  the queue), don't run the command; say in one line that it was cancelled.
- When a note says the user stopped a running job from Resources Helper, say so and ask
  whether to retry or skip; never retry on your own.
- Lease handling never needs the user's approval; it is not a state-changing command.",
];

/// The current rule without its heading.
pub(super) fn rule_body() -> &'static str {
    let rule = RULE.trim();
    match rule.split_once('\n') {
        Some((first, rest)) if first.starts_with('#') => rest.trim(),
        _ => rule,
    }
}

pub(super) fn has_import(claude_md: &str) -> bool {
    claude_md.lines().any(|line| line.trim() == IMPORT)
}

/// Whether `claude_md` holds the current rule's text, pasted under any heading. Line wrapping
/// and line endings do not matter.
pub(super) fn has_pasted_rule(claude_md: &str) -> bool {
    words(claude_md).contains(&words(rule_body()))
}

/// Whether `claude_md` mentions the rule outside the import, as an edited copy would.
pub(super) fn has_other_copy(claude_md: &str) -> bool {
    let text = words(claude_md);
    OPENINGS.iter().any(|opening| text.contains(opening))
}

/// How [`add_import`] added the import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Added {
    /// In place of a pasted copy of the rule and its heading.
    ReplacedCopy,
    /// In place of the import from before the rename.
    ReplacedOldImport,
    /// At the end.
    Appended,
}

/// `claude_md` with the import line, or `None` when it has one.
pub(super) fn add_import(claude_md: &str) -> Option<(String, Added)> {
    if has_import(claude_md) {
        return None;
    }
    let mut lines: Vec<&str> = claude_md.lines().collect();
    let added = if let Some(index) = position_of(&lines, &old_import()) {
        lines[index] = IMPORT;
        Added::ReplacedOldImport
    } else if let Some((start, end)) = find_pasted(&lines) {
        lines.splice(start..end, [IMPORT]);
        Added::ReplacedCopy
    } else {
        while lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.pop();
        }
        if !lines.is_empty() {
            lines.push("");
        }
        lines.push(IMPORT);
        Added::Appended
    };
    Some((join(&lines, claude_md), added))
}

/// `claude_md` without the import line (and a blank line it leaves dangling), or `None` when it
/// has none.
pub(super) fn remove_import(claude_md: &str) -> Option<String> {
    remove_line(claude_md, IMPORT)
}

/// `claude_md` without the import from before the rename, or `None` when it has none.
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "only Windows had installs from before the rename")
)]
pub(super) fn remove_old_import(claude_md: &str) -> Option<String> {
    remove_line(claude_md, &old_import())
}

fn old_import() -> String {
    format!("@{OLD_FILE_NAME}")
}

fn position_of(lines: &[&str], import: &str) -> Option<usize> {
    lines.iter().position(|line| line.trim() == import)
}

fn remove_line(claude_md: &str, import: &str) -> Option<String> {
    let mut lines: Vec<&str> = claude_md.lines().collect();
    let index = position_of(&lines, import)?;
    lines.remove(index);
    let blank = |at: usize| lines.get(at).is_none_or(|line| line.trim().is_empty());
    if index > 0 && blank(index - 1) && blank(index) {
        lines.remove(index - 1);
    }
    Some(join(&lines, claude_md))
}

/// The lines `start..end` of a pasted copy of the rule, with the heading right above it when
/// that heading names the section.
fn find_pasted(lines: &[&str]) -> Option<(usize, usize)> {
    let bodies: Vec<&str> = PASTED.iter().copied().chain([rule_body()]).collect();
    let firsts: Vec<String> = bodies
        .iter()
        .map(|body| words(body.lines().next().unwrap_or_default()))
        .collect();
    let bodies: Vec<String> = bodies.into_iter().map(words).collect();
    let longest = bodies.iter().map(String::len).max().unwrap_or_default();
    for start in (0..lines.len()).filter(|&index| firsts.contains(&words(lines[index]))) {
        let mut span = String::new();
        for (end, line) in lines.iter().enumerate().skip(start) {
            let line = words(line);
            if line.is_empty() {
                continue;
            }
            if !span.is_empty() {
                span.push(' ');
            }
            span.push_str(&line);
            if span.len() > longest {
                break;
            }
            if bodies.contains(&span) {
                return Some((heading_above(lines, start), end + 1));
            }
        }
    }
    None
}

/// `start`, or the index of the section heading above it across blank lines.
fn heading_above(lines: &[&str], start: usize) -> usize {
    let mut above = start;
    while above > 0 && lines[above - 1].trim().is_empty() {
        above -= 1;
    }
    match above.checked_sub(1) {
        Some(heading)
            if lines[heading].trim_start().starts_with('#')
                && lines[heading].contains("Shared Machine Resources") =>
        {
            heading
        }
        _ => start,
    }
}

/// The lines joined with the original's line ending, ending with one.
fn join(lines: &[&str], original: &str) -> String {
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut text = lines.join(newline);
    text.push_str(newline);
    text
}

fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &str = "# Contract\n\n## 10 — Usage\n\n- Save usage.\n\n@RTK.md\n\n";

    #[test]
    fn test_rule_is_a_section_with_the_current_body() {
        assert!(RULE.starts_with("## Shared Machine Resources\n"), "{RULE}");
        let body = rule_body();
        assert!(
            body.starts_with("This machine is shared by several Claude sessions"),
            "{body}"
        );
        assert!(
            body.ends_with("it is not a state-changing command."),
            "{body}"
        );
        assert!(
            !PASTED.contains(&body),
            "the current rule is not a past one"
        );
    }

    #[test]
    fn test_add_import_replaces_a_pasted_older_rule_and_its_numbered_heading() {
        for pasted in PASTED.into_iter().chain([rule_body()]) {
            let claude_md = format!(
                "{CONTRACT}## 11 — Shared Machine Resources\n\n{pasted}\n\n## 12 — Next\n\nText.\n"
            );

            let (updated, added) = add_import(&claude_md).unwrap();

            assert_eq!(added, Added::ReplacedCopy);
            assert_eq!(
                updated,
                format!("{CONTRACT}@allot.md\n\n## 12 — Next\n\nText.\n")
            );
            assert!(!has_other_copy(&updated));
        }
    }

    #[test]
    fn test_add_import_finds_a_rewrapped_copy_with_crlf() {
        let rewrapped = PASTED[1].replace('\n', " ").replace(" - ", "\r\n- ");
        let claude_md =
            format!("# Rules\r\n\r\n## Shared Machine Resources\r\n\r\n{rewrapped}\r\n");

        let (updated, added) = add_import(&claude_md).unwrap();

        assert_eq!(added, Added::ReplacedCopy);
        assert_eq!(updated, "# Rules\r\n\r\n@allot.md\r\n");
    }

    #[test]
    fn test_add_import_appends_when_no_copy_matches() {
        let (updated, added) = add_import(CONTRACT).unwrap();
        assert_eq!(added, Added::Appended);
        assert_eq!(
            updated,
            "# Contract\n\n## 10 — Usage\n\n- Save usage.\n\n@RTK.md\n\n@allot.md\n"
        );
        assert_eq!(add_import(&updated), None);

        let (empty, _) = add_import("").unwrap();
        assert_eq!(empty, "@allot.md\n");
    }

    #[test]
    fn test_an_edited_copy_is_kept_and_reported() {
        let edited = PASTED[1].replace("~30 s", "~20 s");
        let claude_md = format!("## Shared Machine Resources\n\n{edited}\n");

        let (updated, added) = add_import(&claude_md).unwrap();

        assert_eq!(added, Added::Appended);
        assert!(updated.contains("~20 s"));
        assert!(has_other_copy(&updated));
    }

    #[test]
    fn test_remove_import_drops_the_line_and_its_blank() {
        let (added, _) = add_import(CONTRACT).unwrap();
        assert_eq!(
            remove_import(&added).unwrap(),
            "# Contract\n\n## 10 — Usage\n\n- Save usage.\n\n@RTK.md\n"
        );
        let between = "# A\n\n@allot.md\n\n# B\n";
        assert_eq!(remove_import(between).unwrap(), "# A\n\n# B\n");
        assert_eq!(remove_import(CONTRACT), None);
    }

    #[test]
    fn test_add_import_replaces_the_old_import_where_it_was() {
        let claude_md = "# Contract\r\n\r\n@resources-helper.md\r\n\r\n@RTK.md\r\n";

        let (updated, added) = add_import(claude_md).unwrap();

        assert_eq!(added, Added::ReplacedOldImport);
        assert_eq!(updated, "# Contract\r\n\r\n@allot.md\r\n\r\n@RTK.md\r\n");
        assert_eq!(remove_old_import(&updated), None);
    }

    #[test]
    fn test_remove_old_import_leaves_the_new_one() {
        let both = "# A\n\n@allot.md\n\n@resources-helper.md\n";
        assert_eq!(remove_old_import(both).unwrap(), "# A\n\n@allot.md\n");
    }

    #[test]
    fn test_has_pasted_rule_ignores_the_heading_and_wrapping() {
        let rewrapped = rule_body().replace('\n', " ").replace(" - ", "\r\n- ");
        assert!(has_pasted_rule(&format!(
            "## 11 — Shared\r\n\r\n{rewrapped}\r\n"
        )));
        assert!(!has_pasted_rule(PASTED[1]));
        assert!(!has_pasted_rule(""));
    }
}
