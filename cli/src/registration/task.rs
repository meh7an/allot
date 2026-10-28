//! The scheduled task that starts allotd at logon, registered with `schtasks` from a task XML,
//! which sets what the command-line flags cannot: no time limit and running on battery. A
//! per-user logon task needs no admin.

use std::{
    env, fs,
    path::Path,
    process::{Command, Stdio},
};

use allot_shared::{
    autostart::{xml_escape, xml_unescape},
    paths::Paths,
};
use anyhow::{Context, bail};

use super::autostart::Found;

/// The name of the scheduled task that starts allotd at logon.
const TASK_NAME: &str = "allot";

/// The setup report's title for the logon task.
pub(super) fn title(_paths: &Paths) -> String {
    format!("allotd at logon, scheduled task \"{TASK_NAME}\"")
}

/// Registers the logon task unless it already starts `allotd`; one of the same name that starts
/// another allotd, such as a development build, is replaced. Returns what changed, `None` when
/// nothing did.
///
/// # Errors
///
/// Fails when `schtasks` refuses.
pub(super) fn set_up(paths: &Paths, allotd: &Path) -> anyhow::Result<Option<String>> {
    let found = find(TASK_NAME);
    if found.starts(allotd) {
        return Ok(None);
    }
    register(allotd, paths.root())?;
    Ok(Some(found.replaced_note(allotd, "registered")))
}

/// How to register the logon task by hand.
pub(super) fn manual_set_up(_paths: &Paths, allotd: &Path) -> Vec<String> {
    let mut manual = vec!["In PowerShell (no admin needed):".to_owned()];
    manual.extend(
        register_script(allotd)
            .lines()
            .map(|line| format!("  {line}")),
    );
    manual
}

/// Deletes the logon task when it starts `allotd`; one for another allotd is not ours to
/// remove. Returns what was removed.
///
/// # Errors
///
/// Fails when `schtasks` refuses.
pub(super) fn remove(_paths: &Paths, allotd: &Path) -> anyhow::Result<Option<String>> {
    if !find(TASK_NAME).starts(allotd) {
        return Ok(None);
    }
    delete(TASK_NAME)?;
    Ok(Some(format!("removed the scheduled task \"{TASK_NAME}\"")))
}

/// How to delete the logon task by hand.
pub(super) fn manual_remove(_paths: &Paths) -> String {
    manual_delete(TASK_NAME)
}

/// How to delete the task called `name` by hand.
pub(super) fn manual_delete(name: &str) -> String {
    format!("In PowerShell: Unregister-ScheduledTask -TaskName '{name}' -Confirm:$false")
}

/// What `schtasks` knows about the task called `name`.
pub(super) fn find(name: &str) -> Found {
    let output = Command::new("schtasks")
        .args(["/Query", "/TN", name, "/XML"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => {
            match command_of(&String::from_utf8_lossy(&output.stdout)) {
                Some(command) => Found::Runs(command),
                None => Found::Unknown,
            }
        }
        Ok(_) => Found::Missing,
        Err(_) => Found::Unknown,
    }
}

/// Registers the task for the current user to start `allotd`, replacing one of the same name.
/// The XML goes through a file in `scratch`, which is deleted afterwards.
///
/// # Errors
///
/// Fails when the user is unknown or `schtasks` refuses.
pub(super) fn register(allotd: &Path, scratch: &Path) -> anyhow::Result<()> {
    let user = match (env::var("USERDOMAIN"), env::var("USERNAME")) {
        (Ok(domain), Ok(name)) if !name.is_empty() => format!("{domain}\\{name}"),
        _ => bail!("USERNAME is not set"),
    };
    let file = scratch.join("logon-task.xml");
    fs::create_dir_all(scratch).with_context(|| format!("creating {}", scratch.display()))?;
    // Task Scheduler reads the encoding the declaration names: UTF-16 with a byte order mark.
    let xml = task_xml(allotd, &user);
    let bytes: Vec<u8> = [0xFEFF_u16]
        .into_iter()
        .chain(xml.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    fs::write(&file, bytes).with_context(|| format!("writing {}", file.display()))?;
    let result = schtasks(&[
        "/Create",
        "/TN",
        TASK_NAME,
        "/XML",
        &file.to_string_lossy(),
        "/F",
    ]);
    let _ = fs::remove_file(&file);
    result
}

/// Deletes the task called `name`.
///
/// # Errors
///
/// Fails when `schtasks` refuses.
pub(super) fn delete(name: &str) -> anyhow::Result<()> {
    schtasks(&["/Delete", "/TN", name, "/F"])
}

fn schtasks(args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("schtasks")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("running schtasks")?;
    if !output.status.success() {
        bail!(
            "schtasks {} failed: {}",
            args[0],
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// The task: at `user`'s logon, start `allotd` with the user's own rights, with no time limit, on
/// battery too, and not again while it runs.
fn task_xml(allotd: &Path, user: &str) -> String {
    let user = xml_escape(user);
    let allotd = xml_escape(&allotd.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.3" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts the allot daemon (allotd) at logon</Description>
  </RegistrationInfo>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <Enabled>true</Enabled>
  </Settings>
  <Triggers>
    <LogonTrigger>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Actions Context="Author">
    <Exec>
      <Command>{allotd}</Command>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// The first `<Command>` of a task's XML, unquoted.
fn command_of(xml: &str) -> Option<String> {
    let start = xml.find("<Command>")? + "<Command>".len();
    let end = start + xml[start..].find("</Command>")?;
    Some(
        xml_unescape(xml[start..end].trim())
            .trim_matches('"')
            .to_owned(),
    )
}

/// PowerShell that registers the logon task for the current user. Task Scheduler's defaults
/// would stop allotd after 72 hours and refuse to start it on battery.
fn register_script(allotd: &Path) -> String {
    let exe = allotd.to_string_lossy().replace('\'', "''");
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
    use super::*;

    #[test]
    fn test_task_xml_escapes_and_reads_back_its_command() {
        let allotd = Path::new(r"C:\Users\A&B <x>\.allot\bin\allotd.exe");
        let xml = task_xml(allotd, r"PC\A&B");

        assert!(xml.contains(r"<UserId>PC\A&amp;B</UserId>"), "{xml}");
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false"));
        assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew"));
        assert_eq!(
            command_of(&xml).as_deref(),
            Some(r"C:\Users\A&B <x>\.allot\bin\allotd.exe")
        );
    }

    #[test]
    fn test_register_script_disables_the_time_limit_and_quotes_the_path() {
        let script = register_script(Path::new(r"C:\Users\o'brien\.allot\bin\allotd.exe"));
        assert!(script.contains(r"-Execute 'C:\Users\o''brien\.allot\bin\allotd.exe'"));
        assert!(script.contains("-ExecutionTimeLimit ([TimeSpan]::Zero)"));
        assert!(script.contains("-AtLogOn -User \"$env:USERDOMAIN\\$env:USERNAME\""));
        assert_eq!(script.lines().count(), 4);
    }

    #[test]
    fn test_command_of_a_queried_task_unquotes_it() {
        let xml = "<Task>\r\r\n  <Actions>\r\r\n    <Exec>\r\r\n      \
            <Command>\"C:\\Users\\me\\.allot\\bin\\allotd.exe\"</Command>\r\r\n";
        let found = Found::Runs(command_of(xml).unwrap());
        assert!(found.starts(Path::new(r"c:\users\ME\.allot\bin\allotd.exe")));
        assert!(!found.starts(Path::new(r"C:\other\allotd.exe")));
        assert_eq!(command_of("<Task></Task>"), None);
    }
}
