//! `allot update`: finds the latest release, downloads it, checks it against its `.sha256`, and
//! installs it over `~/.allot/bin` (through the release's own `allot install` on Linux and macOS,
//! through its setup on Windows), the panels Allot owns on Linux, and through Allot.app on a Mac
//! that has an app which takes the hand-off. What a run does is recorded in
//! `~/.allot/update-install.json` for allotd.

use std::{cmp::Ordering, fs, io::ErrorKind, path::Path};

use allot_shared::{
    paths::Paths,
    platform::CLI_EXE,
    update::{UpdateState, UpdateStatus},
};
use allot_updater::{Check, Release, Updater, Version, folder, platform_asset};
use anyhow::bail;

use crate::{
    channel,
    guard::{self, Installed},
    output::print_json,
    registration::{self, Setup},
    status::now_ms,
};

mod lock;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(
    not(target_os = "macos"),
    allow(dead_code, reason = "tested on every OS, used on macOS")
)]
mod mac;
mod outcome;
#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "tested on every OS, used on Linux")
)]
mod panels;
#[cfg(any(windows, test))]
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "tested on every OS, used on Windows")
)]
mod setup;
#[cfg(any(unix, test))]
#[cfg_attr(
    not(unix),
    allow(dead_code, reason = "tested on every OS, used on Linux and macOS")
)]
mod unix;

use lock::RunLock;
use outcome::Record;

/// What `allot update` was asked.
#[derive(Debug, Clone)]
pub struct Options {
    /// Only report whether a newer release exists.
    pub check: bool,
    /// With `check`, print the report as JSON.
    pub json: bool,
    /// Install this release instead of the latest.
    pub version: Option<Version>,
    /// Allow installing a release older than the installed one.
    pub allow_downgrade: bool,
}

/// Everything one install needs to know.
struct Job<'a> {
    paths: &'a Paths,
    updater: &'a Updater,
    release: &'a Release,
    /// What `~/.allot/bin` holds now.
    installed: &'a Version,
    allow_downgrade: bool,
    /// What this install has set up, which the new one keeps.
    setup: Setup,
}

/// What the request comes to.
enum Choice {
    Install(Release),
    /// Nothing to install; the line says why.
    Nothing(String),
}

/// Reports whether a newer release exists (`--check`), or updates to the latest release or the
/// one asked for.
///
/// # Errors
///
/// Fails when `~/.allot/bin` holds no allot that reports a version, when the release cannot be
/// found, downloaded or checked, when a pinned version is older than the installed one and the
/// downgrade is not allowed, or when the install does not finish or leaves another version.
pub fn update(options: Options) -> anyhow::Result<()> {
    let paths = Paths::from_env()?;
    let updater = Updater::from_env()?;
    let installed = installed_version(&paths)?;
    if options.check {
        return check(&paths, &updater, &installed, options.json);
    }
    let release = match choose(&paths, &updater, &installed, &options)? {
        Choice::Install(release) => release,
        Choice::Nothing(line) => {
            println!("{line}");
            print_path_notice(&paths);
            return Ok(());
        }
    };
    // The setup starts the new allotd and tray, which must not inherit this process's output.
    #[cfg(windows)]
    allot_sampler::keep_std_handles_private();
    let _lock = RunLock::take(&paths)?;
    let job = Job {
        paths: &paths,
        updater: &updater,
        release: &release,
        installed: &installed,
        allow_downgrade: options.allow_downgrade,
        setup: registration::current_setup(&paths),
    };
    run(&job)
}

/// The version of the allot in `~/.allot/bin`.
fn installed_version(paths: &Paths) -> anyhow::Result<Version> {
    let exe = paths.bin_dir().join(CLI_EXE);
    match guard::installed(paths) {
        Installed::Version(version) => Ok(version),
        Installed::Missing => bail!(
            "{} does not exist, so there is nothing to update; run `allot install` from a release \
             folder, or the one-line install (README, Install)",
            exe.display()
        ),
        Installed::Unreadable => bail!(
            "{} does not report its version, so `allot update` cannot tell what it would \
             replace; run the one-line install (README, Install) to repair it",
            exe.display()
        ),
    }
}

