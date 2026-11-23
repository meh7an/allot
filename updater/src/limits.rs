use std::time::Duration;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;

/// How much and how long the updater accepts. The defaults suit the real release; tests lower
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// The most bytes a release file may have. The largest is the Windows setup, about 24 MB.
    pub max_asset_bytes: u64,
    /// The most bytes a `.sha256` file may have; a real one is under 200.
    pub max_checksum_bytes: u64,
    /// The most bytes all files of an unpacked archive may add up to.
    pub max_unpacked_bytes: u64,
    /// The most entries an archive may have.
    pub max_entries: usize,
    /// How many redirects a download follows: GitHub sends one, to its file host.
    pub max_redirects: u32,
    /// How long to wait for a connection, TLS included.
    pub connect_timeout: Duration,
    /// How long a read may wait for the next bytes before the download counts as stalled.
    pub stall_timeout: Duration,
    /// How long the whole lookup of the latest release may take.
    pub lookup_timeout: Duration,
    /// How long one whole download may take, redirects included.
    pub download_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_asset_bytes: 256 * MIB,
            max_checksum_bytes: 4 * KIB,
            max_unpacked_bytes: 1024 * MIB,
            max_entries: 1000,
            max_redirects: 5,
            connect_timeout: Duration::from_secs(10),
            stall_timeout: Duration::from_secs(30),
            lookup_timeout: Duration::from_secs(20),
            download_timeout: Duration::from_secs(15 * 60),
        }
    }
}
