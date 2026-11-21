//! Reading `config.toml` with a revision and replacing it atomically.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::Duration,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::atomic_file::{sync_directory, temp_path, write_synced};

/// Identifies one version of the file's bytes, so a write can tell whether someone saved the
/// file since it was read. A missing file has the revision of an empty one.
///
/// A 64-bit FNV-1a hash, stable across builds, so a revision a panel holds stays comparable
/// after allotd is updated. Written as 16 hex digits, since JSON numbers lose 64-bit precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Revision(u64);

impl Revision {
    /// The revision of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let hash = bytes.iter().fold(OFFSET_BASIS, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(PRIME)
        });
        Self(hash)
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// A revision string that is not 16 hex digits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a revision is 16 hex digits")]
pub struct ParseRevisionError;

impl FromStr for Revision {
    type Err = ParseRevisionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 16 {
            return Err(ParseRevisionError);
        }
        u64::from_str_radix(text, 16)
            .map(Self)
            .map_err(|_| ParseRevisionError)
    }
}

impl Serialize for Revision {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Revision {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

#[cfg(any(feature = "schema", test))]
impl schemars::JsonSchema for Revision {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Revision".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "A version of `config.toml`'s bytes, as 16 hex digits.",
            "type": "string",
        })
    }
}

/// How often a replace is tried again while another program holds the file.
#[derive(Debug, Clone, Copy)]
pub(super) struct Retry {
    /// Attempts in total, at least 1.
    pub(super) attempts: u32,
    /// Pause between attempts.
    pub(super) delay: Duration,
}

/// About a second: enough for an editor's save or a virus scan to let go, short enough that a
/// panel's Apply does not seem to hang.
pub(super) const HELD_RETRY: Retry = Retry {
    attempts: 40,
    delay: Duration::from_millis(25),
};

/// Why the file was not replaced.
#[derive(Debug)]
pub(super) enum ReplaceError {
    /// The file changed since it was read; it was left as it is.
    Conflict { current: Revision },
    /// Another program kept the file open in a way that blocks replacing it, through every
    /// attempt.
    Held(io::Error),
    /// Reading, writing or replacing the file failed.
    Io(io::Error),
}

impl From<io::Error> for ReplaceError {
    fn from(error: io::Error) -> Self {
        if is_held(&error) {
            Self::Held(error)
        } else {
            Self::Io(error)
        }
    }
}

/// The file `path` stands for: the target of a symbolic link, followed to the end, so a
/// write replaces the target and keeps the link. A link to a missing file gives that file.
///
/// # Errors
///
/// The I/O error when a link cannot be read, or links loop.
pub(super) fn resolve(path: &Path) -> io::Result<PathBuf> {
    const MAX_LINKS: usize = 40;
    let mut path = path.to_owned();
    for _ in 0..MAX_LINKS {
        let is_link = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata.file_type().is_symlink(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        if !is_link {
            return Ok(path);
        }
        let link = fs::read_link(&path)?;
        path = match path.parent() {
            Some(parent) if link.is_relative() => parent.join(link),
            _ => link,
        };
    }
    Err(io::Error::other("too many levels of symbolic links"))
}

/// Reads the bytes of `path`; a missing file reads as empty.
///
/// # Errors
///
/// The I/O error when the file exists but cannot be read.
pub(super) fn read_bytes(path: &Path) -> io::Result<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Replaces `path` with `text` when it is still at `expected`, through a temporary file in
/// the same directory and a rename, so readers see the old or the new file and never a part.
/// `path` must already be resolved with [`resolve`].
///
/// # Errors
///
/// [`ReplaceError::Conflict`] when the file no longer has revision `expected`,
/// [`ReplaceError::Held`] when another program blocked the replace on every attempt, and
/// [`ReplaceError::Io`] for any other failure. The temporary file is removed on failure.
pub(super) fn replace(
    path: &Path,
    text: &str,
    expected: Revision,
    retry: Retry,
) -> Result<(), ReplaceError> {
    refuse_read_only(path)?;
    let temp = temp_path(path);
    if let Err(error) = write_synced(&temp, text.as_bytes(), path) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    let replaced = rename_unchanged(&temp, path, expected, retry);
    if replaced.is_err() {
        let _ = fs::remove_file(&temp);
    }
    replaced?;
    sync_directory(path);
    Ok(())
}

/// Whether `error` means another program has the file open in a way that forbids reading or
/// replacing it: a sharing violation or access denied on Windows.
fn is_held(error: &io::Error) -> bool {
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    cfg!(windows)
        && matches!(
            error.raw_os_error(),
            Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
        )
}

/// On Windows a read-only file cannot be replaced, and the rename would fail as access denied,
/// the same error as a file held open; say what is wrong instead of retrying.
fn refuse_read_only(path: &Path) -> io::Result<()> {
    if cfg!(windows) && fs::metadata(path).is_ok_and(|metadata| metadata.permissions().readonly()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is read-only", path.display()),
        ));
    }
    Ok(())
}

/// The check for a hand edit and the rename, retried while another program holds the file.
/// The check comes right before each rename, so an edit saved meanwhile is not overwritten
/// unless it lands within the fraction of a millisecond between the two.
fn rename_unchanged(
    temp: &Path,
    path: &Path,
    expected: Revision,
    retry: Retry,
) -> Result<(), ReplaceError> {
    let mut attempt = 1;
    loop {
        let outcome = read_bytes(path).and_then(|bytes| {
            let current = Revision::of(&bytes);
            if current != expected {
                return Ok(Err(ReplaceError::Conflict { current }));
            }
            fs::rename(temp, path).map(Ok)
        });
        match outcome {
            Ok(result) => return result,
            Err(error) if is_held(&error) && attempt < retry.attempts.max(1) => {
                attempt += 1;
                thread::sleep(retry.delay);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revision_is_stable_and_round_trips_as_hex() {
        // FNV-1a's published value for the empty input; a change here breaks held revisions.
        assert_eq!(Revision::of(b"").to_string(), "cbf29ce484222325");
        assert_ne!(Revision::of(b"a"), Revision::of(b"b"));
        let revision = Revision::of(b"[admission]\nheadroom_pct = 20.0\n");
        assert_eq!(revision.to_string().parse(), Ok(revision));
        assert_eq!("12".parse::<Revision>(), Err(ParseRevisionError));
        assert_eq!(
            "zzzzzzzzzzzzzzzz".parse::<Revision>(),
            Err(ParseRevisionError)
        );
    }

    #[test]
    fn test_revision_json_is_a_hex_string() {
        let revision = Revision::of(b"");
        let json = serde_json::to_string(&revision).unwrap();
        assert_eq!(json, "\"cbf29ce484222325\"");
        assert_eq!(serde_json::from_str::<Revision>(&json).unwrap(), revision);
        assert!(serde_json::from_str::<Revision>("12").is_err());
    }
}
