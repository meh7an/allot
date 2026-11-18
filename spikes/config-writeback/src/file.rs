//! Reading `config.toml` with a revision and writing it back atomically.

use std::{
    error, fmt,
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU32, Ordering},
    thread,
    time::{Duration, Instant},
};

/// Identifies one version of the file's bytes, so a write can tell whether someone saved the
/// file since it was read. A missing file has the revision of an empty one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revision(u64);

impl Revision {
    /// The revision of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        Self(hasher.finish())
    }
}

/// Why a write did not happen.
#[derive(Debug)]
pub enum WriteError {
    /// The file changed since it was read; it was left as it is.
    Conflict {
        /// The revision on disk now.
        current: Revision,
    },
    /// Reading, writing or replacing the file failed.
    Io(io::Error),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { .. } => f.write_str("the file changed since it was read"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl error::Error for WriteError {}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// What a successful write did.
#[derive(Debug, Clone, Copy)]
pub struct Written {
    /// The revision of the new contents.
    pub revision: Revision,
    /// Time from the last check for a hand edit to the file being replaced: an edit saved in
    /// this window is overwritten without notice.
    pub race_window: Duration,
    /// Replace attempts made, 1 when the first one succeeded.
    pub attempts: u32,
}

/// How often a replace is tried again while another program holds the file.
#[derive(Debug, Clone, Copy)]
pub struct Retry {
    /// Attempts in total, at least 1.
    pub attempts: u32,
    /// Pause between attempts.
    pub delay: Duration,
}

impl Retry {
    /// A single attempt.
    pub const NONE: Self = Self {
        attempts: 1,
        delay: Duration::ZERO,
    };
}

/// Reads the file and its revision; a missing file reads as empty.
///
/// # Errors
///
/// Returns the I/O error when the file exists but cannot be read.
pub fn read(path: &Path) -> io::Result<(String, Revision)> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let revision = Revision::of(&bytes);
    let text = String::from_utf8(bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok((text, revision))
}

/// Replaces the file with `text` when it is still at `expected`, through a temporary file in
/// the same directory and a rename, so readers see the old or the new file and never a part.
///
/// # Errors
///
/// [`WriteError::Conflict`] when the file no longer has revision `expected`, and
/// [`WriteError::Io`] when the temporary file cannot be written or the rename fails on every
/// attempt. The temporary file is removed on failure.
pub fn write(
    path: &Path,
    text: &str,
    expected: Revision,
    retry: Retry,
) -> Result<Written, WriteError> {
    let temp = temp_path(path);
    if let Err(error) = write_synced(&temp, text.as_bytes()) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    let replaced = replace(path, &temp, expected, retry);
    if replaced.is_err() {
        let _ = fs::remove_file(&temp);
    }
    let (race_window, attempts) = replaced?;
    Ok(Written {
        revision: Revision::of(text.as_bytes()),
        race_window,
        attempts,
    })
}

/// Whether `error` means another program has the file open in a way that forbids replacing
/// it: a sharing violation or access denied on Windows.
pub fn is_held(error: &io::Error) -> bool {
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    cfg!(windows)
        && matches!(
            error.raw_os_error(),
            Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
        )
}

fn replace(
    path: &Path,
    temp: &Path,
    expected: Revision,
    retry: Retry,
) -> Result<(Duration, u32), WriteError> {
    let mut attempt = 1;
    loop {
        let (_, current) = read(path)?;
        if current != expected {
            return Err(WriteError::Conflict { current });
        }
        let checked = Instant::now();
        match fs::rename(temp, path) {
            Ok(()) => return Ok((checked.elapsed(), attempt)),
            Err(error) if is_held(&error) && attempt < retry.attempts.max(1) => {
                attempt += 1;
                thread::sleep(retry.delay);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn temp_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{}-{serial}.tmp", process::id()));
    path.with_file_name(name)
}
