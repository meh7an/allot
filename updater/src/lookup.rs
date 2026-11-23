use allot_shared::release;
use semver::Version;

use crate::{
    error::{Result, UpdateError},
    source::Source,
    updater::Updater,
};

/// A published, stable release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The version, which has no pre-release part and no build metadata.
    pub version: Version,
    /// The git tag, `v<version>`.
    pub tag: String,
    /// The release's page, for a notice or a failure to point at.
    pub page_url: String,
}

impl Release {
    /// The release named by `tag`, or `None` for a tag that is not `v` and a plain
    /// `major.minor.patch`: a pre-release such as `v0.7.0-beta.1`, or `nightly`.
    fn from_tag(source: &Source, tag: &str) -> Option<Self> {
        let version = Version::parse(tag.strip_prefix('v')?).ok()?;
        if !version.pre.is_empty() || !version.build.is_empty() {
            return None;
        }
        Some(Self::of(source, version))
    }

    fn of(source: &Source, version: Version) -> Self {
        let tag = release::tag(&version.to_string());
        Self {
            page_url: source.page_url(&tag),
            version,
            tag,
        }
    }
}

/// What the latest release means for the installed version.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Check {
    /// A stable release newer than the installed version.
    Newer(Release),
    /// Nothing to offer: the latest stable release is the installed version or older, or the
    /// latest tag is not a stable release at all and so says nothing.
    NotNewer {
        /// The latest stable release, when there is one.
        latest: Option<Release>,
    },
}

impl Updater {
    /// Asks where `releases/latest` points and compares that release with `installed`.
    ///
    /// The redirect is read, never followed: no page of the repository is fetched. GitHub
    /// leaves drafts and pre-releases out of "latest" itself; a tag that is neither a plain
    /// `vX.Y.Z` is ignored here as well, and one that is not newer is no update.
    ///
    /// # Errors
    ///
    /// Fails when the server cannot be reached or does not answer with a redirect to a tag.
    pub fn check(&self, installed: &Version) -> Result<Check> {
        let tag = self.latest_tag()?;
        Ok(match Release::from_tag(&self.source, &tag) {
            Some(release) if release.version > *installed => Check::Newer(release),
            latest => Check::NotNewer { latest },
        })
    }

    /// The release of `version` without asking the server, for installing a chosen version.
    /// A version that was never published shows as a missing file when it is fetched.
    pub fn release(&self, version: Version) -> Release {
        Release::of(&self.source, version)
    }

    fn latest_tag(&self) -> Result<String> {
        let url = self.source.latest_url();
        let response = self.get(&self.lookup_agent, &url)?;
        let status = response.status();
        let bad = |reason: String| UpdateError::BadLatest {
            url: url.clone(),
            reason,
        };
        if status.is_client_error() || status.is_server_error() {
            return Err(UpdateError::Status {
                url: url.clone(),
                status: status.as_u16(),
            });
        }
        if !status.is_redirection() {
            return Err(bad(format!("it answered {status} instead of a redirect")));
        }
        let location = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| bad("the redirect has no usable Location".to_owned()))?;
        tag_in(location).map(str::to_owned).ok_or_else(|| {
            bad(format!(
                "it redirected to {location}, which is not a release tag"
            ))
        })
    }
}

/// The tag in `…/tag/<tag>`, the last two parts of where `releases/latest` redirects to.
fn tag_in(location: &str) -> Option<&str> {
    let path = location.split(['?', '#']).next()?.trim_end_matches('/');
    let mut parts = path.rsplit('/');
    let tag = parts.next().filter(|tag| !tag.is_empty())?;
    (parts.next()? == "tag").then_some(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_in_reads_absolute_and_relative_locations() {
        assert_eq!(
            tag_in("https://github.com/meh7an/allot/releases/tag/v0.6.0"),
            Some("v0.6.0")
        );
        assert_eq!(tag_in("/meh7an/allot/releases/tag/v0.6.0/"), Some("v0.6.0"));
        assert_eq!(tag_in("/tag/v1.0.0?x=1#y"), Some("v1.0.0"));
    }

    #[test]
    fn test_tag_in_refuses_other_locations() {
        for location in [
            "",
            "https://github.com/meh7an/allot/releases",
            "https://github.com/meh7an/allot/releases/latest",
            "https://github.com/meh7an/allot/releases/tag/",
            "tag",
        ] {
            assert_eq!(tag_in(location), None, "{location}");
        }
    }

    #[test]
    fn test_release_from_tag_takes_only_plain_versions() {
        let source = Source::github();
        let release = Release::from_tag(&source, "v0.7.1").unwrap();
        assert_eq!(release.version, Version::new(0, 7, 1));
        assert_eq!(release.tag, "v0.7.1");
        assert_eq!(
            release.page_url,
            "https://github.com/meh7an/allot/releases/tag/v0.7.1"
        );
        for tag in [
            "v0.7.0-beta.1",
            "v0.7.0+build",
            "0.7.0",
            "nightly",
            "v0.7",
            "v01.0.0",
            "V0.7.0",
            "v",
        ] {
            assert_eq!(Release::from_tag(&source, tag), None, "{tag}");
        }
    }
}
