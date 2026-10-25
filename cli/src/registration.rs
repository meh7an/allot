//! The parts of the setup that live in Claude Code's and Windows' own configuration: the MCP
//! server, the hooks, the CLAUDE.md rule, and the logon task. `allot install` sets them up and
//! `allot uninstall` removes them again. A file is only written when something in it changes,
//! after a copy of the old one is kept beside it.

mod claude_json;
mod claude_md;
mod json;
mod mcp;
mod settings;
mod task;

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use allot_shared::paths::Paths;
use anyhow::Context;
use serde_json::Value;

use self::claude_md::Added;
use crate::legacy;

/// The name the MCP server is registered under.
const MCP_NAME: &str = "allot";
/// The name of the scheduled task that starts allotd at logon.
const TASK_NAME: &str = "allot";

/// A hook entry `settings.json` needs: the event and the `allot` arguments it runs.
type HookSpec = (&'static str, [&'static str; 2]);

/// Every hook entry: the lease check before a call, and the notices after one.
const HOOKS: [HookSpec; 3] = [
    ("PreToolUse", ["hook", "pre-tool"]),
    ("PostToolUse", ["hook", "post-tool"]),
    ("PostToolUseFailure", ["hook", "post-tool"]),
];

/// Whether a step is done, as far as allot can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Done,
    Missing,
    /// The file or command that would tell could not be read.
    Unknown,
}

impl State {
    fn label(self) -> &'static str {
        match self {
            Self::Done => "[done]",
            Self::Missing => "[todo]",
            Self::Unknown => "[check]",
        }
    }
}

/// One line of the setup report, with what to do by hand when it is not done.
struct Step {
    state: State,
    title: String,
    /// What this run changed, or why it could not.
    note: Option<String>,
    manual: Vec<String>,
}

impl Step {
    fn done(title: String) -> Self {
        Self {
            state: State::Done,
            title,
            note: None,
            manual: Vec::new(),
        }
    }

    fn changed(title: String, note: String) -> Self {
        Self {
            note: Some(note),
            ..Self::done(title)
        }
    }

    fn manual(state: State, title: String, note: Option<String>, manual: Vec<String>) -> Self {
        Self {
            state,
            title,
            note,
            manual,
        }
    }

    fn print(&self, number: usize) {
        println!();
        let note = self
            .note
            .as_ref()
            .map(|note| format!(" ({note})"))
            .unwrap_or_default();
        if self.state == State::Done {
            println!("{number}. {} {}{note}", self.state.label(), self.title);
            return;
        }
        println!("{number}. {} {}{note}:", self.state.label(), self.title);
        for line in &self.manual {
            println!("   {line}");
        }
    }
}

/// The per-user `resource-guard` skill directory Claude Code loads skills from.
pub fn skill_dir(paths: &Paths) -> PathBuf {
    claude_dir(paths).join("skills").join("resource-guard")
}

fn claude_dir(paths: &Paths) -> PathBuf {
    paths.home().join(".claude")
}

fn claude_json(paths: &Paths) -> PathBuf {
    paths.home().join(".claude.json")
}

fn settings_json(paths: &Paths) -> PathBuf {
    claude_dir(paths).join("settings.json")
}

fn claude_md(paths: &Paths) -> PathBuf {
    claude_dir(paths).join("CLAUDE.md")
}

fn rule_file(paths: &Paths) -> PathBuf {
    claude_dir(paths).join(claude_md::FILE_NAME)
}

fn allot_exe(paths: &Paths) -> PathBuf {
    paths.bin_dir().join("allot.exe")
}

fn allot_mcp_exe(paths: &Paths) -> PathBuf {
    paths.bin_dir().join("allot-mcp.exe")
}

fn allotd_exe(paths: &Paths) -> PathBuf {
    paths.bin_dir().join("allotd.exe")
}

/// What `allot install` sets up besides its own files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    /// The MCP server, the hooks, and the CLAUDE.md rule.
    pub claude: bool,
    /// The scheduled task that starts allotd at logon.
    pub logon_task: bool,
}

