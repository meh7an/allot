//! The stop record: a file in the data folder saying the user stopped allotd on purpose. While
//! it exists, sessions do not start allotd again; any start of allotd removes it, so Start in
//! a panel, `allot daemon start` and the next sign-in all end it.
//!
//! The panels read it while allotd is down, to tell "stopped by you" from "not answering", so
//! its JSON is part of the contract.

use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{Result, atomic_file};

/// Where the stop came from.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StoppedBy {
    /// `allot daemon stop`.
    Cli,
    /// Stop service in a panel.
    Panel,
}

impl StoppedBy {
    /// How the user stopped it, for sentences such as "you stopped it {how}".
    pub fn how(self) -> &'static str {
        match self {
            Self::Cli => "with `allot daemon stop`",
            Self::Panel => "from the panel",
        }
    }
}

/// The JSON in `~/.allot/stopped.json`.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopRecord {
    /// When allotd was stopped, in Unix milliseconds.
    pub stopped_at_ms: i64,
    /// Who stopped it.
    pub by: StoppedBy,
}

impl StopRecord {
    /// A record of a stop by `by` made now.
    pub fn now(by: StoppedBy) -> Self {
        Self {
            stopped_at_ms: unix_ms(SystemTime::now()),
            by,
        }
    }

    /// The record at `path`, or `None` when there is none. A file that exists but does not
    /// hold a record still keeps allotd stopped, since only its presence counts; it reads as a
    /// stop from the CLI, the only writer that existed before panels, at the file's time.
    pub fn read(path: &Path) -> Option<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
            Err(_) => String::new(),
        };
        serde_json::from_str(&text).ok().or_else(|| {
            let modified = fs::metadata(path).and_then(|metadata| metadata.modified());
            Some(Self {
                stopped_at_ms: modified.map(unix_ms).unwrap_or_default(),
                by: StoppedBy::Cli,
            })
        })
    }

    /// Writes the record to `path` through a temporary file in the same folder, so a reader
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

    /// Removes the record at `path`. Returns whether there was one.
    ///
    /// # Errors
    ///
    /// Fails when the file exists but cannot be removed.
    pub fn clear(path: &Path) -> Result<bool> {
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

fn unix_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_stop_record_write_then_read_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("stopped.json");
        let record = StopRecord {
            stopped_at_ms: 1_792_824_720_000,
            by: StoppedBy::Panel,
        };

        record.write(&path).unwrap();

        assert_eq!(StopRecord::read(&path), Some(record));
        assert!(!dir.path().join("stopped.json.tmp").exists());
    }

    #[test]
    fn test_stop_record_json_uses_snake_case_names() {
        let record = StopRecord {
            stopped_at_ms: 5,
            by: StoppedBy::Cli,
        };
        assert_eq!(
            serde_json::to_string(&record).unwrap(),
            r#"{"stopped_at_ms":5,"by":"cli"}"#
        );
    }

    #[test]
    fn test_stop_record_read_missing_file_is_none() {
        let dir = TempDir::new().unwrap();
        assert_eq!(StopRecord::read(&dir.path().join("stopped.json")), None);
    }

    #[test]
    fn test_stop_record_read_garbled_file_still_counts_as_a_stop() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("stopped.json");
        fs::write(&path, "not json").unwrap();

        let record = StopRecord::read(&path).unwrap();

        assert_eq!(record.by, StoppedBy::Cli);
        assert!(record.stopped_at_ms > 0);
    }

    #[test]
    fn test_stop_record_write_replaces_an_older_record() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("stopped.json");
        StopRecord::now(StoppedBy::Panel).write(&path).unwrap();
        let newer = StopRecord {
            stopped_at_ms: i64::MAX,
            by: StoppedBy::Cli,
        };

        newer.write(&path).unwrap();

        assert_eq!(StopRecord::read(&path), Some(newer));
    }

    #[test]
    fn test_stop_record_clear_reports_whether_one_existed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("stopped.json");
        StopRecord::now(StoppedBy::Cli).write(&path).unwrap();

        assert!(StopRecord::clear(&path).unwrap());
        assert!(!path.exists());
        assert!(!StopRecord::clear(&path).unwrap());
    }

    #[test]
    fn test_stop_record_write_into_a_missing_folder_fails() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("missing").join("stopped.json");
        assert!(StopRecord::now(StoppedBy::Cli).write(&path).is_err());
    }
}
