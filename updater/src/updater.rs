use std::{
    env::consts::{ARCH, OS},
    io,
    time::Duration,
};

use allot_shared::release::{Asset, Target};
use ureq::{
    Agent, Body,
    http::Response,
    tls::{RootCerts, TlsConfig},
};

use crate::{
    error::{Result, UpdateError},
    limits::Limits,
    source::Source,
};

/// The file this platform's update comes from: the setup on Windows, which also holds the tray
/// app, and the release archive elsewhere.
///
/// # Errors
///
/// Fails with [`UpdateError::UnsupportedPlatform`] where the release has no build.
pub fn platform_asset() -> Result<Asset> {
    match Target::current() {
        Some(Target::WindowsX64) => Ok(Asset::Setup),
        Some(target) => Ok(Asset::Archive(target)),
        None => Err(UpdateError::UnsupportedPlatform { os: OS, arch: ARCH }),
    }
}

/// Finds releases, downloads their files and checks them, from one [`Source`].
///
/// Everything blocks. Run it from `tokio::task::spawn_blocking` inside an async program.
#[derive(Debug)]
pub struct Updater {
    pub(crate) source: Source,
    pub(crate) limits: Limits,
    /// Never follows a redirect: the lookup reads where `releases/latest` points, nothing more.
    pub(crate) lookup_agent: Agent,
    pub(crate) download_agent: Agent,
}

impl Updater {
    /// An updater for `source` that stays within `limits`.
    pub fn new(source: Source, limits: Limits) -> Self {
        let lookup_agent = agent(&source, &limits, 0, limits.lookup_timeout);
        let download_agent = agent(
            &source,
            &limits,
            limits.max_redirects,
            limits.download_timeout,
        );
        Self {
            source,
            limits,
            lookup_agent,
            download_agent,
        }
    }

    /// An updater for the source `ALLOT_UPDATE_BASE` names, or GitHub, with the default limits.
    ///
    /// # Errors
    ///
    /// As [`Source::from_env`].
    pub fn from_env() -> Result<Self> {
        Ok(Self::new(Source::from_env()?, Limits::default()))
    }

    /// Where this updater reads releases from.
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The limits this updater keeps.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// `GET url` with `agent`. Every status comes back as a response, for the caller to judge;
    /// only a failed exchange is an error.
    pub(crate) fn get(&self, agent: &Agent, url: &str) -> Result<Response<Body>> {
        agent
            .get(url)
            .call()
            .map_err(|error| self.exchange_error(url, error))
    }

    pub(crate) fn exchange_error(&self, url: &str, error: ureq::Error) -> UpdateError {
        let url = url.to_owned();
        match error {
            ureq::Error::Timeout(timeout) => UpdateError::Timeout {
                url,
                reason: timeout.to_string(),
            },
            ureq::Error::TooManyRedirects => UpdateError::TooManyRedirects {
                url,
                limit: self.limits.max_redirects,
            },
            other => UpdateError::Network {
                url,
                reason: other.to_string(),
            },
        }
    }

    /// What a failed read of a response body means. The HTTP client wraps its own errors in
    /// the `io::Error` it hands back, so a timeout is found inside it.
    pub(crate) fn body_error(
        &self,
        url: &str,
        file: &str,
        received: u64,
        expected: Option<u64>,
        error: io::Error,
    ) -> UpdateError {
        let wrapped = error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<ureq::Error>());
        match wrapped {
            Some(ureq::Error::Timeout(timeout)) => UpdateError::Timeout {
                url: url.to_owned(),
                reason: timeout.to_string(),
            },
            Some(other) => UpdateError::Network {
                url: url.to_owned(),
                reason: other.to_string(),
            },
            None if error.kind() == io::ErrorKind::UnexpectedEof => UpdateError::Truncated {
                file: file.to_owned(),
                received,
                expected,
            },
            None => UpdateError::Network {
                url: url.to_owned(),
                reason: error.to_string(),
            },
        }
    }
}

/// A client that trusts the operating system's roots, so an old Allot still reaches GitHub
/// after GitHub changes its certificate authority, and speaks no more than the exchange needs.
fn agent(source: &Source, limits: &Limits, redirects: u32, whole: Duration) -> Agent {
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    Agent::config_builder()
        .https_only(source.is_https())
        .tls_config(tls)
        .http_status_as_error(false)
        .max_redirects(redirects)
        .user_agent(concat!("allot-updater/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(limits.connect_timeout))
        .timeout_per_read(Some(limits.stall_timeout))
        .timeout_global(Some(whole))
        .build()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_asset_matches_this_build() {
        let asset = platform_asset();
        match Target::current() {
            Some(Target::WindowsX64) => assert_eq!(asset.unwrap(), Asset::Setup),
            Some(target) => assert_eq!(asset.unwrap(), Asset::Archive(target)),
            None => assert!(matches!(
                asset,
                Err(UpdateError::UnsupportedPlatform { .. })
            )),
        }
    }
}