/// Registers the MCP server, adds the hooks and the CLAUDE.md import, and registers the logon
/// task where missing, as `setup` asks, then prints every step with what is still left to do
/// by hand.
pub fn register(paths: &Paths, setup: Setup) {
    let mut steps = Vec::new();
    if setup.claude {
        steps.extend([register_mcp(paths), add_hooks(paths), import_rule(paths)]);
    }
    let claude_changed = steps
        .iter()
        .any(|step| step.state == State::Done && step.note.is_some());
    if setup.logon_task {
        steps.push(register_task(paths));
    }
    if steps.is_empty() {
        return;
    }
    println!();
    println!("Setup; [todo] and [check] steps are yours to run:");
    for (index, step) in steps.iter().enumerate() {
        step.print(index + 1);
    }
    if claude_changed {
        println!();
        println!(
            "Open Claude Code sessions load the MCP server and CLAUDE.md when they start; \
             restart them. The hooks apply at once."
        );
    }
}

fn register_mcp(paths: &Paths) -> Step {
    let exe = allot_mcp_exe(paths);
    let title = "MCP server, user scope".to_owned();
    let config = read_json(&claude_json(paths));
    if let Some(Some(config)) = &config
        && mcp_registered(config, &exe)
    {
        return Step::done(title);
    }
    // An entry of this name that runs another exe, such as a development build.
    let replace =
        matches!(&config, Some(Some(config)) if !config["mcpServers"][MCP_NAME].is_null());
    match mcp::add(&exe, replace) {
        Ok(true) => Step::changed(title, "registered with claude mcp add".to_owned()),
        Ok(false) => write_mcp_entry(paths, title),
        Err(error) => {
            let mut manual = Vec::new();
            if replace {
                manual.push(format!("claude mcp remove --scope user {MCP_NAME}"));
            }
            manual.push(format!(
                "claude mcp add --scope user {MCP_NAME} -- \"{}\"",
                exe.display()
            ));
            manual.push("Sessions load it when they start; restart open sessions.".to_owned());
            Step::manual(State::Missing, title, Some(format!("{error:#}")), manual)
        }
    }
}

/// Without the `claude` command, the entry goes into `~/.claude.json` directly.
fn write_mcp_entry(paths: &Paths, title: String) -> Step {
    let path = claude_json(paths);
    let exe = allot_mcp_exe(paths);
    let written = read_text(&path).and_then(|text| {
        let new = claude_json::set_server(text.as_deref().unwrap_or_default(), &exe)?;
        rewrite(&path, text.as_deref(), &new)
    });
    match written {
        Ok(backup) => Step::changed(
            title,
            format!(
                "claude was not found, so it was written to {}{}",
                path.display(),
                backup_note(backup)
            ),
        ),
        Err(error) => Step::manual(
            State::Missing,
            title,
            Some(format!("{error:#}")),
            vec![
                format!(
                    "claude mcp add --scope user {MCP_NAME} -- \"{}\"",
                    exe.display()
                ),
                format!(
                    "The claude command was not found; set {} to its path, or run this where \
                     it is on PATH.",
                    mcp::CLAUDE_VAR
                ),
            ],
        ),
    }
}

fn add_hooks(paths: &Paths) -> Step {
    let path = settings_json(paths);
    let title = format!("Hooks in {}", path.display());
    let exe = allot_exe(paths);
    let unreadable = |error: anyhow::Error| {
        Step::manual(
            State::Unknown,
            title.clone(),
            Some(format!("{error:#}")),
            manual_hooks(paths, &HOOKS),
        )
    };
    let text = match read_text(&path) {
        Ok(text) => text,
        Err(error) => return unreadable(error),
    };
    let missing = match missing_hooks(text.as_deref(), &exe) {
        Ok(missing) => missing,
        Err(error) => return unreadable(error),
    };
    if missing.is_empty() {
        return Step::done(title);
    }
    let old = text.as_deref().unwrap_or_default();
    match settings::add_hooks(old, &missing, &hook_command(paths))
        .and_then(|new| rewrite(&path, text.as_deref(), &new))
    {
        Ok(backup) => {
            let events = missing
                .iter()
                .map(|(event, _)| *event)
                .collect::<Vec<_>>()
                .join(", ");
            Step::changed(title, format!("added {events}{}", backup_note(backup)))
        }
        Err(error) => Step::manual(
            State::Missing,
            title.clone(),
            Some(format!("{error:#}")),
            manual_hooks(paths, &missing),
        ),
    }
}

