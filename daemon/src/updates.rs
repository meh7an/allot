//! allotd's part of self-update: asking GitHub once a day whether a newer release exists, merging
//! the answer with what `allot update` records into the `update` of `GET /ui/state`, and
//! starting `allot update` for a panel.
//!
//! One request leaves the machine a day, from allotd alone, so every panel, `allot status` and
//! `allot doctor` read the same answer. The `update.check` setting turns it all off: no check,
//! and no update started from a panel.

mod start;
mod status;

use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use allot_shared::{
    paths::Paths,
    platform::CLI_EXE,
    update::{AnnounceRequest, UpdateCheck, UpdateOutcome, UpdateState, UpdateStatus},
};
use allot_updater::{Check, Updater, Version};
use anyhow::anyhow;
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
};
use tokio::time::{self, MissedTickBehavior};

pub use self::start::{LaunchError, UpdateRun};
use self::status::{Inputs, Launched, START_GRACE_MS, version_of};
use crate::{
    error::AppError,
    state::{AppState, Shared, now_ms},
};

/// How long after allotd starts the first check waits, so a start at login does not race the
/// network coming up and a short-lived allotd never asks.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(5 * 60);
/// Time between checks that succeeded.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// Time before a check that failed is tried again.
const RETRY_AFTER: Duration = Duration::from_secs(60 * 60);
/// How often the clock is looked at. The schedule runs on the wall clock, so a laptop that
/// slept through the day checks when it wakes.
const POLL: Duration = Duration::from_secs(60);
/// The lines of the update log scanned for the error `allot update` ended with.
const LOG_TAIL_BYTES: u64 = 4_096;

/// How allotd checks for updates and starts one. Everything a test needs to steer is a public
/// field.
#[derive(Debug, Clone)]
pub struct UpdateOptions {
    /// Where releases are read from, or why they cannot be: an `ALLOT_UPDATE_BASE` that is not
    /// a usable address, which every check then reports.
    pub updater: Result<Arc<Updater>, String>,
    /// The file that keeps the last check across restarts.
    pub check_file: std::path::PathBuf,
    /// The file `allot update` writes its run to.
    pub outcome_file: std::path::PathBuf,
    /// How long after allotd starts the first check waits.
    pub first_check_after: Duration,
    /// Time between checks that succeeded.
    pub every: Duration,
    /// Time before a check that failed is tried again.
    pub retry_after: Duration,
    /// How often the clock is looked at.
    pub poll: Duration,
    /// The run a panel's Update starts.
    pub run: UpdateRun,
}

impl UpdateOptions {
    /// Checks GitHub (or the address `ALLOT_UPDATE_BASE` names) five minutes after allotd
    /// starts and then daily, and updates through the `allot` of `paths`.
    pub fn standard(paths: &Paths) -> Self {
        Self {
            updater: Updater::from_env()
                .map(Arc::new)
                .map_err(|error| error.to_string()),
            check_file: paths.update_check_file(),
            outcome_file: paths.update_outcome_file(),
            first_check_after: FIRST_CHECK_AFTER,
            every: CHECK_EVERY,
            retry_after: RETRY_AFTER,
            poll: POLL,
            run: UpdateRun {
                program: paths.bin_dir().join(CLI_EXE),
                args: vec!["update".into()],
                log: paths.log_dir().join("update.log"),
            },
        }
    }

    /// Options that never reach a network and start nothing, for tests of other parts of
    /// allotd: the files lie in `folder`, the first check is a day away and would fail.
    pub fn inert(folder: &Path) -> Self {
        Self {
            updater: Err("this allotd was built without a release source".to_owned()),
            check_file: folder.join("update-check.json"),
            outcome_file: folder.join("update-install.json"),
            first_check_after: CHECK_EVERY,
            every: CHECK_EVERY,
            retry_after: RETRY_AFTER,
            poll: POLL,
            run: UpdateRun {
                program: folder.join("allot-is-not-installed"),
                args: vec!["update".into()],
                log: folder.join("update.log"),
            },
        }
    }
}

/// What allotd knows about updates.
#[derive(Debug)]
pub struct Updates {
    options: UpdateOptions,
    memory: Mutex<Memory>,
    /// Held while a start is checked and launched, so a double click starts one run.
    starting: tokio::sync::Mutex<()>,
}

#[derive(Debug, Default)]
struct Memory {
    /// The last check, from the file at start and from every check since.
    last: Option<UpdateCheck>,
    /// Whether a check is running now.
    checking: bool,
    /// The run allotd started, until a restart or another start replaces it.
    launched: Option<Launched>,
}

impl Updates {
    /// The state for `options`, with the last check read from its file.
    pub fn new(options: UpdateOptions) -> Self {
        let last = UpdateCheck::read(&options.check_file);
        Self {
            options,
            memory: Mutex::new(Memory {
                last,
                ..Memory::default()
            }),
            starting: tokio::sync::Mutex::new(()),
        }
    }