fn choose(
    paths: &Paths,
    updater: &Updater,
    installed: &Version,
    options: &Options,
) -> anyhow::Result<Choice> {
    if let Some(pin) = &options.version {
        let bin = paths.bin_dir().join(CLI_EXE);
        return Ok(
            if pin_changes(installed, pin, options.allow_downgrade, &bin)? {
                Choice::Install(updater.release(pin.clone()))
            } else {
                Choice::Nothing(format!("allot {installed} is already installed"))
            },
        );
    }
    match updater.check(installed)? {
        Check::Newer(release) => Ok(Choice::Install(release)),
        Check::NotNewer { latest } => Ok(Choice::Nothing(up_to_date(installed, latest.as_ref()))),
        other => bail!("the lookup found something this allot does not know: {other:?}"),
    }
}

fn up_to_date(installed: &Version, latest: Option<&Release>) -> String {
    match latest {
        Some(latest) => format!(
            "allot {installed} is up to date (the latest release is {})",
            latest.version
        ),
        None => format!("allot {installed} is up to date: there is no newer stable release"),
    }
}

/// Whether installing the pinned version `pin` changes anything: no when it is the installed
/// one, and an older one only when the downgrade is allowed.
fn pin_changes(
    installed: &Version,
    pin: &Version,
    allow_downgrade: bool,
    bin: &Path,
) -> anyhow::Result<bool> {
    match pin.cmp(installed) {
        Ordering::Equal => Ok(false),
        Ordering::Greater => Ok(true),
        Ordering::Less => guard::refuse_downgrade(
            &Installed::Version(installed.clone()),
            pin,
            bin,
            allow_downgrade,
        )
        .map(|()| true),
    }
}

/// `--check`: looks up the latest release and says whether it is newer, installing nothing.
fn check(paths: &Paths, updater: &Updater, installed: &Version, json: bool) -> anyhow::Result<()> {
    let found = updater.check(installed)?;
    let notes = channel::older_copy_on_path(paths, installed)
        .map(|copy| copy.notice())
        .into_iter()
        .collect();
    let status = status_of(installed, &found, now_ms(), notes)?;
    if json {
        return print_json(&status);
    }
    print!("{}", describe(&status));
    Ok(())
}

/// The report `--check` makes, in the shape of the contract's `UpdateStatus`.
fn status_of(
    installed: &Version,
    found: &Check,
    checked_at_ms: i64,
    notes: Vec<String>,
) -> anyhow::Result<UpdateStatus> {
    let (state, latest) = match found {
        Check::Newer(release) => (UpdateState::Available, Some(release)),
        Check::NotNewer { latest } => (UpdateState::UpToDate, latest.as_ref()),
        other => bail!("the lookup found something this allot does not know: {other:?}"),
    };
    Ok(UpdateStatus {
        installed: installed.to_string(),
        state,
        latest: latest.map(|release| release.version.to_string()),
        checked_at_ms: Some(checked_at_ms),
        release_url: latest.map(|release| release.page_url.clone()),
        reason: None,
        notes,
    })
}

fn describe(status: &UpdateStatus) -> String {
    let mut text = match (&status.state, &status.latest, &status.release_url) {
        (UpdateState::Available, Some(latest), Some(url)) => format!(
            "allot {} is installed; {latest} is available: {url}\nrun `allot update` to install it\n",
            status.installed
        ),
        (_, Some(latest), _) => format!(
            "allot {} is up to date (the latest release is {latest})\n",
            status.installed
        ),
        _ => format!(
            "allot {} is up to date: there is no newer stable release\n",
            status.installed
        ),
    };
    for note in &status.notes {
        text.push_str(&format!("note: {note}\n"));
    }
    text
}

fn print_path_notice(paths: &Paths) {
    if let Some(notice) = channel::notice(paths) {
        println!("note: {notice}");
    }
}

/// Installs the release, keeps the record of how it went, and cleans up after it.
fn run(job: &Job) -> anyhow::Result<()> {
    let record = Record::start(job.paths, &job.release.version);
    println!(
        "updating allot from {} to {}",
        job.installed, job.release.version
    );
    clean_old_downloads(job);
    match install(job).and_then(|notes| verify(job).map(|()| notes)) {
        Ok(notes) => {
            record.succeeded(&notes);
            remove_downloads(job);
            println!("allot is now {}", job.release.version);
            for note in &notes {
                println!("note: {note}");
            }
            print_path_notice(job.paths);
            Ok(())
        }
        Err(error) => {
            record.failed(format!("{error:#}"));
            Err(error)
        }
    }
}