/// How to add the `missing` hook entries by hand.
fn manual_hooks(paths: &Paths, missing: &[HookSpec]) -> Vec<String> {
    let mut lines =
        vec!["Add each entry to the array of its event under \"hooks\" (they load live):".into()];
    for (event, args) in missing {
        lines.push(format!("\"{event}\":"));
        lines.extend(
            settings::hook_entry_text(&hook_command(paths), *args)
                .lines()
                .map(|line| format!("  {line}")),
        );
    }
    lines
}

fn import_rule(paths: &Paths) -> Step {
    let rule = rule_file(paths);
    let path = claude_md(paths);
    let title = format!("CLAUDE.md rule in {}", rule.display());
    let failed = |error: anyhow::Error| {
        Step::manual(
            State::Missing,
            title.clone(),
            Some(format!("{error:#}")),
            vec![format!(
                "Write the allot rule to {} and add the line @{} to {} between \
                 sessions.",
                rule.display(),
                claude_md::FILE_NAME,
                path.display()
            )],
        )
    };
    // Rewritten every time: an upgrade may change the rule.
    let rule_changed = match write_if_changed(&rule, claude_md::RULE) {
        Ok(changed) => changed,
        Err(error) => return failed(error),
    };
    let text = match read_text(&path) {
        Ok(text) => text,
        Err(error) => return failed(error),
    };
    let old = text.as_deref().unwrap_or_default();
    let mut notes = Vec::new();
    let imported = match claude_md::add_import(old) {
        None => old.to_owned(),
        Some((new, added)) => {
            let backup = match rewrite(&path, text.as_deref(), &new) {
                Ok(backup) => backup,
                Err(error) => return failed(error),
            };
            notes.push(match added {
                Added::ReplacedCopy => format!(
                    "replaced the pasted rule in CLAUDE.md with an import{}",
                    backup_note(backup)
                ),
                Added::ReplacedOldImport => format!(
                    "replaced the Resources Helper import in CLAUDE.md{}",
                    backup_note(backup)
                ),
                Added::Appended => format!("added the import to CLAUDE.md{}", backup_note(backup)),
            });
            new
        }
    };
    if notes.is_empty() && rule_changed {
        notes.push("updated the rule".to_owned());
    }
    if claude_md::has_other_copy(&imported) {
        notes.push(format!(
            "{} also has its own copy of the rule; remove it",
            path.display()
        ));
    }
    if notes.is_empty() {
        Step::done(title)
    } else {
        Step::changed(title, notes.join("; "))
    }
}

/// Registers the logon task unless it already starts this install's allotd; one of the same name
/// that starts another allotd, such as a development build, is replaced.
fn register_task(paths: &Paths) -> Step {
    let title = format!("allotd at logon, scheduled task \"{TASK_NAME}\"");
    let allotd = allotd_exe(paths);
    let found = task::find(TASK_NAME);
    if found.starts(&allotd) {
        return Step::done(title);
    }
    let note = match &found {
        task::Found::Runs(other) => format!("registered; it started {other} before"),
        _ => "registered".to_owned(),
    };
    match task::register(&allotd, paths.root()) {
        Ok(()) => Step::changed(title, note),
        Err(error) => {
            let mut manual = vec!["In PowerShell (no admin needed):".to_owned()];
            manual.extend(
                register_task_script(paths)
                    .lines()
                    .map(|line| format!("  {line}")),
            );
            Step::manual(State::Missing, title, Some(format!("{error:#}")), manual)
        }
    }
}

