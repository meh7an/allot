//! `allot install` and `allot uninstall`: the binaries in `~/.allot/bin`
//! (`%USERPROFILE%\.allot\bin` on Windows), the tray app in `bin\tray` on Windows, the link
//! on `PATH` elsewhere, the skill copy, restarting allotd on the new build, and the
//! registrations with Claude Code and the OS.

use std::{
    env,
    fs::{self, OpenOptions},
    io,
    path::{MAIN_SEPARATOR, Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use allot_client::{BlockingClient, ClientOptions, SpawnOptions};
#[cfg(unix)]
use allot_shared::platform::DAEMON_LOG_TEXT;
use allot_shared::{
    config::Config,
    paths::Paths,
    platform::{ALLOT_BINARIES, CLI_EXE, DAEMON_EXE, EXE_SUFFIX},
};
use anyhow::{Context, bail};

use crate::{daemon, registration};
#[cfg(windows)]
use crate::{legacy, tray};

/// The executables of an install.
const BINARIES: [&str; 3] = ALLOT_BINARIES;
/// Where `cargo build --release` puts the binaries, relative to the workspace.
const RELEASE_DIR: &str = if cfg!(windows) {
    r"target\release"
} else {
    "target/release"
};
const SKILL: &str = include_str!("../../skill/resource-guard/SKILL.md");
/// The rule copy older installs left for pasting; the rule is imported into CLAUDE.md now.
const OLD_RULE_FILE: &str = "claude-md-rule.md";
const SKILL_FILE: &str = "SKILL.md";
/// How long to wait for a stopped allotd to exit. Older builds finish open acquire waits first,
/// which can take a minute.
const EXIT_WAIT: Duration = Duration::from_secs(90);
const EXIT_POLL: Duration = Duration::from_millis(100);
/// How long an allotd its autostart started has to answer, as long as the client gives one it
/// starts itself.
#[cfg(unix)]
const READY_WAIT: Duration = Duration::from_secs(5);

/// What `allot install` installs and sets up.
#[derive(Debug, Clone, Copy)]
pub struct Options<'a> {
    /// The build output (default: this executable's directory).
    pub from: Option<&'a Path>,
    /// The tray app's publish folder (default: the `tray` folder in `from`, when it has one).
    #[cfg(windows)]
    pub tray: Option<&'a Path>,
    /// The tray app's Launch at logon; `None` turns it on only when the tray app is installed
    /// for the first time, so a later install keeps what the user chose.
    #[cfg(windows)]
    pub launch_at_logon: Option<bool>,
    /// The Claude Code integration (with the skill) and allotd's autostart.
    pub setup: registration::Setup,
}

/// Takes over an install from before the rename (Windows), copies the built binaries into the
/// bin directory, copies the skill, restarts allotd on the new build when it was running,
/// installs and starts the tray app (Windows), and registers the rest with Claude Code and the
/// OS, as `options` ask. A failed restart or tray start is reported, not returned: the install
/// itself is complete by then.
///
/// # Errors
///
/// Fails when the source is the bin directory itself or lacks a binary, when the given tray
/// folder has no tray app, when the old daemon does not stop, or when a file cannot be
/// written.
pub fn install(options: Options) -> anyhow::Result<()> {
    // allotd and the tray app outlive this run; the setup's log must not stay open in them.
    #[cfg(windows)]
    allot_sampler::keep_std_handles_private();
    let paths = Paths::from_env()?;
    let source = match options.from {
        Some(dir) => dir.to_owned(),
        None => current_exe_dir()?,
    };
    let bin = paths.bin_dir();
    if same_dir(&source, &bin) {
        bail!(
            "{} is the install itself; run the {CLI_EXE} from the build output instead, such as \
             {RELEASE_DIR}{MAIN_SEPARATOR}{CLI_EXE} install",
            bin.display()
        );
    }
    if let Some(missing) = BINARIES.iter().find(|name| !source.join(name).is_file()) {
        bail!(
            "{} has no {missing}; run `cargo build --release` in the workspace first",
            source.display()
        );
    }
    #[cfg(windows)]
    let tray_build = tray::find_build(&source, options.tray)?;
    if cfg!(debug_assertions) {
        println!("note: this allot is a debug build; install from {RELEASE_DIR} for speed");
    }

    #[cfg(unix)]
    {
        copy_binaries(&source, &bin)?;
        link_on_path(&paths);
        if options.setup.claude {
            copy_skill(&paths)?;
        }
        // Stopped before the autostart step, which starts the new build.
        let client = client_options(&paths);
        let stopped = stop_daemon(&paths, &client);
        let leftovers = remove_leftovers(&bin);
        let autostarted = registration::register(&paths, options.setup);
        restart_after_setup(&paths, stopped, autostarted, client);
        report_leftovers(&leftovers);
        Ok(())
    }
    #[cfg(windows)]
    install_on_windows(options, paths, source, bin, tray_build)
}

fn copy_binaries(source: &Path, bin: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(bin).with_context(|| format!("creating {}", bin.display()))?;
    for name in BINARIES {
        replace_binary(&source.join(name), &bin.join(name))?;
    }
    println!("installed {} into {}", BINARIES.join(", "), bin.display());
    Ok(())
}

/// Where Claude Code's native installer puts `claude`, so a directory on the user's `PATH`
/// wherever it ran.
#[cfg(unix)]
fn path_link(paths: &Paths) -> PathBuf {
    paths.home().join(".local").join("bin").join(CLI_EXE)
}

/// Links `allot` into `~/.local/bin` so it runs from anywhere, unless something else of that
/// name is there; without that directory, says how to put the bin directory on `PATH`.
#[cfg(unix)]
fn link_on_path(paths: &Paths) {
    let link = path_link(paths);
    let target = paths.bin_dir().join(CLI_EXE);
    let Some(dir) = link.parent().filter(|dir| dir.is_dir()) else {
        println!(
            "add {} to PATH to run allot from anywhere",
            paths.bin_dir().display()
        );
        return;
    };
    match fs::read_link(&link) {
        Ok(existing) if existing == target => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match std::os::unix::fs::symlink(&target, &link) {
                Ok(()) => println!("linked {} to {}", link.display(), target.display()),
                Err(error) => println!(
                    "warning: {} was not linked into {} ({error})",
                    target.display(),
                    dir.display()
                ),
            }
        }
        _ => println!(
            "left {} alone: it is not a link to {}",
            link.display(),
            target.display()
        ),
    }
}

