//! One-time setup before the daemon serves: the data directory, the single-instance lock, the
//! bearer token, and logging.

use std::{
    fs::{self, DirBuilder, File, OpenOptions, TryLockError},
    io::{self, Write},
    path::Path,
    sync::Mutex,
};

use anyhow::{Context, anyhow, bail};

use crate::logfile::{KEPT_LOG_FILES, MAX_LOG_BYTES, RotatingFile};

const TOKEN_BYTES: usize = 32;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
// Owner-only modes on Unix: anyone who reads the token can act for every session, and the
// database holds every command and directory the sessions ran.
const PRIVATE_DIR_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;

/// Creates the data directory `root` when missing. On Unix only its owner may enter it, and a
/// directory an earlier build left open to others is closed.
///
/// # Errors
///
/// Fails when the directory cannot be created or its permissions cannot be changed.
pub fn create_data_dir(root: &Path) -> anyhow::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(PRIVATE_DIR_MODE);
    }
    builder
        .create(root)
        .with_context(|| format!("creating {}", root.display()))?;
    keep_private(root, PRIVATE_DIR_MODE)
}

/// Holds the daemon's lock file for as long as it lives; dropping it releases the lock.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Takes the exclusive lock on `path`, creating the file when missing.
    ///
    /// # Errors
    ///
    /// Fails when another daemon holds the lock or the file cannot be opened.
    pub fn acquire(path: &Path) -> anyhow::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .with_context(|| format!("opening lock file {}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => {
                bail!(
                    "another allotd is already running (it holds {})",
                    path.display()
                )
            }
            Err(TryLockError::Error(error)) => {
                Err(error).with_context(|| format!("locking {}", path.display()))
            }
        }
    }
}

/// Reads the bearer token from `path`, or generates a random one and writes it there.
///
/// # Errors
///
/// Fails when the file cannot be read or written, or the OS has no randomness to offer.
pub fn load_or_create_token(path: &Path) -> anyhow::Result<String> {
    let token = match fs::read_to_string(path) {
        Ok(token) if !token.trim().is_empty() => token.trim().to_owned(),
        Ok(_) => create_token(path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => create_token(path)?,
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    keep_private(path, PRIVATE_FILE_MODE)?;
    Ok(token)
}

/// Writes a new random token to `path`, readable only by its owner on Unix.
fn create_token(path: &Path) -> anyhow::Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|error| anyhow!("generating the token: {error}"))?;
    let token: String = bytes
        .iter()
        .flat_map(|byte| {
            [
                HEX_DIGITS[usize::from(byte >> 4)],
                HEX_DIGITS[usize::from(byte & 0x0f)],
            ]
        })
        .map(char::from)
        .collect();
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(PRIVATE_FILE_MODE);
    }
    options
        .open(path)
        .and_then(|mut file| file.write_all(token.as_bytes()))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(token)
}

/// Sets `path` to `mode` when its group or others have any access to it.
#[cfg(unix)]
fn keep_private(path: &Path, mode: u32) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("reading the permissions of {}", path.display()))?
        .permissions();
    if permissions.mode() & 0o077 == 0 {
        return Ok(());
    }
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("restricting {} to its owner", path.display()))
}

/// The user profile is private on Windows already.
#[cfg(not(unix))]
fn keep_private(_path: &Path, _mode: u32) -> anyhow::Result<()> {
    Ok(())
}

/// Sends `tracing` output at INFO and above to `allotd.log` in `log_dir`, which rotates at
/// [`MAX_LOG_BYTES`] and keeps [`KEPT_LOG_FILES`] files.
///
/// # Errors
///
/// Fails when the log file cannot be opened or a global subscriber is already set.
pub fn init_logging(log_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(log_dir).with_context(|| format!("creating {}", log_dir.display()))?;
    let file = RotatingFile::open(log_dir, "allotd", MAX_LOG_BYTES, KEPT_LOG_FILES)
        .with_context(|| format!("opening allotd.log in {}", log_dir.display()))?;
    tracing_subscriber::fmt()
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .map_err(|error| anyhow!("initializing logging: {error}"))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_instance_lock_second_acquire_refuses_until_first_is_dropped() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("allotd.lock");
        let first = InstanceLock::acquire(&path).unwrap();

        let second = InstanceLock::acquire(&path).unwrap_err();
        assert!(second.to_string().contains("already running"), "{second}");

        drop(first);
        InstanceLock::acquire(&path).unwrap();
    }

    #[test]
    fn test_token_is_generated_once_then_reused() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("token");
        let token = load_or_create_token(&path).unwrap();
        assert_eq!(token.len(), TOKEN_BYTES * 2);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(load_or_create_token(&path).unwrap(), token);
    }

    #[test]
    fn test_token_blank_file_is_replaced() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("token");
        fs::write(&path, "  \n").unwrap();
        assert_eq!(load_or_create_token(&path).unwrap().len(), TOKEN_BYTES * 2);
    }

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;

        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    fn set_mode(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_create_data_dir_new_dir_and_token_are_owner_only() {
        let home = TempDir::new().unwrap();
        let root = home.path().join(".allot");
        create_data_dir(&root).unwrap();
        let token = root.join("token");
        load_or_create_token(&token).unwrap();
        assert_eq!((mode(&root), mode(&token)), (0o700, 0o600));
    }

    #[cfg(unix)]
    #[test]
    fn test_create_data_dir_open_dir_and_token_are_closed() {
        let home = TempDir::new().unwrap();
        let root = home.path().join(".allot");
        fs::create_dir(&root).unwrap();
        set_mode(&root, 0o755);
        let token = root.join("token");
        fs::write(&token, "0123abcd").unwrap();
        set_mode(&token, 0o644);

        create_data_dir(&root).unwrap();
        assert_eq!(load_or_create_token(&token).unwrap(), "0123abcd");
        assert_eq!((mode(&root), mode(&token)), (0o700, 0o600));
    }

    #[cfg(unix)]
    #[test]
    fn test_create_data_dir_private_modes_are_left_alone() {
        let home = TempDir::new().unwrap();
        let root = home.path().join(".allot");
        fs::create_dir(&root).unwrap();
        set_mode(&root, 0o500);
        create_data_dir(&root).unwrap();
        assert_eq!(mode(&root), 0o500);
    }
}