/// Removes the MCP server, the hooks, and the CLAUDE.md import, prints what it removed, and
/// returns what is left to undo by hand, for [`print_manual_steps`].
pub fn unregister(paths: &Paths) -> Vec<String> {
    let mut done = Vec::new();
    let mut manual = Vec::new();

    match read_json(&claude_json(paths)) {
        Some(Some(config)) if config["mcpServers"][MCP_NAME].is_null() => {}
        Some(None) => {}
        _ => match mcp::remove(MCP_NAME) {
            Ok(true) => done.push("removed the MCP server".to_owned()),
            Ok(false) => {
                let path = claude_json(paths);
                let removed = read_text(&path).and_then(|text| {
                    let text = text.unwrap_or_default();
                    match claude_json::remove_server(&text, MCP_NAME)? {
                        Some(new) => rewrite(&path, Some(&text), &new).map(|_| true),
                        None => Ok(false),
                    }
                });
                match removed {
                    Ok(true) => {
                        done.push(format!("removed the MCP server from {}", path.display()))
                    }
                    Ok(false) => {}
                    Err(error) => manual.push(format!(
                        "claude mcp remove --scope user {MCP_NAME} ({error:#})"
                    )),
                }
            }
            Err(error) => manual.push(format!(
                "claude mcp remove --scope user {MCP_NAME} ({error:#})"
            )),
        },
    }

    let path = settings_json(paths);
    let removed = read_text(&path).and_then(|text| {
        let Some(text) = text else {
            return Ok(false);
        };
        let Some(new) = settings::remove_hooks(&text, &allot_exe(paths))? else {
            return Ok(false);
        };
        rewrite(&path, Some(&text), &new).map(|_| true)
    });
    match removed {
        Ok(true) => done.push(format!("removed the hooks from {}", path.display())),
        Ok(false) => {}
        Err(error) => manual.push(format!(
            "Remove the hook entries that run {} from {} ({error:#})",
            hook_command(paths),
            path.display()
        )),
    }

    let path = claude_md(paths);
    let removed = read_text(&path).and_then(|text| {
        let text = text.unwrap_or_default();
        match claude_md::remove_import(&text) {
            Some(new) => rewrite(&path, Some(&text), &new).map(|_| (true, new)),
            None => Ok((false, text)),
        }
    });
    match removed {
        Ok((removed, text)) => {
            if removed {
                done.push(format!("removed the rule import from {}", path.display()));
            }
            if claude_md::has_pasted_rule(&text) {
                manual.push(format!(
                    "Remove the allot rule (the \"Shared Machine Resources\" \
                     section) from {}",
                    path.display()
                ));
            }
        }
        Err(error) => manual.push(format!(
            "Remove the line @{} from {} ({error:#})",
            claude_md::FILE_NAME,
            path.display()
        )),
    }
    let rule = rule_file(paths);
    if let Err(error) = fs::remove_file(&rule)
        && error.kind() != io::ErrorKind::NotFound
    {
        manual.push(format!("Delete {} ({error})", rule.display()));
    }

    // Only the task that starts this install's allotd; one for another allotd is not ours to remove.
    if task::find(TASK_NAME).starts(&allotd_exe(paths)) {
        delete_task(TASK_NAME, &mut done, &mut manual);
    }

    for line in &done {
        println!("{line}");
    }
    manual
}

