//! What `GET /ui/state` says about updates: the last check, the record `allot update` keeps
//! of its run, and what allotd knows of the run it started, merged into one [`UpdateStatus`].
//!
//! Nothing here reads a file or a clock, so every rule is a plain function of its inputs.

use allot_shared::update::{OutcomeState, UpdateCheck, UpdateOutcome, UpdateState, UpdateStatus};
use allot_updater::Version;

/// How long after allotd started `allot update` it waits for the run to write its record
/// before calling the start a failure: the run takes its lock and looks up the release first.
pub(super) const START_GRACE_MS: i64 = 90_000;

/// A run that says it is installing for longer than this is taken for dead: the longest a
/// Windows setup may take is 20 minutes.
const STALE_RUN_MS: i64 = 2 * 60 * 60 * 1_000;

/// How long a finished run still counts as installing, while `allot install` restarts allotd.
const RESTART_WINDOW_MS: i64 = 2 * 60 * 1_000;

/// A record written this much before allotd started its run still belongs to it: the clocks
/// are the same, so this only covers a rounding of the record's timestamp.
const SLACK_MS: i64 = 2_000;

/// A start allotd made and has not seen a record for yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Launched {
    /// When allotd started the run, in milliseconds since the Unix epoch.
    pub(super) at_ms: i64,
    /// The release the run was started for.
    pub(super) version: String,
}

/// Everything the status is made of.
pub(super) struct Inputs<'a> {
    /// The running allotd's version.
    pub(super) installed: &'a Version,
    /// Whether the setting lets allotd check.
    pub(super) enabled: bool,
    /// Whether a check is running now.
    pub(super) checking: bool,
    /// The last check, from memory or `update-check.json`.
    pub(super) check: Option<&'a UpdateCheck>,
    /// What `allot update` wrote about its last run.
    pub(super) outcome: Option<&'a UpdateOutcome>,
    /// The run allotd started, until a record or a check replaces it.
    pub(super) launched: Option<&'a Launched>,
    /// Whether the process that wrote an `installing` record still runs; `true` when allotd
    /// cannot tell.
    pub(super) run_alive: bool,
    /// The reason to give when a started run never wrote its record.
    pub(super) silent_reason: &'a str,
    /// Now, in milliseconds since the Unix epoch.
    pub(super) now_ms: i64,
}

/// The status the inputs add up to.
pub(super) fn build(inputs: &Inputs) -> UpdateStatus {
    let check = inputs.check;
    let mut status = UpdateStatus {
        installed: inputs.installed.to_string(),
        state: UpdateState::Unchecked,
        latest: check.and_then(|check| check.latest.clone()),
        checked_at_ms: check.map(|check| check.checked_at_ms),
        release_url: check.and_then(|check| check.release_url.clone()),
        reason: None,
        notes: Vec::new(),
        announce: None,
    };
    if let Some(outcome) = inputs.outcome
        && version_of(&outcome.version).as_ref() == Some(inputs.installed)
        && outcome.state == OutcomeState::Succeeded
    {
        status.notes.clone_from(&outcome.notes);
    }
    if let Some((state, target, reason)) = install_state(inputs) {
        status.state = state;
        status.reason = reason;
        if status.latest.as_deref() != Some(target.as_str()) {
            status.release_url = check
                .filter(|check| check.latest.as_deref() == Some(target.as_str()))
                .and_then(|check| check.release_url.clone());
            status.latest = Some(target);
        }
        return status;
    }
    status.state = if !inputs.enabled {
        UpdateState::Disabled
    } else if inputs.checking {
        UpdateState::Checking
    } else {
        from_check(inputs, &mut status)
    };
    status
}

/// The state the last check alone gives, and the release to announce for it.
fn from_check(inputs: &Inputs, status: &mut UpdateStatus) -> UpdateState {
    let Some(check) = inputs.check else {
        return UpdateState::Unchecked;
    };
    let latest = check.latest.as_deref().and_then(version_of);
    if let Some(latest) = latest.filter(|latest| latest > inputs.installed) {
        let heard = check.notified.as_deref().and_then(version_of);
        if heard.is_none_or(|heard| heard < latest) {
            status.announce = Some(latest.to_string());
        }
        // A re-check that failed does not take the offer back.
        return UpdateState::Available;
    }
    match &check.error {
        Some(error) => {
            status.reason = Some(error.clone());
            UpdateState::CheckFailed
        }
        None => UpdateState::UpToDate,
    }
}

