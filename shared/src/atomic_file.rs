//! Replacing a file at once, so readers see the old or the new file and never a part of it.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU32, Ordering},
};

/// Replaces `path` with `bytes` through a temporary file beside it, flushed to disk before it is
/// renamed over `path`. A failed write leaves the old file whole and removes the temporary one.
/// On Unix the new file keeps the mode of the file it replaces.
///
/// # Errors
///
/// The I/O error when the temporary file cannot be written or renamed.
pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    replace_if(path, bytes, |_| true).map(drop)
}

/// [`replace`], unless `unchanged` rejects what `path` holds right before the rename (`None`
/// when it does not exist): another program saved the file since its caller read it. Returns
/// whether it replaced the file; a rejected replace leaves the file as that program wrote it.
///
/// # Errors
///
/// The I/O error when the file cannot be read, or the temporary file written or renamed.
pub fn replace_if<F>(path: &Path, bytes: &[u8], unchanged: F) -> io::Result<bool>
where
    F: FnOnce(Option<&[u8]>) -> bool,
{
    let temp = temp_path(path);
    let replaced = write_synced(&temp, bytes, path).and_then(|()| {
        let current = match fs::read(path) {
            Ok(current) => Some(current),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        if !unchanged(current.as_deref()) {
            return Ok(false);
        }
        fs::rename(&temp, path).map(|()| true)
    });
    if !matches!(replaced, Ok(true)) {
        let _ = fs::remove_file(&temp);
    }
    if replaced? {
        sync_directory(path);
        return Ok(true);
    }
    Ok(false)
}

/// Writes the temporary file and flushes it to disk, so the rename never exposes an empty
/// file after a crash. On Unix it takes the mode of the file it replaces; created with the
/// umask, it could otherwise turn a private file readable to others.
pub(crate) fn write_synced(temp: &Path, bytes: &[u8], replaced: &Path) -> io::Result<()> {
    let mut file = File::create_new(temp)?;
    #[cfg(unix)]
    if let Ok(metadata) = fs::metadata(replaced) {
        file.set_permissions(metadata.permissions())?;
    }
    #[cfg(not(unix))]
    let _ = replaced;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Makes the rename itself durable on Unix. Best effort: the file is already replaced, and a
/// failure here only matters after a power loss.
pub(crate) fn sync_directory(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        let _ = File::open(dir).and_then(|dir| dir.sync_all());
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// A temporary name beside `path` that no other write, in this process or another, uses.
pub(crate) fn temp_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{}-{serial}.tmp", process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_writes_the_file_and_leaves_no_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        replace(&path, b"{}\n").unwrap();
        replace(&path, b"{\"a\": 1}\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"{\"a\": 1}\n");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn test_replace_if_leaves_a_file_another_program_changed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        fs::write(&path, "theirs").unwrap();
        let replaced = replace_if(&path, b"ours", |current| {
            current == Some(b"read".as_slice())
        });
        assert!(!replaced.unwrap());
        assert_eq!(fs::read(&path).unwrap(), b"theirs");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(
            replace_if(&path, b"ours", |current| current
                == Some(b"theirs".as_slice()))
            .unwrap()
        );
        assert_eq!(fs::read(&path).unwrap(), b"ours");
    }

    #[cfg(unix)]
    #[test]
    fn test_replace_keeps_a_private_files_mode() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        replace(&path, b"{\"mcpServers\": {}}").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