/// Checks that the install holds the release, which the setup's exit code cannot promise.
fn verify(job: &Job) -> anyhow::Result<()> {
    match guard::installed(job.paths) {
        Installed::Version(found) if found == job.release.version => Ok(()),
        Installed::Version(found) => bail!(
            "expected allot {} in {}, found {found}",
            job.release.version,
            job.paths.bin_dir().display()
        ),
        Installed::Missing | Installed::Unreadable => bail!(
            "allot in {} does not report a version after the update",
            job.paths.bin_dir().display()
        ),
    }
}

/// Downloads the release's file for this platform and says so.
fn fetch(job: &Job) -> anyhow::Result<allot_updater::Verified> {
    let asset = platform_asset()?;
    let file = asset.file_name(&job.release.version.to_string());
    println!("downloading {file}");
    let verified = job.updater.fetch(job.paths, job.release, asset)?;
    println!(
        "checked {file} ({} bytes) against its .sha256",
        verified.bytes()
    );
    Ok(verified)
}

/// Removes the update folders of other versions. A failure only leaves them for the next run.
fn clean_old_downloads(job: &Job) {
    if let Err(error) = folder::clean(job.paths, &job.release.version) {
        println!("warning: old downloads were not removed ({error})");
    }
}

fn remove_downloads(job: &Job) {
    let folder = folder::of(job.paths, &job.release.version);
    match fs::remove_dir_all(&folder) {
        Err(error) if error.kind() != ErrorKind::NotFound => {
            println!("warning: {} was not removed ({error})", folder.display());
        }
        _ => {}
    }
}

/// The setup runs silently, and its log says what `allot.exe install` did.
#[cfg(windows)]
fn install(job: &Job) -> anyhow::Result<Vec<String>> {
    let verified = fetch(job)?;
    setup::run(verified.path(), job.setup.claude, job.allow_downgrade)?;
    Ok(Vec::new())
}

/// The release installs itself; on a Mac with an app that takes the hand-off, the app does.
#[cfg(unix)]
fn install(job: &Job) -> anyhow::Result<Vec<String>> {
    let mut notes = Vec::new();
    #[cfg(target_os = "macos")]
    if hand_off_to_app(job, &mut notes)? {
        return Ok(notes);
    }
    let verified = fetch(job)?;
    let root = job.updater.unpack(&verified)?;
    unix::run(&root, job.setup, job.allow_downgrade)?;
    #[cfg(target_os = "linux")]
    notes.extend(panels::update(job.paths, job.updater, job.release));
    Ok(notes)
}

/// Hands the update to Allot.app and waits for it to bring `~/.allot/bin` along. Returns whether
/// it did; without an app that takes the hand-off, or when the hand-off fails, `~/.allot/bin` is
/// updated alone and the app keeps its version.
#[cfg(target_os = "macos")]
fn hand_off_to_app(job: &Job, notes: &mut Vec<String>) -> anyhow::Result<bool> {
    let Some(app) = mac::find(job.paths) else {
        return Ok(false);
    };
    // The app only moves forward.
    if job.release.version < *job.installed {
        return Ok(false);
    }
    let version = app.version.as_deref().unwrap_or("of an unknown version");
    if !app.takes_hand_off {
        notes.push(format!(
            "Allot.app {version} cannot take an update from the command line, so it keeps its \
             version; update it from the DMG or with `brew upgrade --cask allot-app`"
        ));
        return Ok(false);
    }
    println!("handing the update to {}", app.path.display());
    if let Err(error) = mac::hand_off(&app.path, &job.release.version) {
        println!(
            "warning: {error:#}; updating {} alone",
            job.paths.bin_dir().display()
        );
        notes.push(format!(
            "Allot.app {version} did not take the update, so it keeps its version"
        ));
        return Ok(false);
    }
    wait_for_install(job)?;
    Ok(true)
}