/// `Installing` or `InstallFailed`, with the release the run is for and the reason, when the
/// run allotd started or the record `allot update` left says so.
fn install_state(inputs: &Inputs) -> Option<(UpdateState, String, Option<String>)> {
    let record = inputs
        .outcome
        .filter(|outcome| {
            // The record of an earlier run says nothing about the one allotd started since.
            inputs
                .launched
                .is_none_or(|launched| outcome.started_at_ms >= launched.at_ms - SLACK_MS)
        })
        .filter(|outcome| {
            version_of(&outcome.version).is_some_and(|version| &version > inputs.installed)
        });
    let Some(outcome) = record else {
        return launched_state(inputs);
    };
    let version = outcome.version.clone();
    match outcome.state {
        OutcomeState::Installing => {
            let stale = inputs.now_ms - outcome.started_at_ms > STALE_RUN_MS;
            if inputs.run_alive && !stale {
                return Some((UpdateState::Installing, version, None));
            }
            Some((
                UpdateState::InstallFailed,
                version,
                Some("the update did not finish".to_owned()),
            ))
        }
        OutcomeState::Succeeded => {
            let ended = outcome.finished_at_ms.unwrap_or(outcome.started_at_ms);
            (inputs.now_ms - ended <= RESTART_WINDOW_MS).then_some((
                UpdateState::Installing,
                version,
                None,
            ))
        }
        OutcomeState::Failed => {
            let superseded = inputs
                .check
                .and_then(|check| check.latest.as_deref())
                .and_then(version_of)
                .zip(version_of(&outcome.version))
                .is_some_and(|(latest, failed)| latest > failed);
            (!superseded).then(|| (UpdateState::InstallFailed, version, outcome.reason.clone()))
        }
        // A run state a newer `allot update` made up says nothing this allotd can show.
        _ => None,
    }
}

/// The run allotd started, while no record of it exists.
fn launched_state(inputs: &Inputs) -> Option<(UpdateState, String, Option<String>)> {
    let launched = inputs.launched?;
    if inputs.now_ms - launched.at_ms < START_GRACE_MS {
        return Some((UpdateState::Installing, launched.version.clone(), None));
    }
    Some((
        UpdateState::InstallFailed,
        launched.version.clone(),
        Some(inputs.silent_reason.to_owned()),
    ))
}

