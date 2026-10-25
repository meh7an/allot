//! The tray app in `bin\tray\`: finding its build, asking the running app to quit, mirroring
//! the published folder, and starting it. The tray app owns its Launch at logon entry, so
//! install and uninstall ask the exe to set it.

use std::{
    collections::HashSet,
    fs, io,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use allot_shared::paths::Paths;
use anyhow::{Context, bail};

/// The folder of the tray app, both beside `allot.exe` in a release and under the bin directory.
const DIR: &str = "tray";
const EXE: &str = "allot-tray.exe";
/// How long the running app gets to quit before its files are renamed aside instead.
const QUIT_WAIT: Duration = Duration::from_secs(10);
/// A started tray app that exits within this time handed itself to another running copy.
const START_CHECK: Duration = Duration::from_millis(1500);
const POLL: Duration = Duration::from_millis(100);

// Win32 process creation flags, as for allotd: no console, and no Ctrl+C from the terminal.
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// `bin\tray\` of the install at `paths`.
pub fn dir(paths: &Paths) -> PathBuf {
    paths.bin_dir().join(DIR)
}

/// The tray app's exe in `dir`.
pub fn exe(dir: &Path) -> PathBuf {
    dir.join(EXE)
}

/// The tray build to install: `tray` when given (it must hold the exe), else the `tray`
/// folder beside the other binaries in `source`, or `None` when that has none.
///
/// # Errors
///
/// Fails when `tray` is given but has no `allot-tray.exe`.
pub fn find_build(source: &Path, tray: Option<&Path>) -> anyhow::Result<Option<PathBuf>> {
    if let Some(tray) = tray {
        if !tray.join(EXE).is_file() {
            bail!(
                "{} has no {EXE}; pass the tray app's publish folder",
                tray.display()
            );
        }
        return Ok(Some(tray.to_owned()));
    }
    let beside = source.join(DIR);
    Ok(beside.join(EXE).is_file().then_some(beside))
}

/// Asks the running tray app to quit through `exe`, the installed copy (the running app only
/// hears launches of its own exe path), and waits until that copy is no longer running.
/// Returns false when it still runs after [`QUIT_WAIT`].
pub fn quit(exe: &Path) -> bool {
    // An app with nothing running exits at once; a failure to run it leaves nothing to wait for.
    let _ = run(exe, &["--quit"]);
    let deadline = Instant::now() + QUIT_WAIT;
    while is_running(exe) {
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL);
    }
    true
}

/// Makes `target` an exact copy of `source`: copies every file, renaming aside the ones in use,
/// and deletes what `source` lacks. Returns the files that could not be deleted because they
/// are in use; a later install deletes them.
///
/// # Errors
///
/// Fails when a file cannot be copied or a directory cannot be created.
pub fn mirror(source: &Path, target: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let files = files_under(source)?;
    for relative in &files {
        copy_file(&source.join(relative), &target.join(relative))?;
    }
    let keep: HashSet<_> = files.iter().map(|path| lowercase(path)).collect();
    let mut in_use = Vec::new();
    for relative in files_under(target)? {
        if !keep.contains(&lowercase(&relative)) && fs::remove_file(target.join(&relative)).is_err()
        {
            in_use.push(target.join(relative));
        }
    }
    remove_empty_dirs(target);
    Ok(in_use)
}

/// Deletes `target` and everything in it, except files still in use, which are returned.
pub fn remove(target: &Path) -> Vec<PathBuf> {
    let in_use = files_under(target)
        .unwrap_or_default()
        .into_iter()
        .map(|relative| target.join(relative))
        .filter(|path| fs::remove_file(path).is_err())
        .collect();
    remove_empty_dirs(target);
    let _ = fs::remove_dir(target);
    in_use
}

/// Turns the Launch at logon of the tray app `exe` on or off.
///
/// # Errors
///
/// Fails when the exe does not run or reports a failure.
pub fn set_launch_at_logon(exe: &Path, on: bool) -> anyhow::Result<()> {
    let value = if on { "on" } else { "off" };
    let status = run(exe, &["--launch-at-logon", value])?;
    if !status.success() {
        let name = exe.file_name().unwrap_or_default().to_string_lossy();
        bail!("{name} --launch-at-logon {value} exited with {status}");
    }
    Ok(())
}

/// Starts the tray app in `target` detached and checks that it stays up.
///
/// # Errors
///
/// Fails when it cannot be started, or when it exits at once, which means another copy of the
/// tray app was already running and got the launch instead.
pub fn start(target: &Path) -> anyhow::Result<()> {
    let exe = target.join(EXE);
    let mut child = Command::new(&exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .with_context(|| format!("starting {}", exe.display()))?;
    let deadline = Instant::now() + START_CHECK;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            bail!(
                "{EXE} exited at once ({status}); another tray app may be running, such as a \
                 development build: quit it from its tray menu, then start {}",
                exe.display()
            );
        }
        thread::sleep(POLL);
    }
    Ok(())
}

/// Runs the exe with `args` and waits for it.
fn run(exe: &Path, args: &[&str]) -> anyhow::Result<std::process::ExitStatus> {
    Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("running {} {}", exe.display(), args.join(" ")))
}

/// A running exe cannot be opened for writing.
fn is_running(exe: &Path) -> bool {
    match fs::OpenOptions::new().write(true).open(exe) {
        Ok(_) => false,
        Err(error) => error.kind() != io::ErrorKind::NotFound,
    }
}

