//! `allot install` and `allot uninstall`: the binaries in `%USERPROFILE%\.allot\bin`,
//! the tray app in `bin\tray`, the skill copy, restarting allotd on the new build, and Claude
//! Code's registrations.

use std::{
    env,
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use allot_client::{BlockingClient, ClientOptions, SpawnOptions};
use allot_shared::{
    config::Config,
    paths::Paths,
    platform::{ALLOT_BINARIES, DAEMON_EXE},
};
use anyhow::{Context, bail};

use crate::{daemon, legacy, registration, tray};

/// The executables of an install.
const BINARIES: [&str; 3] = ALLOT_BINARIES;
const SKILL: &str = include_str!("../../skill/resource-guard/SKILL.md");
/// The rule copy older installs left for pasting; the rule is imported into CLAUDE.md now.
const OLD_RULE_FILE: &str = "claude-md-rule.md";
const SKILL_FILE: &str = "SKILL.md";
/// How long to wait for a stopped allotd to exit. Older builds finish open acquire waits first,
/// which can take a minute.
const EXIT_WAIT: Duration = Duration::from_secs(90);
const EXIT_POLL: Duration = Duration::from_millis(100);

/// What `allot install` installs and sets up.
#[derive(Debug, Clone, Copy)]
pub struct Options<'a> {
    /// The build output (default: this executable's directory).
    pub from: Option<&'a Path>,
    /// The tray app's publish folder (default: the `tray` folder in `from`, when it has one).
    pub tray: Option<&'a Path>,
    /// The tray app's Launch at logon; `None` turns it on only when the tray app is installed
    /// for the first time, so a later install keeps what the user chose.
    pub launch_at_logon: Option<bool>,
    /// The Claude Code integration (with the skill) and the logon task.
    pub setup: registration::Setup,
}

/// Takes over an install from before the rename, copies the built binaries into the bin
/// directory, copies the skill, restarts allotd on the new build when it was running, installs
/// and starts the tray app, and registers the rest with Claude Code and Task Scheduler, as
/// `options` ask. A failed restart or tray start is reported, not returned: the install itself
/// is complete by then.
///
/// # Errors
///
/// Fails when the source is the bin directory itself or lacks a binary, when the given tray
/// folder has no tray app, when the old daemon does not stop, or when a file cannot be
/// written.
pub fn install(options: Options) -> anyhow::Result<()> {
    // allotd and the tray app outlive this run; the setup's log must not stay open in them.
    allot_sampler::keep_std_handles_private();
    let paths = Paths::from_env()?;
    let source = match options.from {
        Some(dir) => dir.to_owned(),
        None => current_exe_dir()?,
    };
    let bin = paths.bin_dir();
    if same_dir(&source, &bin) {
        bail!(
            "{} is the install itself; run the allot.exe from the build output instead, such as \
             target\\release\\allot.exe install",
            bin.display()
        );
    }
    if let Some(missing) = BINARIES.iter().find(|name| !source.join(name).is_file()) {
        bail!(
            "{} has no {missing}; run `cargo build --release` in the workspace first",
            source.display()
        );
    }
    let tray_build = tray::find_build(&source, options.tray)?;
    if cfg!(debug_assertions) {
        println!("note: this allot is a debug build; install from target\\release for speed");
    }

    let taken_over = legacy::take_over(&paths)?;
    let tray_dir = tray::dir(&paths);
    // Before allotd restarts, so the tray app does not report the service stopping. Asked through
    // the installed exe: a launch of the same app from another path does not reach the running
    // one.
    let installed_tray = tray::exe(&tray_dir);
    if tray_build.is_some() && installed_tray.is_file() && !tray::quit(&installed_tray) {
        println!(
            "warning: the tray app did not quit; its files are renamed aside, and it keeps \
             running until you quit it from its tray menu"
        );
    }

    fs::create_dir_all(&bin).with_context(|| format!("creating {}", bin.display()))?;
    for name in BINARIES {
        replace_binary(&source.join(name), &bin.join(name))?;
    }
    println!("installed {} into {}", BINARIES.join(", "), bin.display());

    if options.setup.claude {
        let skill = registration::skill_dir(&paths).join(SKILL_FILE);
        write_file(&skill, SKILL)?;
        println!("copied the resource-guard skill to {}", skill.display());
    }
    remove_if_present(&paths.root().join(OLD_RULE_FILE))?;

    let replaces_old_daemon = taken_over.is_some_and(|taken| taken.daemon_stopped);
    if let Err(error) = restart_daemon(&paths, replaces_old_daemon) {
        println!(
            "warning: allotd was not restarted ({error:#}); it runs the new build from its next start"
        );
    }
    let mut leftovers = remove_leftovers(&bin);
    let launch_at_logon = options
        .launch_at_logon
        .or(taken_over.and_then(|taken| taken.launch_at_logon));
    match &tray_build {
        Some(build) => leftovers.extend(install_tray(build, &tray_dir, launch_at_logon)?),
        None => println!(
            "no tray app in {}; {} (publish it with `dotnet publish -c Release` in \
             tray\\src\\Allot.Tray and pass --tray <publish folder>)",
            source.join("tray").display(),
            if tray_dir.exists() {
                "kept the installed one"
            } else {
                "installed without it"
            }
        ),
    }

    registration::register(&paths, options.setup);
    // After the new entries are in place, so a session is never left without its hooks.
    registration::print_manual_steps(&registration::unregister_legacy(&paths));
    leftovers.extend(legacy::clean_up(&paths));
    report_leftovers(&leftovers);
    Ok(())
}

