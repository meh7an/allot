use std::{env, net::IpAddr};

use crate::error::{Result, UpdateError};

/// The environment variable that replaces [`GITHUB_BASE`], for tests and CI.
pub const BASE_VARIABLE: &str = "ALLOT_UPDATE_BASE";

/// Where Allot's releases live: `<base>/latest` redirects to the newest release's page and
/// `<base>/download/<tag>/<file>` serves its files, the same shape `install.sh` reads through
/// `ALLOT_DOWNLOAD_BASE`.
pub const GITHUB_BASE: &str = "https://github.com/meh7an/allot/releases";

/// The address the updater reads releases from.
///
/// The real one is HTTPS. A plain `http` address is accepted only for a loopback host, so a
/// stray `ALLOT_UPDATE_BASE` cannot send an update over a network in the clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    base: String,
    https: bool,
}

impl Source {
    /// GitHub's releases of Allot.
    pub fn github() -> Self {
        Self {
            base: GITHUB_BASE.to_owned(),
            https: true,
        }
    }

    /// A source at `base`, such as a local server for a test: `http://127.0.0.1:8080/releases`.
    ///
    /// # Errors
    ///
    /// Fails with [`UpdateError::BadBase`] when `base` is not an `https` address, or an `http`
    /// address of a loopback host, or carries a query, a fragment or credentials.
    pub fn new(base: &str) -> Result<Self> {
        let bad = |reason| UpdateError::BadBase {
            base: base.to_owned(),
            reason,
        };
        let trimmed = base.trim().trim_end_matches('/');
        let (https, rest) = if let Some(rest) = trimmed.strip_prefix("https://") {
            (true, rest)
        } else if let Some(rest) = trimmed.strip_prefix("http://") {
            (false, rest)
        } else {
            return Err(bad("it must start with https://"));
        };
        if rest.contains(['?', '#']) {
            return Err(bad("it must not have a query or a fragment"));
        }
        let authority = rest.split('/').next().unwrap_or_default();
        if authority.contains('@') {
            return Err(bad("it must not carry credentials"));
        }
        let host = host_of(authority);
        if host.is_empty() {
            return Err(bad("it has no host"));
        }
        if !https && !is_loopback(host) {
            return Err(bad("plain http is allowed only for a loopback host"));
        }
        Ok(Self {
            base: trimmed.to_owned(),
            https,
        })
    }

    /// The source named by [`BASE_VARIABLE`], or GitHub's when it is unset or empty.
    ///
    /// # Errors
    ///
    /// Fails with [`UpdateError::BadBase`] when the variable is not a usable address.
    pub fn from_env() -> Result<Self> {
        Self::from_variable(env::var(BASE_VARIABLE))
    }

    fn from_variable(value: std::result::Result<String, env::VarError>) -> Result<Self> {
        match value {
            Ok(base) if !base.trim().is_empty() => Self::new(&base),
            Ok(_) | Err(env::VarError::NotPresent) => Ok(Self::github()),
            Err(env::VarError::NotUnicode(_)) => Err(UpdateError::BadBase {
                base: String::new(),
                reason: "it is not valid text",
            }),
        }
    }

    /// The address without a trailing slash.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// `false` for a loopback `http` source.
    pub(crate) fn is_https(&self) -> bool {
        self.https
    }

    /// The address that redirects to the newest release.
    pub(crate) fn latest_url(&self) -> String {
        format!("{}/latest", self.base)
    }

    /// The release's page.
    pub(crate) fn page_url(&self, tag: &str) -> String {
        format!("{}/tag/{tag}", self.base)
    }

    /// One file of the release `tag`.
    pub(crate) fn asset_url(&self, tag: &str, file: &str) -> String {
        format!("{}/download/{tag}/{file}", self.base)
    }
}

/// The host in `host`, `host:port` or `[::1]:port`.
fn host_of(authority: &str) -> &str {
    match authority.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or_default(),
        None => authority.split(':').next().unwrap_or_default(),
    }
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_github_urls_follow_the_release_layout() {
        let source = Source::github();
        assert_eq!(
            source.latest_url(),
            "https://github.com/meh7an/allot/releases/latest"
        );
        assert_eq!(
            source.page_url("v0.7.0"),
            "https://github.com/meh7an/allot/releases/tag/v0.7.0"
        );
        assert_eq!(
            source.asset_url("v0.7.0", "allot-0.7.0-setup.exe"),
            "https://github.com/meh7an/allot/releases/download/v0.7.0/allot-0.7.0-setup.exe"
        );
        assert!(source.is_https());
    }

    #[test]
    fn test_source_new_accepts_https_and_loopback_http() {
        for base in [
            "https://example.com/releases",
            "https://example.com/releases/",
            "http://127.0.0.1:8080/releases",
            "http://localhost:9",
            "http://[::1]:9/x",
        ] {
            let source = Source::new(base).unwrap();
            assert!(!source.base().ends_with('/'), "{base}");
        }
    }

    #[test]
    fn test_source_new_refuses_unsafe_addresses() {
        for base in [
            "",
            "ftp://example.com/releases",
            "example.com/releases",
            "http://example.com/releases",
            "http://127.0.0.1.example.com/releases",
            "https://user:secret@example.com/releases",
            "https://example.com/releases?x=1",
            "https://example.com/releases#top",
            "https:///releases",
        ] {
            assert!(
                matches!(Source::new(base), Err(UpdateError::BadBase { .. })),
                "{base}"
            );
        }
    }

    #[test]
    fn test_source_from_variable_falls_back_to_github_when_unset_or_empty() {
        for value in [
            Err(env::VarError::NotPresent),
            Ok(String::new()),
            Ok("  ".into()),
        ] {
            assert_eq!(Source::from_variable(value).unwrap(), Source::github());
        }
    }

    #[test]
    fn test_source_from_variable_uses_and_checks_the_override() {
        let source = Source::from_variable(Ok("http://127.0.0.1:5/releases/".into())).unwrap();
        assert_eq!(source.base(), "http://127.0.0.1:5/releases");
        let refused = Source::from_variable(Ok("http://example.com".into()));
        assert!(matches!(refused, Err(UpdateError::BadBase { .. })));
        let unreadable = Source::from_variable(Err(env::VarError::NotUnicode("x".into())));
        assert!(matches!(unreadable, Err(UpdateError::BadBase { .. })));
    }

    #[test]
    fn test_source_new_marks_http_as_not_https() {
        assert!(!Source::new("http://127.0.0.1:1").unwrap().is_https());
        assert!(Source::new("https://example.com").unwrap().is_https());
    }
}