/// Removes the link [`link_on_path`] made, when it still points at this install.
#[cfg(unix)]
fn unlink_from_path(paths: &Paths) {
    let link = path_link(paths);
    let target = paths.bin_dir().join(CLI_EXE);
    if fs::read_link(&link).is_ok_and(|existing| existing == target)
        && fs::remove_file(&link).is_ok()
    {
        println!("removed the link {}", link.display());
    }
}

fn copy_skill(paths: &Paths) -> anyhow::Result<()> {
    let skill = registration::skill_dir(paths).join(SKILL_FILE);
    write_file(&skill, SKILL)?;
    println!("copied the resource-guard skill to {}", skill.display());
    Ok(())
}

/// The rest of [`install`] on Windows, after the source was checked.
#[cfg(windows)]
fn install_on_windows(
    options: Options,
    paths: Paths,
    source: PathBuf,
    bin: PathBuf,
    tray_build: Option<PathBuf>,
) -> anyhow::Result<()> {
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

    copy_binaries(&source, &bin)?;
    if options.setup.claude {
        copy_skill(&paths)?;
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
#[cfg(windows)]
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

/// Quits and removes the tray app and its Launch at logon entry (Windows), stops allotd,
/// removes the binaries, the link on `PATH` (Unix), and the skill copy, unregisters what Claude
/// Code and the OS have, and prints what is left to undo by hand. The database, token, config,
/// settings, and logs stay unless `purge`.
///
/// # Errors
///
/// Fails when allotd keeps running or a file that is not in use cannot be removed.
pub fn uninstall(purge: bool) -> anyhow::Result<()> {
    #[cfg(windows)]
    allot_sampler::keep_std_handles_private();
    let paths = Paths::from_env()?;
    let mut in_use = Vec::new();
    #[cfg(windows)]
    {
        let tray_dir = tray::dir(&paths);
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
    }
    // Before the binaries go, so sessions stop calling the hook's allot.exe first.
    #[cfg_attr(
        unix,
        expect(unused_mut, reason = "only Windows has an old install to undo")
    )]
    let mut manual_steps = registration::unregister(&paths);
    #[cfg(windows)]
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
    #[cfg(unix)]
    unlink_from_path(&paths);
    in_use.extend(remove_binaries(&bin)?);
    let skill_dir = registration::skill_dir(&paths);
    remove_if_present(&skill_dir.join(SKILL_FILE))?;
    // Only an empty directory is removed: anything else in it is not ours.
    let _ = fs::remove_dir(&skill_dir);
    remove_if_present(&paths.root().join(OLD_RULE_FILE))?;
    #[cfg(windows)]
    in_use.extend(legacy::clean_up(&paths));

    if purge {
        in_use.extend(purge_root(paths.root())?);
        println!("removed {}", paths.root().display());
        #[cfg(windows)]
        {
            let old = legacy::root(&paths);
            if old.exists() {
                in_use.extend(purge_root(&old)?);
                println!("removed {}", old.display());
            }
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
#[cfg(windows)]
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

/// Copies `source` to a temporary file beside `target`, which then replaces `target` at once.
/// A running allotd keeps the file it started from, and macOS kills a running signed binary
/// that is overwritten in place, so the old file is never written to.
#[cfg(unix)]
fn replace_binary(source: &Path, target: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let temporary = target.with_file_name(format!(".{name}.allot-new"));
    let copied = fs::copy(source, &temporary)
        .and_then(|_| fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755)))
        .with_context(|| format!("copying {} to {}", source.display(), temporary.display()))
        .and_then(|()| {
            fs::rename(&temporary, target)
                .with_context(|| format!("replacing {}", target.display()))
        });
    if copied.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    copied
}

/// `<stem>.old-<unix millis>` beside `target`, with `.exe` on Windows.
pub(crate) fn aside_path(target: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let stem = target.file_stem().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!("{stem}.old-{millis}{EXE_SUFFIX}"))
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

/// The executables in `dir`: on Windows its `.exe` files, elsewhere every file, since Unix
/// executables have no extension and the bin directory holds nothing else.
fn executables(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && (cfg!(not(windows))
                    || path
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe")))
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
            // On Windows the bin directory still holds the executables in use; Unix deletes
            // files in use.
            if cfg!(not(windows)) || executables(&path).is_empty() {
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

/// What stopping the running allotd found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stopped {
    NotRunning,
    /// It exited, and nothing runs in its place.
    Exited,
    /// A session's allot-mcp started the installed allotd before the installer could.
    Replaced {
        pid: u32,
    },
}

/// Stops a running allotd and waits until it has exited.
fn stop_daemon(paths: &Paths, options: &ClientOptions) -> anyhow::Result<Stopped> {
    let client = BlockingClient::new(options.clone())?;
    let Some(old_pid) = daemon::shutdown(&client)? else {
        return Ok(Stopped::NotRunning);
    };
    Ok(match wait_for_exit(&paths.lock_file(), &client, old_pid)? {
        Successor::Running { pid } => Stopped::Replaced { pid },
        Successor::None => Stopped::Exited,
    })
}

/// Starts the installed allotd and returns its pid.
fn start_daemon(paths: &Paths, mut options: ClientOptions) -> anyhow::Result<u32> {
    options.spawn = Some(SpawnOptions::at(paths.bin_dir().join(DAEMON_EXE)));
    let client = BlockingClient::new(options)?;
    client.ensure_running()?;
    Ok(client.health()?.pid)
}

/// Stops a running allotd and starts the installed one. A stopped allotd stays stopped (the
/// logon task or the first session that needs it starts it), unless it `replaces` the daemon of
/// the install from before the rename, which open sessions still talk to.
#[cfg(windows)]
fn restart_daemon(paths: &Paths, replaces: bool) -> anyhow::Result<()> {
    let options = client_options(paths);
    let pid = match stop_daemon(paths, &options)? {
        Stopped::Replaced { pid } => pid,
        Stopped::Exited => start_daemon(paths, options)?,
        Stopped::NotRunning if replaces => start_daemon(paths, options)?,
        Stopped::NotRunning => {
            println!("allotd is not running; the logon task or the first session starts it");
            return Ok(());
        }
    };
    println!("restarted allotd on the new build (pid {pid}); its leases were restored");
    Ok(())
}

/// Reports allotd on the new build after the setup, which started it when it set up the
/// autostart (`autostarted`), and starts it here when it was `stopped` and nothing else did. A
/// stopped allotd stays stopped otherwise: the first session that needs it starts it.
#[cfg(unix)]
fn restart_after_setup(
    paths: &Paths,
    stopped: anyhow::Result<Stopped>,
    autostarted: bool,
    options: ClientOptions,
) {
    let restarted = stopped.and_then(|stopped| match stopped {
        Stopped::Replaced { pid } => Ok(Some(pid)),
        Stopped::Exited if autostarted => await_daemon(options).map(Some),
        Stopped::Exited => start_daemon(paths, options).map(Some),
        Stopped::NotRunning => Ok(None),
    });
    match restarted {
        Ok(Some(pid)) => {
            println!("restarted allotd on the new build (pid {pid}); its leases were restored");
        }
        Ok(None) if !autostarted => println!("allotd is not running; the first session starts it"),
        Ok(None) => {}
        Err(error) => println!(
            "warning: allotd was not restarted ({error:#}); it runs the new build from its next start"
        ),
    }
}

/// Waits until the allotd the autostart started answers, and returns its pid. Starting one here
/// as well would race it for the instance lock.
#[cfg(unix)]
fn await_daemon(options: ClientOptions) -> anyhow::Result<u32> {
    let client = BlockingClient::new(options)?;
    let deadline = Instant::now() + READY_WAIT;
    loop {
        if let Ok(health) = client.health() {
            return Ok(health.pid);
        }
        if Instant::now() >= deadline {
            bail!(
                "its autostart started it, but it did not answer within {} s; see {DAEMON_LOG_TEXT}",
                READY_WAIT.as_secs()
            );
        }
        thread::sleep(EXIT_POLL);
    }
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
    #[cfg(windows)]
    use std::os::windows::fs::OpenOptionsExt;
    use std::process;

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
    #[cfg(windows)]
    const FILE_SHARE_READ: u32 = 0x1;
    #[cfg(windows)]
    const FILE_SHARE_DELETE: u32 = 0x4;

    /// Opens `path` the way the loader holds a running exe: readable and renamable, but not
    /// writable.
    #[cfg(windows)]
    fn hold_like_a_running_exe(path: &Path) -> fs::File {
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .open(path)
            .unwrap()
    }

    #[cfg(windows)]
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

    #[cfg(unix)]
    #[test]
    fn test_replace_binary_on_unix_renames_an_executable_copy_over_the_target() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let dir = TempDir::new("rename");
        let source = dir.path().join("build-allotd");
        let target = dir.path().join("allotd");
        fs::write(&source, "new").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&target, "old").unwrap();
        // A running allotd keeps the file it started from: the old inode stays readable.
        let old = fs::File::open(&target).unwrap();
        let old_inode = old.metadata().unwrap().ino();

        replace_binary(&source, &target).unwrap();

        let new = fs::metadata(&target).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_ne!(new.ino(), old_inode, "replaced, not written in place");
        assert_eq!(new.permissions().mode() & 0o777, 0o755);
        assert_eq!(
            executables(dir.path()).len(),
            2,
            "no temporary file is left"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_leftovers_on_unix_deletes_every_other_file() {
        let dir = TempDir::new("unix-leftovers");
        for name in ["allot", "allotd", "allotd.old-1", ".allot-mcp.allot-new"] {
            fs::write(dir.path().join(name), name).unwrap();
        }
        assert!(remove_leftovers(dir.path()).is_empty());
        let mut kept: Vec<_> = executables(dir.path())
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        kept.sort();
        assert_eq!(kept, ["allot", "allotd"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_aside_path_on_unix_has_no_extension() {
        let aside = aside_path(Path::new("/bin/allot-mcp"));
        let name = aside.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("allot-mcp.old-"), "{name}");
        assert!(!name.contains(".exe"), "{name}");
    }

    #[cfg(windows)]
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

    #[cfg(windows)]
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