    fn memory(&self) -> MutexGuard<'_, Memory> {
        self.memory.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Marks a check as running; `false` when one already is.
    fn begin_check(&self) -> bool {
        let mut memory = self.memory();
        !std::mem::replace(&mut memory.checking, true)
    }

    fn end_check(&self) {
        self.memory().checking = false;
    }

    /// Replaces the stored check with what `change` makes of it, and keeps the result in the
    /// file. `change` returns `None` to leave everything as it is; the answer says whether it
    /// did not.
    ///
    /// The result is stored in memory even when the file cannot be written, so the status
    /// stays right until allotd restarts.
    fn store(
        &self,
        change: impl FnOnce(Option<&UpdateCheck>) -> Option<UpdateCheck>,
    ) -> anyhow::Result<bool> {
        let mut memory = self.memory();
        let Some(record) = change(memory.last.as_ref()) else {
            return Ok(false);
        };
        let written = record.write(&self.options.check_file);
        memory.last = Some(record);
        written
            .map_err(|error| anyhow!("writing {}: {error}", self.options.check_file.display()))?;
        Ok(true)
    }
}

/// The version of this allotd.
fn installed() -> Version {
    // The package version is a release number, so parsing it cannot fail; the fallback only
    // keeps the signature honest.
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

/// What `GET /ui/state` says about updates.
pub(crate) fn status(state: &Shared) -> UpdateStatus {
    let updates = &state.updates;
    let (check, checking, launched) = {
        let memory = updates.memory();
        (
            memory.last.clone(),
            memory.checking,
            memory.launched.clone(),
        )
    };
    let outcome = UpdateOutcome::read(&updates.options.outcome_file);
    // A record whose process allotd can see gone belongs to a run that died; before the first
    // sample allotd cannot tell.
    let run_alive = match (&outcome, state.latest()) {
        (Some(outcome), Some(latest)) => latest.processes.get(outcome.pid).is_some(),
        _ => true,
    };
    let now = now_ms();
    let silent = launched
        .as_ref()
        .is_some_and(|launched| now - launched.at_ms >= START_GRACE_MS);
    let silent_reason = if silent {
        silent_reason(&updates.options.run.log)
    } else {
        String::new()
    };
    status::build(&Inputs {
        installed: &installed(),
        enabled: state.settings().config.update.check,
        checking,
        check: check.as_ref(),
        outcome: outcome.as_ref(),
        launched: launched.as_ref(),
        run_alive,
        silent_reason: &silent_reason,
        now_ms: now,
    })
}

/// Why a run that was started wrote no record: the error its log ends with, when it has one.
fn silent_reason(log: &Path) -> String {
    let seconds = START_GRACE_MS / 1_000;
    let ended_with = last_error_line(log);
    match ended_with {
        Some(line) => format!("allot update stopped before it began to install: {line}"),
        None => format!(
            "allot update wrote no record within {seconds} seconds; its log is {}",
            log.display()
        ),
    }
}

/// The last line of the log's tail that starts with `Error:`.
fn last_error_line(log: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(log).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(LOG_TAIL_BYTES)))
        .ok()?;
    let mut tail = String::new();
    file.take(LOG_TAIL_BYTES).read_to_string(&mut tail).ok()?;
    tail.lines()
        .rev()
        .map(str::trim)
        .find(|line| line.starts_with("Error:"))
        .map(str::to_owned)
}

/// Whether the schedule asks for a check now.
fn is_due(last: Option<&UpdateCheck>, now_ms: i64, every: Duration, retry_after: Duration) -> bool {
    let Some(last) = last else {
        return true;
    };
    let wait = if last.error.is_some() {
        retry_after
    } else {
        every
    };
    let waited = now_ms.saturating_sub(last.checked_at_ms);
    // A clock set back leaves the last check in the future: ask again.
    waited < 0 || u128::try_from(waited).is_ok_and(|waited| waited >= wait.as_millis())
}

/// The check file's next content: what `result` found, over what `previous` knew.
///
/// A failed check keeps the release found before, so a panel goes on offering it, and a check
/// that finds no stable release keeps what was known.
fn record_of(
    previous: Option<&UpdateCheck>,
    result: Result<Check, String>,
    now_ms: i64,
) -> UpdateCheck {
    let mut record = UpdateCheck {
        checked_at_ms: now_ms,
        latest: previous.and_then(|check| check.latest.clone()),
        release_url: previous.and_then(|check| check.release_url.clone()),
        error: None,
        notified: previous.and_then(|check| check.notified.clone()),
    };
    match result {
        Ok(Check::Newer(release)) => {
            record.latest = Some(release.version.to_string());
            record.release_url = Some(release.page_url);
        }
        Ok(Check::NotNewer {
            latest: Some(release),
        }) => {
            record.latest = Some(release.version.to_string());
            record.release_url = Some(release.page_url);
        }
        Ok(Check::NotNewer { latest: None }) => {}
        Ok(_) => record.error = Some("the lookup found something this allotd does not know".into()),
        Err(error) => record.error = Some(error),
    }
    record
}

