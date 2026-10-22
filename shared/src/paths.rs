use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{Error, Result};

const APP_DIR: &str = "resources-helper";

/// Locations of the per-user files under `%LOCALAPPDATA%\resources-helper\`.
///
/// Nothing is created here; the daemon creates the directory on first start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// Resolves the paths from the `LOCALAPPDATA` environment variable.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when `LOCALAPPDATA` is unset or empty.
    pub fn from_env() -> Result<Self> {
        Self::from_local_app_data(env::var_os("LOCALAPPDATA"))
    }

    fn from_local_app_data(base: Option<OsString>) -> Result<Self> {
        match base {
            Some(base) if !base.is_empty() => Ok(Self::under(base)),
            _ => Err(Error::Config("LOCALAPPDATA is not set".into())),
        }
    }

    /// Paths rooted at `<base>\resources-helper`, for tests and non-default installs.
    pub fn under(base: impl Into<PathBuf>) -> Self {
        Self {
            root: base.into().join(APP_DIR),
        }
    }

    /// The `resources-helper` directory itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The TOML config file.
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// The bearer token clients send to the daemon.
    pub fn token_file(&self) -> PathBuf {
        self.root.join("token")
    }

    /// The SQLite database.
    pub fn database_file(&self) -> PathBuf {
        self.root.join("rh.db")
    }

    /// The file the running daemon keeps locked, so a second one refuses to start.
    pub fn lock_file(&self) -> PathBuf {
        self.root.join("rhd.lock")
    }

    /// Directory for the daemon's log files.
    pub fn log_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_from_local_app_data_roots_under_app_dir() {
        let base = PathBuf::from(r"C:\Users\someone\AppData\Local");
        let paths = Paths::from_local_app_data(Some(base.clone().into_os_string())).unwrap();
        let root = base.join("resources-helper");
        assert_eq!(paths.root(), root);
        assert_eq!(paths.config_file(), root.join("config.toml"));
        assert_eq!(paths.token_file(), root.join("token"));
        assert_eq!(paths.database_file(), root.join("rh.db"));
        assert_eq!(paths.log_dir(), root.join("logs"));
        assert_eq!(paths.lock_file(), root.join("rhd.lock"));
    }

    #[test]
    fn test_paths_from_local_app_data_unset_or_empty_is_config_error() {
        for base in [None, Some(OsString::new())] {
            let result = Paths::from_local_app_data(base);
            assert!(
                matches!(result, Err(Error::Config(message)) if message.contains("LOCALAPPDATA"))
            );
        }
    }

    #[test]
    fn test_paths_from_env_matches_local_app_data() {
        if let Some(base) = env::var_os("LOCALAPPDATA").filter(|base| !base.is_empty()) {
            assert_eq!(Paths::from_env().unwrap(), Paths::under(base));
        }
    }
}