/// Waits until `~/.allot/bin` holds the release, for the app that was handed the update.
#[cfg(target_os = "macos")]
fn wait_for_install(job: &Job) -> anyhow::Result<()> {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    let deadline = Instant::now() + mac::HAND_OFF_WAIT;
    println!("waiting for Allot.app to update itself and the service");
    loop {
        if verify(job).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "Allot.app has not brought {} to {} within {} s; open the app to see why",
                job.paths.bin_dir().display(),
                job.release.version,
                mac::HAND_OFF_WAIT.as_secs()
            );
        }
        thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use allot_updater::Source;

    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn release(text: &str) -> Release {
        Updater::new(Source::github(), allot_updater::Limits::default()).release(version(text))
    }

    #[test]
    fn test_pin_changes_installs_a_newer_version_and_skips_the_installed_one() {
        let bin = Path::new("bin");
        assert!(pin_changes(&version("0.6.1"), &version("0.7.0"), false, bin).unwrap());
        assert!(!pin_changes(&version("0.6.1"), &version("0.6.1"), false, bin).unwrap());
    }

    #[test]
    fn test_pin_changes_refuses_an_older_version_unless_the_downgrade_is_allowed() {
        let bin = Path::new("bin");
        let message = pin_changes(&version("0.7.0"), &version("0.6.0"), false, bin)
            .unwrap_err()
            .to_string();
        assert!(message.contains("--allow-downgrade"), "{message}");

        assert!(pin_changes(&version("0.7.0"), &version("0.6.0"), true, bin).unwrap());
    }

    #[test]
    fn test_status_of_a_newer_release_is_available_with_its_page() {
        let found = Check::Newer(release("0.7.0"));

        let status = status_of(&version("0.6.1"), &found, 42, vec!["note".to_owned()]).unwrap();

        assert_eq!(status.state, UpdateState::Available);
        assert_eq!(status.installed, "0.6.1");
        assert_eq!(status.latest.as_deref(), Some("0.7.0"));
        assert_eq!(
            status.release_url.as_deref(),
            Some("https://github.com/meh7an/allot/releases/tag/v0.7.0")
        );
        assert_eq!(status.checked_at_ms, Some(42));
        assert_eq!(status.reason, None);
        assert_eq!(status.notes, ["note"]);
    }

    #[test]
    fn test_status_of_no_newer_release_is_up_to_date_and_keeps_the_latest_it_found() {
        let found = Check::NotNewer {
            latest: Some(release("0.6.1")),
        };
        let status = status_of(&version("0.6.1"), &found, 7, Vec::new()).unwrap();
        assert_eq!(status.state, UpdateState::UpToDate);
        assert_eq!(status.latest.as_deref(), Some("0.6.1"));

        let none = Check::NotNewer { latest: None };
        let status = status_of(&version("0.6.1"), &none, 7, Vec::new()).unwrap();
        assert_eq!(status.state, UpdateState::UpToDate);
        assert_eq!(status.latest, None);
        assert_eq!(status.release_url, None);
    }

    #[test]
    fn test_status_json_is_the_contracts_update_status() {
        let found = Check::Newer(release("0.7.0"));
        let status = status_of(&version("0.6.1"), &found, 42, Vec::new()).unwrap();

        let json = serde_json::to_value(&status).unwrap();

        assert_eq!(json["state"], "available");
        assert_eq!(json["installed"], "0.6.1");
        assert_eq!(json["notes"], serde_json::json!([]));
    }

    #[test]
    fn test_describe_says_what_to_run_when_an_update_is_available() {
        let found = Check::Newer(release("0.7.0"));
        let status = status_of(&version("0.6.1"), &found, 1, Vec::new()).unwrap();

        let text = describe(&status);

        assert!(
            text.starts_with("allot 0.6.1 is installed; 0.7.0 is available: "),
            "{text}"
        );
        assert!(text.contains("run `allot update`"), "{text}");
    }

    #[test]
    fn test_describe_an_up_to_date_install_names_the_latest_and_the_notes() {
        let found = Check::NotNewer {
            latest: Some(release("0.6.1")),
        };
        let status = status_of(
            &version("0.6.1"),
            &found,
            1,
            vec!["allot on PATH is old".to_owned()],
        )
        .unwrap();

        assert_eq!(
            describe(&status),
            "allot 0.6.1 is up to date (the latest release is 0.6.1)\n\
             note: allot on PATH is old\n"
        );
    }

    #[test]
    fn test_up_to_date_without_a_latest_release_says_so() {
        assert_eq!(
            up_to_date(&version("0.6.1"), None),
            "allot 0.6.1 is up to date: there is no newer stable release"
        );
    }

    #[test]
    fn test_installed_version_names_the_fix_for_a_missing_install() {
        let home = crate::test_support::TempDir::new();
        let paths = Paths::under(home.path());

        let message = installed_version(&paths).unwrap_err().to_string();

        assert!(message.contains("nothing to update"), "{message}");
        assert!(message.contains("allot install"), "{message}");
    }
}