/// Runs the checks the schedule asks for until allotd stops.
pub(crate) async fn run(state: AppState) {
    let options = &state.updates.options;
    let mut ticks = time::interval(options.poll);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        let due = state.settings().config.update.check
            && state.started.elapsed() >= options.first_check_after
            && is_due(
                state.updates.memory().last.as_ref(),
                now_ms(),
                options.every,
                options.retry_after,
            );
        if due && state.updates.begin_check() {
            perform(&state).await;
        }
    }
}

/// One check, after [`Updates::begin_check`] took the turn: asks, then keeps the answer in
/// memory and in the file.
async fn perform(state: &AppState) {
    let updates = &state.updates;
    let answer = match &updates.options.updater {
        Ok(updater) => {
            let updater = updater.clone();
            let installed = installed();
            tokio::task::spawn_blocking(move || {
                updater.check(&installed).map_err(|error| error.to_string())
            })
            .await
            .unwrap_or_else(|error| Err(format!("the check stopped: {error}")))
        }
        Err(reason) => Err(reason.clone()),
    };
    match &answer {
        Ok(Check::Newer(release)) => tracing::info!("update check: {} is out", release.version),
        Ok(_) => tracing::info!("update check: this allotd is the newest release"),
        Err(error) => tracing::warn!("update check failed: {error}"),
    }
    let kept = state.clone();
    let stored = tokio::task::spawn_blocking(move || {
        kept.updates
            .store(|previous| Some(record_of(previous, answer, now_ms())))
    })
    .await;
    match stored {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::error!("keeping the update check failed: {error:#}"),
        Err(error) => tracing::error!("keeping the update check stopped: {error}"),
    }
    updates.end_check();
}

fn refuse_when_off(state: &Shared) -> Result<(), AppError> {
    if state.settings().config.update.check {
        return Ok(());
    }
    Err(AppError::Conflict(
        "update checks are off in the settings, so allotd does not contact github.com or start \
         an update; `allot update` still works from a terminal"
            .to_owned(),
    ))
}

/// `POST /update/check`: starts a check now and answers at once with the status, which says
/// `checking` until the next poll shows the result.
pub(crate) async fn check_now(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<UpdateStatus>), AppError> {
    refuse_when_off(&state)?;
    if state.updates.begin_check() {
        let working = state.clone();
        tokio::spawn(async move { perform(&working).await });
    }
    Ok((StatusCode::ACCEPTED, Json(status(&state))))
}

/// `POST /update/start`: starts `allot update` detached for the release the last check found,
/// and answers at once with the status, which says `installing`.
pub(crate) async fn start_update(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<UpdateStatus>), AppError> {
    refuse_when_off(&state)?;
    let _starting = state.updates.starting.lock().await;
    let current = status(&state);
    let version = match (current.state, current.latest) {
        (UpdateState::Installing, _) => {
            return Err(AppError::Conflict(
                "an update is already running".to_owned(),
            ));
        }
        (UpdateState::Available | UpdateState::InstallFailed, Some(latest)) => latest,
        _ => {
            return Err(AppError::Conflict(
                "no newer release is known; check for updates first".to_owned(),
            ));
        }
    };
    let run = state.updates.options.run.clone();
    let started_at_ms = now_ms();
    let launched = tokio::task::spawn_blocking(move || start::launch(&run))
        .await
        .map_err(|error| anyhow!("starting allot update stopped: {error}"))?;
    launched.map_err(|error| match error {
        LaunchError::Missing(_) => AppError::Conflict(error.to_string()),
        other => AppError::Internal(other.into()),
    })?;
    tracing::info!("started allot update for {version}");
    state.updates.memory().launched = Some(Launched {
        at_ms: started_at_ms,
        version,
    });
    Ok((StatusCode::ACCEPTED, Json(status(&state))))
}

/// `POST /update/announced`: a panel raised its notification for a release, so none does again.
pub(crate) async fn announced(
    State(state): State<AppState>,
    body: Result<Json<AnnounceRequest>, JsonRejection>,
) -> Result<Json<UpdateStatus>, AppError> {
    let Json(request) = body.map_err(|rejection| {
        AppError::Validation(format!("not an announcement: {}", rejection.body_text()))
    })?;
    let version = version_of(&request.version).ok_or_else(|| {
        AppError::Validation(format!("{} is not a release version", request.version))
    })?;
    let kept = state.clone();
    let stored = tokio::task::spawn_blocking(move || {
        kept.updates.store(|previous| {
            let mut record = previous?.clone();
            let heard = record.notified.as_deref().and_then(version_of);
            if heard.is_none_or(|heard| heard < version) {
                record.notified = Some(version.to_string());
            }
            Some(record)
        })
    })
    .await
    .map_err(|error| anyhow!("keeping the announcement stopped: {error}"))??;
    if !stored {
        return Err(AppError::Conflict(
            "no check has found a release to announce yet".to_owned(),
        ));
    }
    Ok(Json(status(&state)))
}