/// Mirrors the tray build into `target`, sets Launch at logon (`None`: on for a first tray
/// install, untouched otherwise), and starts it. Returns the files still in use.
fn install_tray(
    build: &Path,
    target: &Path,
    launch_at_logon: Option<bool>,
) -> anyhow::Result<Vec<PathBuf>> {
    let exe = tray::exe(target);
    let first = !exe.is_file();
    let in_use = tray::mirror(build, target)?;
    println!("installed the tray app into {}", target.display());
    if let Some(on) = launch_at_logon.or(first.then_some(true)) {
        let state = if on { "on" } else { "off" };
        match tray::set_launch_at_logon(&exe, on) {
            Ok(()) => println!("turned {state} the tray app's Launch at logon"),
            Err(error) => println!("warning: Launch at logon was not turned {state} ({error:#})"),
        }
    }
    match tray::start(target) {
        Ok(()) => println!("started the tray app"),
        Err(error) => println!("warning: {error:#}"),
    }
    Ok(in_use)
}

/// Quits and removes the tray app and its Launch at logon entry, stops allotd, removes the
/// binaries and the skill copy, unregisters what Claude Code has, and prints what is left to
/// undo by hand. The database, token, config, settings, and logs stay unless `purge`.
///
/// # Errors
///
/// Fails when allotd keeps running or a file that is not in use cannot be removed.
pub fn uninstall(purge: bool) -> anyhow::Result<()> {
    allot_sampler::keep_std_handles_private();
    let paths = Paths::from_env()?;
    let tray_dir = tray::dir(&paths);
    let mut in_use = Vec::new();
    let tray_exe = tray::exe(&tray_dir);
    if tray_exe.is_file() {
        if let Err(error) = tray::set_launch_at_logon(&tray_exe, false) {
            println!("warning: Launch at logon was not turned off ({error:#})");
        }
        if !tray::quit(&tray_exe) {
            println!("warning: the tray app did not quit; quit it from its tray menu");
        }
    }
    if tray_dir.exists() {
        in_use.extend(tray::remove(&tray_dir));
        println!("removed the tray app");
    }
    // Before the binaries go, so sessions stop calling the hook's allot.exe first.
    let mut manual_steps = registration::unregister(&paths);
    manual_steps.extend(registration::unregister_legacy(&paths));

    let client = BlockingClient::new(client_options(&paths))?;
    if let Some(old_pid) = daemon::shutdown(&client)? {
        // Open sessions can start allotd again from the binaries about to be removed.
        if let Successor::Running { pid } = wait_for_exit(&paths.lock_file(), &client, old_pid)? {
            println!("warning: a session started allotd again (pid {pid}); close open sessions");
        } else {
            println!("allotd stopped");
        }
    }

    let bin = paths.bin_dir();
    in_use.extend(remove_binaries(&bin)?);
    let skill_dir = registration::skill_dir(&paths);
    remove_if_present(&skill_dir.join(SKILL_FILE))?;
    // Only an empty directory is removed: anything else in it is not ours.
    let _ = fs::remove_dir(&skill_dir);
    remove_if_present(&paths.root().join(OLD_RULE_FILE))?;
    in_use.extend(legacy::clean_up(&paths));

    if purge {
        in_use.extend(purge_root(paths.root())?);
        println!("removed {}", paths.root().display());
        let old = legacy::root(&paths);
        if old.exists() {
            in_use.extend(purge_root(&old)?);
            println!("removed {}", old.display());
        }
    } else {
        let _ = fs::remove_dir(&bin);
        println!(
            "removed the binaries and the skill copy; kept the database, token, config, and \
             logs in {} (`allot uninstall --purge` removes them)",
            paths.root().display()
        );
    }
    report_leftovers(&in_use);
    registration::print_manual_steps(&manual_steps);
    Ok(())
}

fn current_exe_dir() -> anyhow::Result<PathBuf> {
    let exe = env::current_exe().context("finding this executable")?;
    exe.parent()
        .map(Path::to_owned)
        .context("this executable has no parent directory")
}

