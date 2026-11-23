//! The record an `allot update` run keeps in `~/.allot/update-install.json`, for allotd to read.

use std::{path::PathBuf, process};

use allot_shared::{
    paths::Paths,
    update::{OutcomeState, UpdateOutcome},
};
use allot_updater::Version;

use crate::status::now_ms;

/// The run's record: written when it starts, and again when it ends.
#[derive(Debug)]
pub(super) struct Record {
    path: PathBuf,
    outcome: UpdateOutcome,
}

impl Record {
    /// Notes that this run is installing `version`.
    pub(super) fn start(paths: &Paths, version: &Version) -> Self {
        let record = Self {
            path: paths.update_outcome_file(),
            outcome: UpdateOutcome {
                state: OutcomeState::Installing,
                version: version.to_string(),
                started_at_ms: now_ms(),
                finished_at_ms: None,
                pid: process::id(),
                reason: None,
                notes: Vec::new(),
            },
        };
        record.save();
        record
    }

    /// Notes that the install finished, with `notes` for the panels to show.
    pub(super) fn succeeded(mut self, notes: &[String]) {
        self.outcome.state = OutcomeState::Succeeded;
        self.outcome.notes = notes.to_vec();
        self.finish();
    }

    /// Notes that the install did not finish, and why.
    pub(super) fn failed(mut self, reason: String) {
        self.outcome.state = OutcomeState::Failed;
        self.outcome.reason = Some(reason);
        self.finish();
    }

    fn finish(&mut self) {
        self.outcome.finished_at_ms = Some(now_ms());
        self.save();
    }

    /// Reported, not returned: a record that cannot be written is no reason to stop an update.
    fn save(&self) {
        if let Err(error) = self.outcome.write(&self.path) {
            println!("warning: {} was not written ({error})", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::TempDir;

    fn paths(home: &TempDir) -> Paths {
        let paths = Paths::under(home.path());
        fs::create_dir_all(paths.root()).unwrap();
        paths
    }

    fn version() -> Version {
        Version::parse("0.7.0").unwrap()
    }

    #[test]
    fn test_start_records_an_install_in_progress_by_this_process() {
        let home = TempDir::new();
        let paths = paths(&home);

        let _record = Record::start(&paths, &version());

        let outcome = UpdateOutcome::read(&paths.update_outcome_file()).unwrap();
        assert_eq!(outcome.state, OutcomeState::Installing);
        assert_eq!(outcome.version, "0.7.0");
        assert_eq!(outcome.pid, process::id());
        assert_eq!(outcome.finished_at_ms, None);
    }

    #[test]
    fn test_succeeded_keeps_the_notes_and_stamps_the_end() {
        let home = TempDir::new();
        let paths = paths(&home);

        Record::start(&paths, &version()).succeeded(&["loads at your next login".to_owned()]);

        let outcome = UpdateOutcome::read(&paths.update_outcome_file()).unwrap();
        assert_eq!(outcome.state, OutcomeState::Succeeded);
        assert_eq!(outcome.notes, ["loads at your next login"]);
        assert!(outcome.finished_at_ms.unwrap() >= outcome.started_at_ms);
        assert_eq!(outcome.reason, None);
    }

    #[test]
    fn test_failed_keeps_the_reason() {
        let home = TempDir::new();
        let paths = paths(&home);

        Record::start(&paths, &version()).failed("the setup exited with 5".to_owned());

        let outcome = UpdateOutcome::read(&paths.update_outcome_file()).unwrap();
        assert_eq!(outcome.state, OutcomeState::Failed);
        assert_eq!(outcome.reason.as_deref(), Some("the setup exited with 5"));
    }

    #[test]
    fn test_a_record_that_cannot_be_written_does_not_stop_the_run() {
        let home = TempDir::new();
        // No ~/.allot folder: the write fails, and the run goes on.
        let paths = Paths::under(home.path());

        Record::start(&paths, &version()).failed("x".to_owned());

        assert_eq!(UpdateOutcome::read(&paths.update_outcome_file()), None);
    }
}
