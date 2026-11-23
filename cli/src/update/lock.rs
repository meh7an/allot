//! The lock that keeps two `allot update` runs out of each other's `~/.allot/updates`.

use std::fs::{self, File, OpenOptions, TryLockError};

use allot_shared::paths::Paths;
use anyhow::{Context, bail};

const LOCK_FILE: &str = "update.lock";

/// Held for the length of a run: the lock ends with this value, or with the process.
#[derive(Debug)]
pub(super) struct RunLock {
    _file: File,
}

impl RunLock {
    /// Takes the lock, creating the update folder when it is missing.
    ///
    /// # Errors
    ///
    /// Fails when another `allot update` holds the lock or the file cannot be opened.
    pub(super) fn take(paths: &Paths) -> anyhow::Result<Self> {
        let dir = paths.updates_dir();
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join(LOCK_FILE);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => bail!(
                "another `allot update` is running; it holds {}",
                path.display()
            ),
            Err(TryLockError::Error(error)) => {
                Err(error).with_context(|| format!("locking {}", path.display()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn test_take_refuses_a_second_run_until_the_first_ends() {
        let home = TempDir::new();
        let paths = Paths::under(home.path());

        let first = RunLock::take(&paths).unwrap();
        let message = RunLock::take(&paths).unwrap_err().to_string();
        assert!(message.contains("another `allot update`"), "{message}");

        drop(first);
        assert!(RunLock::take(&paths).is_ok());
    }
}