#[cfg(test)]
mod tests {
    use allot_updater::Release;

    use super::*;

    const NOW: i64 = 1_792_824_720_000;
    const HOUR: i64 = 60 * 60 * 1_000;

    fn check_at(checked_at_ms: i64, error: Option<&str>) -> UpdateCheck {
        UpdateCheck {
            checked_at_ms,
            latest: Some("0.7.0".into()),
            release_url: Some("https://example.test/tag/v0.7.0".into()),
            error: error.map(Into::into),
            notified: Some("0.7.0".into()),
        }
    }

    fn release(version: &str) -> Release {
        let version = Version::parse(version).unwrap();
        Release {
            tag: format!("v{version}"),
            page_url: format!("https://example.test/tag/v{version}"),
            version,
        }
    }

    fn due(last: Option<&UpdateCheck>, now: i64) -> bool {
        is_due(last, now, CHECK_EVERY, RETRY_AFTER)
    }

    #[test]
    fn test_is_due_without_a_check_is_due() {
        assert!(due(None, NOW));
    }

    #[test]
    fn test_is_due_a_successful_check_waits_a_day() {
        let last = check_at(NOW - 23 * HOUR, None);
        assert!(!due(Some(&last), NOW));
        assert!(due(Some(&last), NOW + 2 * HOUR));
    }

    #[test]
    fn test_is_due_a_failed_check_is_retried_after_an_hour() {
        let last = check_at(NOW - 59 * 60_000, Some("offline"));
        assert!(!due(Some(&last), NOW));
        assert!(due(Some(&last), NOW + 2 * 60_000));
    }

    #[test]
    fn test_is_due_a_check_in_the_future_is_due() {
        let last = check_at(NOW + 5 * HOUR, None);
        assert!(due(Some(&last), NOW));
    }

    #[test]
    fn test_record_of_a_newer_release_keeps_it_and_the_notice_already_sent() {
        let previous = check_at(NOW - 25 * HOUR, None);

        let record = record_of(Some(&previous), Ok(Check::Newer(release("0.8.0"))), NOW);

        assert_eq!(record.checked_at_ms, NOW);
        assert_eq!(record.latest.as_deref(), Some("0.8.0"));
        assert_eq!(
            record.release_url.as_deref(),
            Some("https://example.test/tag/v0.8.0")
        );
        assert_eq!(record.error, None);
        assert_eq!(record.notified.as_deref(), Some("0.7.0"));
    }

    #[test]
    fn test_record_of_a_failure_keeps_the_release_found_before_and_says_why() {
        let previous = check_at(NOW - 25 * HOUR, None);

        let record = record_of(
            Some(&previous),
            Err("could not reach github.com".into()),
            NOW,
        );

        assert_eq!(record.latest.as_deref(), Some("0.7.0"));
        assert_eq!(record.error.as_deref(), Some("could not reach github.com"));
        assert_eq!(record.checked_at_ms, NOW);
    }

    #[test]
    fn test_record_of_no_stable_release_keeps_what_was_known() {
        let previous = check_at(NOW - 25 * HOUR, Some("offline"));

        let record = record_of(Some(&previous), Ok(Check::NotNewer { latest: None }), NOW);

        assert_eq!(record.latest.as_deref(), Some("0.7.0"));
        assert_eq!(record.error, None);
    }

    #[test]
    fn test_record_of_the_first_check_has_nothing_to_keep() {
        let record = record_of(None, Ok(Check::NotNewer { latest: None }), NOW);

        assert_eq!(record.latest, None);
        assert_eq!(record.notified, None);
    }

    #[test]
    fn test_last_error_line_finds_the_error_the_run_ended_with() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("update.log");
        std::fs::write(&log, "looking up\nError: could not reach github.com\n").unwrap();

        assert_eq!(
            last_error_line(&log).as_deref(),
            Some("Error: could not reach github.com")
        );
        assert_eq!(last_error_line(&dir.path().join("missing.log")), None);
    }

    #[test]
    fn test_silent_reason_names_the_log_when_it_holds_no_error() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("update.log");
        std::fs::write(&log, "allot 0.6.1 is up to date\n").unwrap();

        let reason = silent_reason(&log);

        assert!(reason.contains("no record within 90 seconds"), "{reason}");
        assert!(reason.contains("update.log"), "{reason}");
    }
}