/// Whether both directories exist and are the same one.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Copies `source` over `target`. A running executable cannot be overwritten or deleted, but
/// it can be renamed, so the old file is renamed aside first and deleted when nothing runs
/// it; otherwise a later install deletes it.
fn replace_binary(source: &Path, target: &Path) -> anyhow::Result<()> {
    let aside = if target.exists() {
        let aside = aside_path(target);
        fs::rename(target, &aside)
            .with_context(|| format!("renaming {} aside", target.display()))?;
        Some(aside)
    } else {
        None
    };
    if let Err(error) = fs::copy(source, target) {
        if let Some(aside) = &aside {
            let _ = fs::rename(aside, target);
        }
        return Err(error)
            .with_context(|| format!("copying {} to {}", source.display(), target.display()));
    }
    if let Some(aside) = aside {
        let _ = fs::remove_file(aside);
    }
    Ok(())
}

/// `<stem>.old-<unix millis>.exe` beside `target`.
pub(crate) fn aside_path(target: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let stem = target.file_stem().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!("{stem}.old-{millis}.exe"))
}

/// Deletes every executable in `bin` that is not part of the install, such as old builds
/// renamed aside, and returns the ones still in use.
fn remove_leftovers(bin: &Path) -> Vec<PathBuf> {
    executables(bin)
        .into_iter()
        .filter(|path| {
            !BINARIES
                .iter()
                .any(|name| path.file_name() == Some(name.as_ref()))
        })
        .filter(|path| fs::remove_file(path).is_err())
        .collect()
}

/// Deletes every executable in `bin`, renaming the ones in use aside so a later install can
/// write fresh copies, and returns those.
fn remove_binaries(bin: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut in_use = Vec::new();
    for path in executables(bin) {
        if fs::remove_file(&path).is_ok() {
            continue;
        }
        let is_installed = BINARIES
            .iter()
            .any(|name| path.file_name() == Some(name.as_ref()));
        if is_installed {
            let aside = aside_path(&path);
            fs::rename(&path, &aside)
                .with_context(|| format!("removing or renaming {}", path.display()))?;
            in_use.push(aside);
        } else {
            in_use.push(path);
        }
    }
    Ok(in_use)
}

fn executables(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        })
        .collect()
}

/// Removes everything under `root` except executables still in use, which are returned.
fn purge_root(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", root.display())),
    };
    for entry in entries {
        let path = entry?.path();
        let removed = if path.is_dir() {
            // The bin directory still holds the executables in use.
            if executables(&path).is_empty() {
                fs::remove_dir_all(&path)
            } else {
                continue;
            }
        } else {
            fs::remove_file(&path)
        };
        removed.with_context(|| format!("removing {}", path.display()))?;
    }
    let in_use = executables(&root.join("bin"));
    if in_use.is_empty() {
        fs::remove_dir(root).with_context(|| format!("removing {}", root.display()))?;
    }
    Ok(in_use)
}

fn report_leftovers(in_use: &[PathBuf]) {
    if in_use.is_empty() {
        return;
    }
    println!("still in use, delete later (the next install deletes them):");
    for path in in_use {
        println!("  {}", path.display());
    }
}

fn remove_if_present(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("removing {}", path.display()))
        }
        _ => Ok(()),
    }
}

fn write_file(path: &Path, contents: &str) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

/// Client options for the install at `paths` that never start allotd. An invalid config falls
/// back to the default port, so a broken config cannot block an install or uninstall.
fn client_options(paths: &Paths) -> ClientOptions {
    let config = Config::load(&paths.config_file()).unwrap_or_else(|error| {
        eprintln!("warning: {error}; using the default port");
        Config::default()
    });
    ClientOptions::local(paths, &config, false)
}

/// What took the place of a stopped allotd.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Successor {
    /// A session's allot-mcp started the installed allotd before the installer could.
    Running { pid: u32 },
    /// No allotd holds the instance lock any more.
    None,
}

/// Stops a running allotd and starts the installed one. A stopped allotd stays stopped (the
/// logon task or the first session that needs it starts it), unless it `replaces` the daemon of
/// the install from before the rename, which open sessions still talk to.
fn restart_daemon(paths: &Paths, replaces: bool) -> anyhow::Result<()> {
    let mut options = client_options(paths);
    let client = BlockingClient::new(options.clone())?;
    let running = match daemon::shutdown(&client)? {
        Some(old_pid) => wait_for_exit(&paths.lock_file(), &client, old_pid)?,
        None if replaces => Successor::None,
        None => {
            println!("allotd is not running; the logon task or the first session starts it");
            return Ok(());
        }
    };
    let pid = match running {
        Successor::Running { pid } => pid,
        Successor::None => {
            options.spawn = Some(SpawnOptions::at(paths.bin_dir().join(DAEMON_EXE)));
            let client = BlockingClient::new(options)?;
            client.ensure_running()?;
            client.health()?.pid
        }
    };
    println!("restarted allotd on the new build (pid {pid}); its leases were restored");
    Ok(())
}