/// Removes what Claude Code and Task Scheduler have of the install from before the rename,
/// where it runs the old binaries: the MCP server, the hooks, the CLAUDE.md import and its
/// rule file, and the logon task. Prints what it removed and returns what is left to undo by
/// hand, for [`print_manual_steps`].
pub fn unregister_legacy(paths: &Paths) -> Vec<String> {
    let mut done = Vec::new();
    let mut manual = Vec::new();
    let bin = legacy::bin_dir(paths);

    let path = claude_json(paths);
    let old_mcp = bin.join(legacy::MCP_EXE);
    if let Some(Some(config)) = read_json(&path)
        && config["mcpServers"][legacy::MCP_NAME]["command"]
            .as_str()
            .is_some_and(|command| same_path(command, &old_mcp))
    {
        let removed = match mcp::remove(legacy::MCP_NAME) {
            Ok(true) => Ok(true),
            Ok(false) => read_text(&path).and_then(|text| {
                let text = text.unwrap_or_default();
                match claude_json::remove_server(&text, legacy::MCP_NAME)? {
                    Some(new) => rewrite(&path, Some(&text), &new).map(|_| true),
                    None => Ok(false),
                }
            }),
            Err(error) => Err(error),
        };
        match removed {
            Ok(true) => done.push(format!("removed the MCP server \"{}\"", legacy::MCP_NAME)),
            Ok(false) => {}
            Err(error) => manual.push(format!(
                "claude mcp remove --scope user {} ({error:#})",
                legacy::MCP_NAME
            )),
        }
    }

    let path = settings_json(paths);
    let old_cli = bin.join(legacy::CLI_EXE);
    let removed = read_text(&path).and_then(|text| {
        let Some(text) = text else {
            return Ok(false);
        };
        let Some(new) = settings::remove_hooks(&text, &old_cli)? else {
            return Ok(false);
        };
        rewrite(&path, Some(&text), &new).map(|_| true)
    });
    match removed {
        Ok(true) => done.push(format!(
            "removed the hooks that ran {} from {}",
            old_cli.display(),
            path.display()
        )),
        Ok(false) => {}
        Err(error) => manual.push(format!(
            "Remove the hook entries that run {} from {} ({error:#})",
            old_cli.display(),
            path.display()
        )),
    }

    let path = claude_md(paths);
    let removed = read_text(&path).and_then(|text| {
        let Some(text) = text else {
            return Ok(false);
        };
        match claude_md::remove_old_import(&text) {
            Some(new) => rewrite(&path, Some(&text), &new).map(|_| true),
            None => Ok(false),
        }
    });
    match removed {
        Ok(true) => done.push(format!(
            "removed the line @{} from {}",
            legacy::RULE_FILE,
            path.display()
        )),
        Ok(false) => {}
        Err(error) => manual.push(format!(
            "Remove the line @{} from {} ({error:#})",
            legacy::RULE_FILE,
            path.display()
        )),
    }
    let rule = claude_dir(paths).join(legacy::RULE_FILE);
    match fs::remove_file(&rule) {
        Ok(()) => done.push(format!("removed {}", rule.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => manual.push(format!("Delete {} ({error})", rule.display())),
    }

    if task::find(legacy::TASK_NAME).starts(&bin.join(legacy::DAEMON_EXE)) {
        delete_task(legacy::TASK_NAME, &mut done, &mut manual);
    }

    for line in &done {
        println!("{line}");
    }
    manual
}

fn delete_task(name: &str, done: &mut Vec<String>, manual: &mut Vec<String>) {
    match task::delete(name) {
        Ok(()) => done.push(format!("removed the scheduled task \"{name}\"")),
        Err(error) => manual.push(format!(
            "In PowerShell: Unregister-ScheduledTask -TaskName '{name}' -Confirm:$false \
             ({error:#})"
        )),
    }
}

/// Prints the steps [`unregister`] left to do by hand, if any.
pub fn print_manual_steps(manual: &[String]) {
    if manual.is_empty() {
        return;
    }
    println!();
    println!("Remaining steps. Run them yourself:");
    for (number, step) in manual.iter().enumerate() {
        println!("{}. {step}", number + 1);
    }
}

/// The hook entries `settings` (the file's text, `None` when it does not exist) lacks, in
/// [`HOOKS`] order.
fn missing_hooks(settings: Option<&str>, exe: &Path) -> anyhow::Result<Vec<HookSpec>> {
    let settings: Value = match settings {
        Some(text) if !text.trim().is_empty() => {
            serde_json::from_str(text).context("the file is not valid JSON")?
        }
        _ => Value::Null,
    };
    Ok(HOOKS
        .into_iter()
        .filter(|&(event, args)| !hook_registered(&settings, event, args, exe))
        .collect())
}

/// The file's text, `None` when it does not exist.
fn read_text(path: &Path) -> anyhow::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// The parsed file, `Ok(None)` when it does not exist, and `None` when it cannot be read or
/// parsed.
fn read_json(path: &Path) -> Option<Option<Value>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).ok().map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(None),
        Err(_) => None,
    }
}

