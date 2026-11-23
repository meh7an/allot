//! What allotd knows about newer releases, in the two shapes it takes: [`UpdateStatus`], the part
//! of `GET /ui/state` every panel reads, and [`UpdateCheck`], the file that keeps the last check
//! across allotd's restarts.
//!
//! Every enum here is part of the contract, and the generated decoders reject a value they do not
//! know, which would drop a whole poll in a panel that predates it. A new state or kind therefore
//! needs a new field instead, never a new variant.

use std::{fs, io, path::Path};

use serde::{Deserialize, Serialize};

use crate::{Result, atomic_file};

/// Where the update stands.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UpdateState {
    /// No check has finished since allotd first ran, so nothing is known about newer releases.
    Unchecked,
    /// A check is running now.
    Checking,
    /// The installed version is the newest release the last check found.
    UpToDate,
    /// A newer release is out; `latest` and `release_url` name it.
    Available,
    /// `allot update` is installing `latest`.
    Installing,
    /// The last check could not tell whether a newer release exists; `reason` says why.
    CheckFailed,
    /// The last install did not finish; `reason` says why. `latest` still names the newer
    /// release, so a panel can offer to try again.
    InstallFailed,
    /// The user turned checking off, so nothing contacts github.com.
    Disabled,
}

/// What `GET /ui/state` says about updates, in its `update` field.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// Version of the running allotd, for example `0.6.1`.
    pub installed: String,
    /// What the update is doing or has found.
    pub state: UpdateState,
    /// The newest release any check has found, without the leading `v`; `None` until one has.
    /// A failed check keeps it, so a panel goes on offering the update it knew of.
    pub latest: Option<String>,
    /// When the last check finished, successful or not, in milliseconds since the Unix epoch;
    /// `None` before the first.
    pub checked_at_ms: Option<i64>,
    /// The page of `latest` on GitHub.
    pub release_url: Option<String>,
    /// Why the last check or install failed; `None` in every other state.
    pub reason: Option<String>,
    /// Lines for a panel to show under its banner as given, for example that a panel loads its
    /// new version at the next login. `allot update` writes them; empty when there is nothing to
    /// say.
    pub notes: Vec<String>,
}

/// The JSON in `~/.allot/update-check.json`: the last check of GitHub's releases, so a restart
/// does not ask again and one notice goes out per version. Only allotd writes it.
///
/// Readers ignore keys they do not know, unlike `config.toml`, so a file a newer allotd wrote
/// still reads after a downgrade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCheck {
    /// When the last check finished, successful or not, in milliseconds since the Unix epoch.
    pub checked_at_ms: i64,
    /// The newest release any check has found, without the leading `v`. A failed check leaves
    /// it as it was.
    pub latest: Option<String>,
    /// The page of `latest` on GitHub.
    pub release_url: Option<String>,
    /// Why the last check failed; `None` when it succeeded.
    pub error: Option<String>,
    /// The newest version a notice went out for, so no restart repeats it.
    #[serde(default)]
    pub notified: Option<String>,
}

impl UpdateCheck {
    /// The check stored at `path`, or `None` when there is none or it cannot be read, which
    /// only costs one more check.
    pub fn read(path: &Path) -> Option<Self> {
        let text = fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Writes the check to `path` through a temporary file in the same folder, so a reader
    /// never sees half of it.
    ///
    /// # Errors
    ///
    /// Fails when the folder is missing or the file cannot be written or renamed.
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        json.push('\n');
        atomic_file::replace(path, json.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn check() -> UpdateCheck {
        UpdateCheck {
            checked_at_ms: 1_792_824_720_000,
            latest: Some("0.7.0".into()),
            release_url: Some("https://github.com/meh7an/allot/releases/tag/v0.7.0".into()),
            error: None,
            notified: Some("0.7.0".into()),
        }
    }

    #[test]
    fn test_update_state_json_uses_snake_case_names() {
        let names = [
            (UpdateState::Unchecked, "unchecked"),
            (UpdateState::Checking, "checking"),
            (UpdateState::UpToDate, "up_to_date"),
            (UpdateState::Available, "available"),
            (UpdateState::Installing, "installing"),
            (UpdateState::CheckFailed, "check_failed"),
            (UpdateState::InstallFailed, "install_failed"),
            (UpdateState::Disabled, "disabled"),
        ];
        for (state, name) in names {
            let json = serde_json::to_string(&state).unwrap();
            assert_eq!(json, format!("\"{name}\""));
            assert_eq!(serde_json::from_str::<UpdateState>(&json).unwrap(), state);
        }
    }

    #[test]
    fn test_update_status_json_round_trips_with_every_field_set() {
        let status = UpdateStatus {
            installed: "0.6.1".into(),
            state: UpdateState::InstallFailed,
            latest: Some("0.7.0".into()),
            checked_at_ms: Some(1_792_824_720_000),
            release_url: Some("https://github.com/meh7an/allot/releases/tag/v0.7.0".into()),
            reason: Some("the setup exited with code 5".into()),
            notes: vec!["The GNOME panel loads at your next login.".into()],
        };
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(serde_json::from_str::<UpdateStatus>(&json).unwrap(), status);
    }

    #[test]
    fn test_update_check_write_then_read_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("update-check.json");

        check().write(&path).unwrap();

        assert_eq!(UpdateCheck::read(&path), Some(check()));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn test_update_check_json_uses_these_key_names() {
        let json = serde_json::to_string(&check()).unwrap();
        assert_eq!(
            json,
            concat!(
                r#"{"checked_at_ms":1792824720000,"latest":"0.7.0","#,
                r#""release_url":"https://github.com/meh7an/allot/releases/tag/v0.7.0","#,
                r#""error":null,"notified":"0.7.0"}"#
            )
        );
    }

    #[test]
    fn test_update_check_read_missing_file_is_none() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            UpdateCheck::read(&dir.path().join("update-check.json")),
            None
        );
    }

    #[test]
    fn test_update_check_read_garbled_file_is_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("update-check.json");
        fs::write(&path, "not json").unwrap();
        assert_eq!(UpdateCheck::read(&path), None);
    }

    #[test]
    fn test_update_check_read_ignores_keys_a_newer_allotd_wrote() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("update-check.json");
        fs::write(
            &path,
            r#"{"checked_at_ms":5,"latest":null,"release_url":null,"error":"offline",
                "notified":null,"next_check_at_ms":9}"#,
        )
        .unwrap();

        let read = UpdateCheck::read(&path).unwrap();

        assert_eq!(read.checked_at_ms, 5);
        assert_eq!(read.error.as_deref(), Some("offline"));
    }

    #[test]
    fn test_update_check_read_file_without_notified_key_reads_as_never_notified() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("update-check.json");
        fs::write(
            &path,
            r#"{"checked_at_ms":5,"latest":"0.7.0","release_url":null,"error":null}"#,
        )
        .unwrap();

        assert_eq!(UpdateCheck::read(&path).unwrap().notified, None);
    }

    #[test]
    fn test_update_check_write_replaces_an_older_check() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("update-check.json");
        check().write(&path).unwrap();
        let newer = UpdateCheck {
            checked_at_ms: i64::MAX,
            latest: None,
            error: Some("could not reach github.com".into()),
            ..check()
        };

        newer.write(&path).unwrap();

        assert_eq!(UpdateCheck::read(&path), Some(newer));
    }

    #[test]
    fn test_update_check_write_into_a_missing_folder_fails() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("missing").join("update-check.json");
        assert!(check().write(&path).is_err());
    }
}