/// Waits until the allotd with `old_pid` has exited: no allotd holds the instance `lock`, or
/// another one answers. allotd stops answering before its process exits, and an allotd started
/// in between would find the lock taken and quit.
///
/// # Errors
///
/// Fails when the lock file cannot be opened or the old allotd still runs after [`EXIT_WAIT`].
pub(crate) fn wait_for_exit(
    lock: &Path,
    client: &BlockingClient,
    old_pid: u32,
) -> anyhow::Result<Successor> {
    let deadline = Instant::now() + EXIT_WAIT;
    loop {
        if let Ok(health) = client.health()
            && health.pid != old_pid
        {
            return Ok(Successor::Running { pid: health.pid });
        }
        if lock_is_free(lock)? {
            return Ok(Successor::None);
        }
        if Instant::now() >= deadline {
            bail!(
                "the old allotd (pid {old_pid}) stopped answering but has not exited after {} s",
                EXIT_WAIT.as_secs()
            );
        }
        thread::sleep(EXIT_POLL);
    }
}

fn lock_is_free(lock: &Path) -> anyhow::Result<bool> {
    match OpenOptions::new().write(true).open(lock) {
        Ok(file) => Ok(file.try_lock().is_ok()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error).with_context(|| format!("opening {}", lock.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::{os::windows::fs::OpenOptionsExt, process};

    use super::*;

    /// A fresh directory under the system temp directory, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!("allot-install-{}-{name}", process::id()));
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

    // Win32 share modes: what a running executable's image allows other openers.
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_DELETE: u32 = 0x4;

    /// Opens `path` the way the loader holds a running exe: readable and renamable, but not
    /// writable.
    fn hold_like_a_running_exe(path: &Path) -> fs::File {
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .open(path)
            .unwrap()
    }

    #[test]
    fn test_replace_binary_swaps_a_file_held_like_a_running_exe() {
        let dir = TempDir::new("swap");
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let source = dir.path().join("new.exe");
        let target = bin.join("allot-mcp.exe");
        fs::write(&source, "new").unwrap();
        fs::write(&target, "old").unwrap();
        let held = hold_like_a_running_exe(&target);
        assert!(
            fs::copy(&source, &target).is_err(),
            "a plain copy must fail"
        );

        replace_binary(&source, &target).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        drop(held);
        let leftovers = remove_leftovers(&bin);
        assert!(leftovers.is_empty(), "{leftovers:?}");
        assert_eq!(executables(&bin), [target]);
    }

    #[test]
    fn test_replace_binary_into_an_empty_dir_copies() {
        let dir = TempDir::new("empty");
        let source = dir.path().join("src.exe");
        let target = dir.path().join("allot.exe");
        fs::write(&source, "build").unwrap();
        replace_binary(&source, &target).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "build");
    }

    #[test]
    fn test_remove_leftovers_keeps_installed_binaries_and_other_files() {
        let dir = TempDir::new("leftovers");
        for name in [
            "allot.exe",
            "allot-mcp-old.exe",
            "allotd.old-1.exe",
            "notes.txt",
        ] {
            fs::write(dir.path().join(name), name).unwrap();
        }
        assert!(remove_leftovers(dir.path()).is_empty());
        assert!(dir.path().join("allot.exe").is_file());
        assert!(dir.path().join("notes.txt").is_file());
        assert!(!dir.path().join("allot-mcp-old.exe").exists());
        assert!(!dir.path().join("allotd.old-1.exe").exists());
    }

    #[test]
    fn test_aside_path_keeps_the_stem_and_exe_extension() {
        let aside = aside_path(Path::new(r"C:\bin\allot-mcp.exe"));
        let name = aside.file_name().unwrap().to_string_lossy();
        assert!(
            name.starts_with("allot-mcp.old-") && name.ends_with(".exe"),
            "{name}"
        );
        assert_eq!(aside.parent(), Some(Path::new(r"C:\bin")));
    }

    #[test]
    fn test_lock_is_free_only_while_no_one_holds_it() {
        let dir = TempDir::new("lock");
        let lock = dir.path().join("allotd.lock");
        assert!(lock_is_free(&lock).unwrap(), "a missing file is free");

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock)
            .unwrap();
        file.lock().unwrap();
        assert!(!lock_is_free(&lock).unwrap());

        drop(file);
        assert!(lock_is_free(&lock).unwrap());
    }
}