/// The version in `text`, with or without a leading `v`.
pub(super) fn version_of(text: &str) -> Option<Version> {
    Version::parse(text.strip_prefix('v').unwrap_or(text)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_792_824_720_000;
    const MINUTE: i64 = 60_000;

    fn installed() -> Version {
        Version::new(0, 6, 1)
    }

    fn check(latest: Option<&str>, error: Option<&str>) -> UpdateCheck {
        UpdateCheck {
            checked_at_ms: NOW - 5 * MINUTE,
            latest: latest.map(Into::into),
            release_url: latest.map(|latest| format!("https://example.test/tag/v{latest}")),
            error: error.map(Into::into),
            notified: None,
        }
    }

    fn outcome(state: OutcomeState, version: &str, started_ago_ms: i64) -> UpdateOutcome {
        UpdateOutcome {
            state,
            version: version.into(),
            started_at_ms: NOW - started_ago_ms,
            finished_at_ms: None,
            pid: 4_242,
            reason: None,
            notes: Vec::new(),
        }
    }

    struct Scene {
        installed: Version,
        enabled: bool,
        checking: bool,
        check: Option<UpdateCheck>,
        outcome: Option<UpdateOutcome>,
        launched: Option<Launched>,
        run_alive: bool,
    }

    fn scene() -> Scene {
        Scene {
            installed: installed(),
            enabled: true,
            checking: false,
            check: None,
            outcome: None,
            launched: None,
            run_alive: true,
        }
    }

    impl Scene {
        fn status(&self) -> UpdateStatus {
            build(&Inputs {
                installed: &self.installed,
                enabled: self.enabled,
                checking: self.checking,
                check: self.check.as_ref(),
                outcome: self.outcome.as_ref(),
                launched: self.launched.as_ref(),
                run_alive: self.run_alive,
                silent_reason: "allot update wrote no record within 90 seconds",
                now_ms: NOW,
            })
        }
    }

    #[test]
    fn test_status_before_any_check_is_unchecked() {
        let status = scene().status();

        assert_eq!(status.state, UpdateState::Unchecked);
        assert_eq!(status.installed, "0.6.1");
        assert_eq!(status.latest, None);
        assert_eq!(status.checked_at_ms, None);
    }

    #[test]
    fn test_status_a_newer_release_is_available_with_its_page_and_an_announcement() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), None));

        let status = scene.status();

        assert_eq!(status.state, UpdateState::Available);
        assert_eq!(status.latest.as_deref(), Some("0.7.0"));
        assert_eq!(
            status.release_url.as_deref(),
            Some("https://example.test/tag/v0.7.0")
        );
        assert_eq!(status.checked_at_ms, Some(NOW - 5 * MINUTE));
        assert_eq!(status.announce.as_deref(), Some("0.7.0"));
    }

    #[test]
    fn test_status_a_version_already_announced_is_not_announced_again() {
        let mut scene = scene();
        let mut found = check(Some("0.7.0"), None);
        found.notified = Some("0.7.0".into());
        scene.check = Some(found);

        let status = scene.status();

        assert_eq!(status.state, UpdateState::Available);
        assert_eq!(status.announce, None);
    }

    #[test]
    fn test_status_a_still_newer_release_is_announced_after_an_older_one() {
        let mut scene = scene();
        let mut found = check(Some("0.8.0"), None);
        found.notified = Some("0.7.0".into());
        scene.check = Some(found);

        assert_eq!(scene.status().announce.as_deref(), Some("0.8.0"));
    }

    #[test]
    fn test_status_the_installed_release_is_up_to_date_and_names_the_latest() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.6.1"), None));

        let status = scene.status();

        assert_eq!(status.state, UpdateState::UpToDate);
        assert_eq!(status.latest.as_deref(), Some("0.6.1"));
        assert_eq!(status.announce, None);
    }

    #[test]
    fn test_status_a_build_ahead_of_the_latest_release_is_up_to_date() {
        let mut scene = scene();
        scene.installed = Version::new(0, 7, 0);
        scene.check = Some(check(Some("0.6.1"), None));

        assert_eq!(scene.status().state, UpdateState::UpToDate);
    }

    #[test]
    fn test_status_a_failed_check_reports_why_and_keeps_what_was_known() {
        let mut scene = scene();
        scene.check = Some(check(None, Some("could not reach github.com")));

        let status = scene.status();

        assert_eq!(status.state, UpdateState::CheckFailed);
        assert_eq!(status.reason.as_deref(), Some("could not reach github.com"));
        assert_eq!(status.checked_at_ms, Some(NOW - 5 * MINUTE));
    }

    #[test]
    fn test_status_a_failed_recheck_keeps_offering_a_release_found_before() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), Some("could not reach github.com")));

        let status = scene.status();

        assert_eq!(status.state, UpdateState::Available);
        assert_eq!(status.reason, None);
        assert_eq!(status.latest.as_deref(), Some("0.7.0"));
    }

    #[test]
    fn test_status_a_check_in_progress_is_checking() {
        let mut scene = scene();
        scene.checking = true;
        scene.check = Some(check(Some("0.7.0"), None));

        assert_eq!(scene.status().state, UpdateState::Checking);
    }

    #[test]
    fn test_status_checking_turned_off_is_disabled_and_keeps_the_cache() {
        let mut scene = scene();
        scene.enabled = false;
        scene.check = Some(check(Some("0.7.0"), None));

        let status = scene.status();

        assert_eq!(status.state, UpdateState::Disabled);
        assert_eq!(status.latest.as_deref(), Some("0.7.0"));
        assert_eq!(status.announce, None);
    }

    #[test]
    fn test_status_a_run_that_is_installing_shows_installing_for_its_release() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), None));
        scene.outcome = Some(outcome(OutcomeState::Installing, "0.7.0", MINUTE));

        let status = scene.status();

        assert_eq!(status.state, UpdateState::Installing);
        assert_eq!(status.latest.as_deref(), Some("0.7.0"));
        assert_eq!(
            status.release_url.as_deref(),
            Some("https://example.test/tag/v0.7.0")
        );
        assert_eq!(status.announce, None);
    }

    #[test]
    fn test_status_installing_wins_over_turning_checking_off() {
        let mut scene = scene();
        scene.enabled = false;
        scene.outcome = Some(outcome(OutcomeState::Installing, "0.7.0", MINUTE));

        assert_eq!(scene.status().state, UpdateState::Installing);
    }

    #[test]
    fn test_status_an_installing_record_whose_run_is_gone_is_a_failure() {
        let mut scene = scene();
        scene.run_alive = false;
        scene.outcome = Some(outcome(OutcomeState::Installing, "0.7.0", MINUTE));

        let status = scene.status();

        assert_eq!(status.state, UpdateState::InstallFailed);
        assert_eq!(status.reason.as_deref(), Some("the update did not finish"));
        assert_eq!(status.latest.as_deref(), Some("0.7.0"));
    }

    #[test]
    fn test_status_an_installing_record_hours_old_is_a_failure_even_if_its_pid_runs() {
        let mut scene = scene();
        scene.outcome = Some(outcome(OutcomeState::Installing, "0.7.0", 3 * 60 * MINUTE));

        assert_eq!(scene.status().state, UpdateState::InstallFailed);
    }

    #[test]
    fn test_status_a_failed_record_gives_install_failed_with_its_reason() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), None));
        let mut failed = outcome(OutcomeState::Failed, "0.7.0", 10 * MINUTE);
        failed.reason = Some("the download does not match its checksum".into());
        scene.outcome = Some(failed);

        let status = scene.status();

        assert_eq!(status.state, UpdateState::InstallFailed);
        assert_eq!(
            status.reason.as_deref(),
            Some("the download does not match its checksum")
        );
        assert_eq!(status.latest.as_deref(), Some("0.7.0"));
    }

    #[test]
    fn test_status_a_failed_record_gives_way_to_a_newer_release() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.8.0"), None));
        scene.outcome = Some(outcome(OutcomeState::Failed, "0.7.0", 10 * MINUTE));

        let status = scene.status();

        assert_eq!(status.state, UpdateState::Available);
        assert_eq!(status.latest.as_deref(), Some("0.8.0"));
    }

    #[test]
    fn test_status_a_record_of_the_installed_or_an_older_release_is_history() {
        for version in ["0.6.1", "0.6.0"] {
            let mut scene = scene();
            scene.check = Some(check(Some("0.6.1"), None));
            scene.outcome = Some(outcome(OutcomeState::Failed, version, MINUTE));

            assert_eq!(scene.status().state, UpdateState::UpToDate, "{version}");
        }
    }

    #[test]
    fn test_status_the_notes_of_a_finished_update_reach_the_new_allotd() {
        let mut scene = scene();
        scene.installed = Version::new(0, 7, 0);
        scene.check = Some(check(Some("0.7.0"), None));
        let mut done = outcome(OutcomeState::Succeeded, "0.7.0", 10 * MINUTE);
        done.finished_at_ms = Some(NOW - 9 * MINUTE);
        done.notes = vec!["The GNOME extension loads at your next login.".into()];
        scene.outcome = Some(done);

        let status = scene.status();

        assert_eq!(status.state, UpdateState::UpToDate);
        assert_eq!(
            status.notes,
            ["The GNOME extension loads at your next login."]
        );
    }

    #[test]
    fn test_status_a_finished_update_still_shows_installing_while_allotd_restarts() {
        let mut scene = scene();
        let mut done = outcome(OutcomeState::Succeeded, "0.7.0", MINUTE);
        done.finished_at_ms = Some(NOW - 30_000);
        scene.outcome = Some(done);

        assert_eq!(scene.status().state, UpdateState::Installing);
    }

    #[test]
    fn test_status_a_finished_update_whose_allotd_never_restarted_falls_back_to_the_check() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), None));
        let mut done = outcome(OutcomeState::Succeeded, "0.7.0", 20 * MINUTE);
        done.finished_at_ms = Some(NOW - 19 * MINUTE);
        scene.outcome = Some(done);

        assert_eq!(scene.status().state, UpdateState::Available);
    }

    #[test]
    fn test_status_a_start_without_a_record_yet_is_installing_within_the_grace() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), None));
        scene.launched = Some(Launched {
            at_ms: NOW - 10_000,
            version: "0.7.0".into(),
        });

        assert_eq!(scene.status().state, UpdateState::Installing);
    }

    #[test]
    fn test_status_a_start_that_wrote_no_record_in_time_is_a_failure_with_the_reason() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), None));
        scene.launched = Some(Launched {
            at_ms: NOW - START_GRACE_MS - 1,
            version: "0.7.0".into(),
        });

        let status = scene.status();

        assert_eq!(status.state, UpdateState::InstallFailed);
        assert_eq!(
            status.reason.as_deref(),
            Some("allot update wrote no record within 90 seconds")
        );
        assert_eq!(status.latest.as_deref(), Some("0.7.0"));
    }

    #[test]
    fn test_status_a_new_start_hides_the_failed_record_of_the_run_before_it() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), None));
        scene.outcome = Some(outcome(OutcomeState::Failed, "0.7.0", 3 * 60 * MINUTE));
        scene.launched = Some(Launched {
            at_ms: NOW - 5_000,
            version: "0.7.0".into(),
        });

        assert_eq!(scene.status().state, UpdateState::Installing);
    }

    #[test]
    fn test_status_the_record_of_the_started_run_replaces_the_start() {
        let mut scene = scene();
        scene.check = Some(check(Some("0.7.0"), None));
        scene.launched = Some(Launched {
            at_ms: NOW - 20_000,
            version: "0.7.0".into(),
        });
        let mut failed = outcome(OutcomeState::Failed, "0.7.0", 10_000);
        failed.reason = Some("the download does not match its checksum".into());
        scene.outcome = Some(failed);

        let status = scene.status();

        assert_eq!(status.state, UpdateState::InstallFailed);
        assert_eq!(
            status.reason.as_deref(),
            Some("the download does not match its checksum")
        );
    }

    #[test]
    fn test_version_of_reads_a_leading_v_and_refuses_text() {
        assert_eq!(version_of("v0.7.0"), Some(Version::new(0, 7, 0)));
        assert_eq!(version_of("0.7.0"), Some(Version::new(0, 7, 0)));
        assert_eq!(version_of("latest"), None);
    }
}
