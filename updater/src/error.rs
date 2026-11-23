use std::{io, path::PathBuf};

/// What the updater returns.
pub type Result<T, E = UpdateError> = std::result::Result<T, E>;

/// Everything that can stop a lookup, download or unpack.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UpdateError {
    /// The release has no archive for this operating system and processor.
    #[error("there is no Allot release for {os} on {arch}")]
    UnsupportedPlatform {
        /// `std::env::consts::OS`.
        os: &'static str,
        /// `std::env::consts::ARCH`.
        arch: &'static str,
    },
    /// `ALLOT_UPDATE_BASE`, or the address given to [`Source::new`](crate::Source::new), is
    /// unusable.
    #[error("{base:?} cannot be the address of a release page: {reason}")]
    BadBase {
        /// The address as given.
        base: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// No connection, a failed TLS handshake, or a broken exchange.
    #[error("could not reach {url}: {reason}")]
    Network {
        /// The address asked for.
        url: String,
        /// What the HTTP client reported.
        reason: String,
    },
    /// The server did not answer, or stopped sending, within the limits.
    #[error("{url} took too long: {reason}")]
    Timeout {
        /// The address asked for.
        url: String,
        /// Which limit ran out.
        reason: String,
    },
    /// Redirects that never ended, or only ended after more hops than allowed.
    #[error("{url} redirected more than {limit} times")]
    TooManyRedirects {
        /// The address asked for.
        url: String,
        /// How many redirects are followed.
        limit: u32,
    },
    /// The server answered with a status the updater does not handle.
    #[error("{url} answered {status}")]
    Status {
        /// The address asked for.
        url: String,
        /// The HTTP status code.
        status: u16,
    },
    /// The release has no such file, or the release itself does not exist.
    #[error("{file} is not part of release {tag}")]
    AssetMissing {
        /// The file asked for.
        file: String,
        /// The release tag.
        tag: String,
    },
    /// `releases/latest` did not point at a release.
    #[error("{url} did not point at a release: {reason}")]
    BadLatest {
        /// The address asked for.
        url: String,
        /// What it answered instead.
        reason: String,
    },
    /// The answer is bigger than the limit for this kind of file.
    #[error("{url} is larger than the limit of {limit} bytes")]
    TooLarge {
        /// The address asked for.
        url: String,
        /// The limit in bytes.
        limit: u64,
    },
    /// The connection ended before the whole file arrived.
    #[error("the download of {file} ended early, after {received} bytes")]
    Truncated {
        /// The file being downloaded.
        file: String,
        /// How many bytes arrived.
        received: u64,
        /// How many the server announced, when it did.
        expected: Option<u64>,
    },
    /// The `.sha256` file is not one line with a hash and a name.
    #[error("{file} is not a checksum file: {reason}")]
    BadChecksumFile {
        /// The checksum file's name.
        file: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// The `.sha256` file is the checksum of another file.
    #[error("{checksum_file} holds the checksum of {listed}, not of {expected}")]
    ChecksumNamesOther {
        /// The checksum file's name.
        checksum_file: String,
        /// The name inside it.
        listed: String,
        /// The file it should cover.
        expected: String,
    },
    /// The download is not the file the release published.
    #[error("{file} does not match its checksum: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// The file that was downloaded.
        file: String,
        /// The hash in the `.sha256` file.
        expected: String,
        /// The hash of what arrived.
        actual: String,
    },
    /// Only the gzipped tarballs are unpacked.
    #[error("{file} is not a gzipped tarball, so it cannot be unpacked")]
    NotATarball {
        /// The file's name.
        file: String,
    },
    /// The archive is cut off, damaged or has no release folder.
    #[error("{file} is not a usable archive: {reason}")]
    BadArchive {
        /// The archive's name.
        file: String,
        /// What is wrong with it.
        reason: String,
    },
    /// An archive entry would write outside the release folder, or is something the
    /// updater never unpacks. Nothing of the archive is left on disk.
    #[error("refused archive entry {path:?}: {reason}")]
    UnsafeEntry {
        /// The entry's path as the archive spells it.
        path: String,
        /// Why it was refused.
        reason: UnsafeReason,
    },
    /// A file or folder could not be read, written or removed.
    #[error("could not {action} {}: {source}", .path.display())]
    Io {
        /// What was being done.
        action: &'static str,
        /// The file or folder.
        path: PathBuf,
        /// What the system reported.
        source: io::Error,
    },
}

impl UpdateError {
    pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

/// Why an archive entry was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UnsafeReason {
    /// The path starts at a root or a drive.
    #[error("the path is absolute")]
    Absolute,
    /// The path climbs out of its folder with `..`.
    #[error("the path climbs out of its folder with `..`")]
    ParentDir,
    /// The path is not under the release's own folder.
    #[error("the path is outside the release folder")]
    OutsideRoot,
    /// The path has a part that is not plain text, or holds a colon.
    #[error("the path has a part the updater does not unpack")]
    OddName,
    /// A link points absolutely, upwards or at a file outside the release folder.
    #[error("the link points outside the release folder")]
    LinkOutside,
    /// A device, a pipe, a sparse file or another kind of entry that is not a file, folder or
    /// link.
    #[error("this kind of entry is never unpacked")]
    UnsupportedType,
    /// A second entry with a path that already exists.
    #[error("the path appears twice")]
    Duplicate,
    /// Symbolic links cannot be created on this platform.
    #[error("symbolic links are not unpacked on this platform")]
    LinksUnsupported,
    /// More entries than the limit.
    #[error("the archive has too many entries")]
    TooManyEntries,
    /// More data than the limit.
    #[error("the archive holds more data than the limit")]
    TooMuchData,
}