/// Replaces the file at `path`, whose text was `old` (`None` when it did not exist), with `new`.
/// The old file is kept as `<name>.allot-backup`, whose path is returned. The new text goes to a
/// temporary file first, so a failed write leaves the old file whole.
fn rewrite(path: &Path, old: Option<&str>, new: &str) -> anyhow::Result<Option<PathBuf>> {
    let backup = match old {
        Some(_) => {
            let backup = sibling(path, "allot-backup");
            fs::copy(path, &backup).with_context(|| format!("backing up {}", path.display()))?;
            Some(backup)
        }
        None => None,
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let temporary = sibling(path, "allot-new");
    fs::write(&temporary, new).with_context(|| format!("writing {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(backup)
}

/// Writes `contents` to `path` unless it already holds them; returns whether it wrote.
fn write_if_changed(path: &Path, contents: &str) -> anyhow::Result<bool> {
    if read_text(path)?.as_deref() == Some(contents) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

/// `<name>.<extension>` beside `path`.
fn sibling(path: &Path, extension: &str) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{name}.{extension}"))
}

fn backup_note(backup: Option<PathBuf>) -> String {
    backup
        .map(|backup| format!("; the old file is {}", backup.display()))
        .unwrap_or_default()
}

/// Whether `~/.claude.json` registers the MCP server, at user scope, to run `exe`.
fn mcp_registered(config: &Value, exe: &Path) -> bool {
    config["mcpServers"][MCP_NAME]["command"]
        .as_str()
        .is_some_and(|command| same_path(command, exe))
}

/// Whether `settings.json` has an `event` hook that runs `exe` with `args`.
fn hook_registered(settings: &Value, event: &str, args: [&str; 2], exe: &Path) -> bool {
    let Some(entries) = settings["hooks"][event].as_array() else {
        return false;
    };
    entries
        .iter()
        .filter_map(|entry| entry["hooks"].as_array())
        .flatten()
        .any(|hook| is_allot_hook(hook, exe, args))
}

/// Whether one hook of an entry runs `exe` with `args`.
fn is_allot_hook(hook: &Value, exe: &Path, args: [&str; 2]) -> bool {
    hook["command"]
        .as_str()
        .is_some_and(|command| same_path(command, exe))
        && hook["args"]
            .as_array()
            .is_some_and(|found| found.iter().map(Value::as_str).eq(args.map(Some)))
}

/// Windows paths compare without case and with either separator.
fn same_path(text: &str, path: &Path) -> bool {
    let normalize = |text: &str| text.trim_matches('"').replace('/', "\\").to_lowercase();
    normalize(text) == normalize(&path.to_string_lossy())
}

/// The hook command with forward slashes, the form Claude Code's settings examples use.
fn hook_command(paths: &Paths) -> String {
    allot_exe(paths).to_string_lossy().replace('\\', "/")
}

/// PowerShell that registers the logon task for the current user. Task Scheduler's defaults
/// would stop allotd after 72 hours and refuse to start it on battery.
fn register_task_script(paths: &Paths) -> String {
    let exe = allotd_exe(paths).to_string_lossy().replace('\'', "''");
    format!(
        "$action = New-ScheduledTaskAction -Execute '{exe}'\n\
         $trigger = New-ScheduledTaskTrigger -AtLogOn -User \"$env:USERDOMAIN\\$env:USERNAME\"\n\
         $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) \
         -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew\n\
         Register-ScheduledTask -TaskName '{TASK_NAME}' -Action $action -Trigger $trigger \
         -Settings $settings -Description 'Starts the allot daemon (allotd) at logon'"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn paths() -> Paths {
        Paths::under(r"C:\Users\someone")
    }

    #[test]
    fn test_mcp_registered_matches_path_without_case_or_separator() {
        let config = json!({
            "mcpServers": {
                "allot": {
                    "type": "stdio",
                    "command": "c:/users/SOMEONE/.allot/bin/allot-mcp.exe",
                    "args": [],
                },
            },
        });
        assert!(mcp_registered(&config, &allot_mcp_exe(&paths())));
    }

    #[test]
    fn test_mcp_registered_other_path_or_project_scope_is_missing() {
        let elsewhere = json!({
            "mcpServers": { "allot": { "command": "E:/target/debug/allot-mcp.exe" } },
        });
        let project_only = json!({
            "projects": {
                "E:/repo": {
                    "mcpServers": {
                        "allot": {
                            "command": r"C:\Users\someone\.allot\bin\allot-mcp.exe",
                        },
                    },
                },
            },
        });
        for config in [elsewhere, project_only, json!({})] {
            assert!(
                !mcp_registered(&config, &allot_mcp_exe(&paths())),
                "{config}"
            );
        }
    }

    const PRE_TOOL: [&str; 2] = ["hook", "pre-tool"];
    const POST_TOOL: [&str; 2] = ["hook", "post-tool"];

    fn entry(args: [&str; 2]) -> Value {
        serde_json::from_str(&settings::hook_entry_text(&hook_command(&paths()), args)).unwrap()
    }

    #[test]
    fn test_hook_registered_finds_the_entry_beside_other_hooks() {
        let settings = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "rtk hook" }] },
                    entry(PRE_TOOL),
                ],
            },
        });
        let exe = allot_exe(&paths());
        assert!(hook_registered(&settings, "PreToolUse", PRE_TOOL, &exe));
        assert!(!hook_registered(&settings, "PostToolUse", POST_TOOL, &exe));
    }

    #[test]
    fn test_hook_registered_needs_the_args_under_their_event() {
        let exe = allot_exe(&paths());
        let settings = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{ "command": hook_command(&paths()), "args": ["probe"] }],
                }],
                "PostToolUse": [entry(PRE_TOOL)],
            },
        });
        assert!(!hook_registered(&settings, "PreToolUse", PRE_TOOL, &exe));
        assert!(!hook_registered(&settings, "PostToolUse", POST_TOOL, &exe));
        assert!(!hook_registered(
            &json!({ "hooks": {} }),
            "PreToolUse",
            PRE_TOOL,
            &exe
        ));
    }

    #[test]
    fn test_hook_entry_uses_forward_slashes_and_exec_form() {
        let entry = entry(POST_TOOL);
        let hook = &entry["hooks"][0];
        assert_eq!(hook["command"], "C:/Users/someone/.allot/bin/allot.exe");
        assert_eq!(hook["args"], json!(["hook", "post-tool"]));
        assert_eq!(entry["matcher"], "Bash|PowerShell");
    }

    #[test]
    fn test_hooks_cover_both_post_tool_events() {
        let events: Vec<_> = HOOKS
            .iter()
            .map(|(event, args)| (*event, args[1]))
            .collect();
        assert_eq!(
            events,
            [
                ("PreToolUse", "pre-tool"),
                ("PostToolUse", "post-tool"),
                ("PostToolUseFailure", "post-tool"),
            ]
        );
    }

    #[test]
    fn test_missing_hooks_of_a_missing_file_is_every_hook() {
        let exe = allot_exe(&paths());
        assert_eq!(missing_hooks(None, &exe).unwrap(), HOOKS);
        assert_eq!(missing_hooks(Some(""), &exe).unwrap(), HOOKS);
        assert!(missing_hooks(Some("{"), &exe).is_err());
    }

    #[test]
    fn test_register_task_script_disables_the_time_limit_and_quotes_the_path() {
        let script = register_task_script(&Paths::under(r"C:\Users\o'brien"));
        assert!(script.contains(r"-Execute 'C:\Users\o''brien\.allot\bin\allotd.exe'"));
        assert!(script.contains("-ExecutionTimeLimit ([TimeSpan]::Zero)"));
        assert!(script.contains("-AtLogOn -User \"$env:USERDOMAIN\\$env:USERNAME\""));
        assert_eq!(script.lines().count(), 4);
    }
}
