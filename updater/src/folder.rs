use std::{fs, io, path::PathBuf};

use allot_shared::paths::Paths;
use semver::Version;

use crate::error::{Result, UpdateError};

/// The folder `~/.allot/updates/<version>/`, which holds one release's downloaded files and
/// what was unpacked from them.
pub fn of(paths: &Paths, version: &Version) -> PathBuf {
    paths.updates_dir().join(version.to_string())
}

/// Removes the update folders of every version but `keep`, and returns the versions removed,
/// oldest first.
///
/// Only folders named like a version are touched; anything else in `~/.allot/updates` stays.
///
/// # Errors
///
/// Fails when the update folder cannot be read or a version's folder cannot be removed.
pub fn clean(paths: &Paths, keep: &Version) -> Result<Vec<Version>> {
    let updates = paths.updates_dir();
    let entries = match fs::read_dir(&updates) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(UpdateError::io("read", &updates, error)),
    };
    let mut removed = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| UpdateError::io("read", &updates, error))?;
        let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
        let version = entry
            .file_name()
            .to_str()
            .and_then(|name| Version::parse(name).ok());
        if let (true, Some(version)) = (is_folder, version)
            && version != *keep
        {
            let path = entry.path();
            fs::remove_dir_all(&path).map_err(|error| UpdateError::io("remove", &path, error))?;
            removed.push(version);
        }
    }
    removed.sort();
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn make(paths: &Paths, name: &str) {
        let folder = paths.updates_dir().join(name);
        fs::create_dir_all(folder.join("inner")).unwrap();
        fs::write(folder.join("inner").join("file"), b"x").unwrap();
    }

    #[test]
    fn test_of_is_a_folder_per_version_under_updates() {
        let paths = Paths::under("home");
        assert_eq!(
            of(&paths, &version("0.7.0")),
            paths.root().join("updates").join("0.7.0")
        );
    }

    #[test]
    fn test_clean_keeps_one_version_and_strangers() {
        let home = TempDir::new().unwrap();
        let paths = Paths::under(home.path());
        for name in ["0.6.0", "0.6.1", "0.7.0", "0.7.0-beta.1", "notes"] {
            make(&paths, name);
        }
        fs::write(paths.updates_dir().join("0.5.0"), b"a file, not a folder").unwrap();

        let removed = clean(&paths, &version("0.7.0")).unwrap();

        assert_eq!(
            removed,
            [version("0.6.0"), version("0.6.1"), version("0.7.0-beta.1")]
        );
        let mut left: Vec<String> = fs::read_dir(paths.updates_dir())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["0.5.0", "0.7.0", "notes"]);
    }

    #[test]
    fn test_clean_without_an_updates_folder_removes_nothing() {
        let home = TempDir::new().unwrap();
        let paths = Paths::under(home.path());
        assert_eq!(clean(&paths, &version("0.7.0")).unwrap(), []);
        assert!(!paths.updates_dir().exists());
    }
}
