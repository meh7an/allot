//! Finds the latest Allot release, downloads one of its files and proves it intact.
//!
//! `allot update` and allotd build on this crate. Its steps, each its own call:
//!
//! 1. [`Updater::check`] reads where `releases/latest` redirects to, without following it, and
//!    says whether that stable release is newer than the installed version.
//! 2. [`Updater::fetch`] downloads a file of that release into `~/.allot/updates/<version>/`
//!    and checks its SHA-256 against the release's `.sha256`.
//! 3. [`Updater::unpack`] unpacks a verified `.tar.gz`, refusing any entry that would leave its
//!    folder.
//! 4. [`clean`] removes the folders of other versions.
//!
//! Releases are read from GitHub unless `ALLOT_UPDATE_BASE` names another address, which is
//! how tests and CI serve a release from a local server (see [`Source`]).

mod checksum;
mod error;
mod fetch;
pub mod folder;
mod limits;
mod lookup;
mod source;
mod unpack;
mod updater;

pub use error::{Result, UnsafeReason, UpdateError};
pub use fetch::Verified;
pub use folder::clean;
pub use limits::Limits;
pub use lookup::{Check, Release};
pub use semver::Version;
pub use source::{BASE_VARIABLE, GITHUB_BASE, Source};
pub use updater::{Updater, platform_asset};
