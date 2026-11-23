use std::{
    fmt::Write as _,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use allot_shared::{paths::Paths, release::Asset};
use semver::Version;
use sha2::{Digest, Sha256};
use ureq::{Body, http::Response};

use crate::{
    checksum,
    error::{Result, UpdateError},
    folder,
    lookup::Release,
    updater::Updater,
};

const CHUNK: usize = 64 * 1024;

/// A release file whose SHA-256 matched the one the release published, now in its update
/// folder.
///
/// Only [`Updater::fetch`] makes one, and [`Updater::unpack`] takes nothing else, so nothing is
/// unpacked, and by the callers' convention nothing is run, before its hash was checked. The
/// `.sha256` comes from the same server as the file: it proves the file arrived whole, and
/// HTTPS, not the hash, proves who sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub(crate) asset: Asset,
    pub(crate) version: Version,
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
    pub(crate) bytes: u64,
}

impl Verified {
    /// Which release file this is.
    pub fn asset(&self) -> Asset {
        self.asset
    }

    /// The release it belongs to.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Where the file is, inside `~/.allot/updates/<version>/`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's SHA-256 in lower-case hex.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// The file's size.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Removes the half-written download unless it was renamed into place.
struct PartFile {
    path: PathBuf,
    kept: bool,
}

impl Drop for PartFile {
    fn drop(&mut self) {
        if !self.kept {
            // Nothing to do when it is already gone; a leftover is removed by the next fetch.
            let _ = fs::remove_file(&self.path);
        }
    }
}

impl Updater {
    /// Downloads `asset` of `release` into `~/.allot/updates/<version>/` and checks it against
    /// the release's `.sha256`.
    ///
    /// The checksum file is read first, and refused unless it names `asset`, so a mix-up costs
    /// no big download. The file is written beside its final name, hashed as it arrives, and
    /// renamed into place only when the hash matches; every failure leaves neither the file
    /// nor a partial one. The `.sha256` is kept next to it.
    ///
    /// # Errors
    ///
    /// Fails when the release has no such file ([`UpdateError::AssetMissing`]), when a limit is
    /// exceeded or the connection stalls, ends early or loops through redirects, when the
    /// `.sha256` is unusable or names another file, and with
    /// [`UpdateError::ChecksumMismatch`] when the hash differs.
    pub fn fetch(&self, paths: &Paths, release: &Release, asset: Asset) -> Result<Verified> {
        let version = release.version.to_string();
        let file = asset.file_name(&version);
        let checksum_name = asset.checksum_name(&version);
        let folder = folder::of(paths, &release.version);
        fs::create_dir_all(&folder).map_err(|error| UpdateError::io("create", &folder, error))?;

        let checksum_url = self.source.asset_url(&release.tag, &checksum_name);
        let checksum_text = self.download_text(&checksum_url, &checksum_name, release)?;
        let expected = checksum::listed_hash(&checksum_name, &checksum_text, &file)?;

        let url = self.source.asset_url(&release.tag, &file);
        let part_path = folder.join(format!("{file}.part"));
        // Declared before the file so the file is closed before the guard removes it.
        let mut part = PartFile {
            path: part_path,
            kept: false,
        };
        let (actual, bytes) = self.download_file(&url, &file, release, &part.path)?;
        if actual != expected {
            return Err(UpdateError::ChecksumMismatch {
                file,
                expected,
                actual,
            });
        }

        let path = folder.join(&file);
        write(&folder.join(&checksum_name), checksum_text.as_bytes())?;
        fs::rename(&part.path, &path).map_err(|error| UpdateError::io("rename", &path, error))?;
        part.kept = true;
        Ok(Verified {
            asset,
            version: release.version.clone(),
            path,
            sha256: actual,
            bytes,
        })
    }

    fn download_text(&self, url: &str, name: &str, release: &Release) -> Result<String> {
        let response = self.download_response(url, name, release)?;
        let mut bytes = Vec::new();
        self.copy_body(
            url,
            name,
            response,
            self.limits.max_checksum_bytes,
            |chunk| {
                bytes.extend_from_slice(chunk);
                Ok(())
            },
        )?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Writes the body of `url` to `part_path` and returns its SHA-256 and size.
    fn download_file(
        &self,
        url: &str,
        name: &str,
        release: &Release,
        part_path: &Path,
    ) -> Result<(String, u64)> {
        let response = self.download_response(url, name, release)?;
        let mut file =
            File::create(part_path).map_err(|error| UpdateError::io("create", part_path, error))?;
        let mut hasher = Sha256::new();
        let bytes = self.copy_body(url, name, response, self.limits.max_asset_bytes, |chunk| {
            hasher.update(chunk);
            file.write_all(chunk)
                .map_err(|error| UpdateError::io("write", part_path, error))
        })?;
        file.sync_all()
            .map_err(|error| UpdateError::io("write", part_path, error))?;
        Ok((hex(&hasher.finalize()), bytes))
    }

    /// The response for `url`, or the error for a missing file or an unexpected status.
    fn download_response(
        &self,
        url: &str,
        name: &str,
        release: &Release,
    ) -> Result<Response<Body>> {
        let response = self.get(&self.download_agent, url)?;
        match response.status().as_u16() {
            200 => Ok(response),
            404 => Err(UpdateError::AssetMissing {
                file: name.to_owned(),
                tag: release.tag.clone(),
            }),
            status => Err(UpdateError::Status {
                url: url.to_owned(),
                status,
            }),
        }
    }

    /// Hands the body to `sink` in chunks, refusing more than `limit` bytes, and returns how
    /// many arrived. A body shorter than its `Content-Length` is an error, not a short file.
    fn copy_body(
        &self,
        url: &str,
        name: &str,
        response: Response<Body>,
        limit: u64,
        mut sink: impl FnMut(&[u8]) -> Result<()>,
    ) -> Result<u64> {
        let too_large = || UpdateError::TooLarge {
            url: url.to_owned(),
            limit,
        };
        let body = response.into_body();
        let expected = body.content_length();
        if expected.is_some_and(|length| length > limit) {
            return Err(too_large());
        }
        let mut reader = body.into_reader();
        let mut buffer = vec![0; CHUNK];
        let mut received = 0u64;
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    return Err(self.body_error(url, name, received, expected, error));
                }
            };
            received += read as u64;
            if received > limit {
                return Err(too_large());
            }
            sink(&buffer[..read])?;
        }
        if expected.is_some_and(|length| length != received) {
            return Err(UpdateError::Truncated {
                file: name.to_owned(),
                received,
                expected,
            });
        }
        Ok(received)
    }
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).map_err(|error| UpdateError::io("write", path, error))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            // Writing to a String cannot fail.
            let _ = write!(text, "{byte:02x}");
            text
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_pads_every_byte_to_two_digits() {
        assert_eq!(hex(&[0x00, 0x0a, 0xff, 0x10]), "000aff10");
    }
}