/// Copies `source` over `target`. A file in use (a loaded DLL, the running exe) cannot be
/// overwritten but can be renamed, so it is renamed aside first.
fn copy_file(source: &Path, target: &Path) -> anyhow::Result<()> {
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    if fs::copy(source, target).is_ok() {
        return Ok(());
    }
    fs::rename(target, aside_path(target))
        .with_context(|| format!("renaming {} aside", target.display()))?;
    fs::copy(source, target)
        .map(drop)
        .with_context(|| format!("copying {} to {}", source.display(), target.display()))
}

/// `<name>.old-<unix millis>` beside `target`.
fn aside_path(target: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!("{name}.old-{millis}"))
}

/// Every file under `root`, relative to it; none when `root` does not exist.
fn files_under(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![PathBuf::new()];
    while let Some(relative) = dirs.pop() {
        let entries = match fs::read_dir(root.join(&relative)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("reading {}", root.join(&relative).display()));
            }
        };
        for entry in entries {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// Removes every empty directory under `root`, deepest first; `root` itself stays.
fn remove_empty_dirs(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            remove_empty_dirs(&path);
            let _ = fs::remove_dir(&path);
        }
    }
}

/// Windows file names compare without case.
fn lowercase(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

#[cfg(test)]
mod tests {
    use std::{env, os::windows::fs::OpenOptionsExt, process};

    use super::*;

    /// A fresh directory under the system temp directory, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!("allot-tray-{}-{name}", process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn sorted_files(root: &Path) -> Vec<String> {
        let mut files: Vec<_> = files_under(root)
            .unwrap()
            .iter()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect();
        files.sort();
        files
    }

    /// Opens `path` the way the loader holds a running exe or a loaded DLL.
    fn hold_like_a_running_exe(path: &Path) -> fs::File {
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_DELETE: u32 = 0x4;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .open(path)
            .unwrap()
    }

    #[test]
    fn test_mirror_copies_new_files_and_deletes_the_ones_the_build_lacks() {
        let dir = TempDir::new("mirror");
        let source = dir.path().join("publish");
        let target = dir.path().join("tray");
        write(&source, "allot-tray.exe", "new exe");
        write(&source, "en-US/Microsoft.ui.xaml.dll.mui", "mui");
        write(&target, "allot-tray.exe", "old exe");
        write(&target, "gone.dll", "old");
        write(&target, "fr-FR/gone.mui", "old");

        let in_use = mirror(&source, &target).unwrap();

        assert!(in_use.is_empty(), "{in_use:?}");
        assert_eq!(
            sorted_files(&target),
            ["allot-tray.exe", "en-US/Microsoft.ui.xaml.dll.mui"]
        );
        assert_eq!(
            fs::read_to_string(target.join("allot-tray.exe")).unwrap(),
            "new exe"
        );
        assert!(!target.join("fr-FR").exists());
    }

    // A held file can still be deleted, unlike a running image, so the files left in use are
    // covered by the install tests, which run a real exe.
    #[test]
    fn test_mirror_replaces_a_file_that_cannot_be_overwritten() {
        let dir = TempDir::new("in-use");
        let source = dir.path().join("publish");
        let target = dir.path().join("tray");
        write(&source, "WinUI.dll", "new");
        write(&target, "WinUI.dll", "old");
        let held = hold_like_a_running_exe(&target.join("WinUI.dll"));
        assert!(fs::copy(source.join("WinUI.dll"), target.join("WinUI.dll")).is_err());

        mirror(&source, &target).unwrap();

        assert_eq!(fs::read_to_string(target.join("WinUI.dll")).unwrap(), "new");
        drop(held);
        mirror(&source, &target).unwrap();
        assert_eq!(sorted_files(&target), ["WinUI.dll"]);
    }

    #[test]
    fn test_remove_deletes_the_folder() {
        let dir = TempDir::new("remove");
        let target = dir.path().join("tray");
        write(&target, "allot-tray.exe", "exe");
        write(&target, "de-DE/a.mui", "mui");

        assert!(remove(&target).is_empty());
        assert!(!target.exists());
    }

    #[test]
    fn test_find_build_prefers_the_given_folder_and_needs_the_exe() {
        let dir = TempDir::new("find");
        let source = dir.path().join("release");
        assert_eq!(find_build(&source, None).unwrap(), None);

        write(&source, "tray/allot-tray.exe", "exe");
        assert_eq!(
            find_build(&source, None).unwrap(),
            Some(source.join("tray"))
        );

        let publish = dir.path().join("publish");
        fs::create_dir_all(&publish).unwrap();
        let error = find_build(&source, Some(&publish)).unwrap_err();
        assert!(
            error.to_string().contains("has no allot-tray.exe"),
            "{error}"
        );
        write(&publish, "allot-tray.exe", "exe");
        assert_eq!(find_build(&source, Some(&publish)).unwrap(), Some(publish));
    }

    #[test]
    fn test_is_running_only_while_the_exe_is_held() {
        let dir = TempDir::new("running");
        let exe = dir.path().join("allot-tray.exe");
        assert!(!is_running(&exe), "a missing exe is not running");
        fs::write(&exe, "exe").unwrap();
        assert!(!is_running(&exe));
        let held = hold_like_a_running_exe(&exe);
        assert!(is_running(&exe));
        drop(held);
        assert!(!is_running(&exe));
    }
}
