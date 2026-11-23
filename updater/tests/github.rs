//! The updater against the real releases on github.com. These need the network, so they are
//! ignored; run them by hand:
//!
//! ```text
//! cargo test -p allot-updater --test github -- --ignored --nocapture
//! ```
//!
//! `ALLOT_UPDATER_TEST_HOME` names a folder to download into and keep; without it the
//! download goes to a temporary folder that is removed afterwards.

use std::{env, fs, path::PathBuf};

use allot_shared::{paths::Paths, release::Asset};
use allot_updater::{Check, Limits, Source, Updater, Version};

fn updater() -> Updater {
    Updater::new(Source::github(), Limits::default())
}

#[test]
#[ignore = "contacts github.com"]
fn test_github_latest_redirect_names_a_stable_release() {
    let outcome = updater().check(&Version::new(0, 0, 1)).unwrap();
    let Check::Newer(release) = outcome else {
        panic!("expected a release newer than 0.0.1, got {outcome:?}");
    };
    println!("latest: {} at {}", release.tag, release.page_url);
    assert!(release.version >= Version::new(0, 6, 0));
    assert!(release.version.pre.is_empty());
}

#[test]
#[ignore = "contacts github.com"]
fn test_github_asset_downloads_over_https_and_matches_its_checksum() {
    let updater = updater();
    let release = updater.release(Version::new(0, 6, 0));
    let kept = env::var_os("ALLOT_UPDATER_TEST_HOME").map(PathBuf::from);
    let scratch = tempfile::tempdir().unwrap();
    let home = kept.clone().unwrap_or_else(|| scratch.path().to_path_buf());
    fs::create_dir_all(&home).unwrap();

    let verified = updater
        .fetch(&Paths::under(&home), &release, Asset::KdeWidget)
        .unwrap();

    println!(
        "{}: {} bytes, sha256 {}",
        verified.path().display(),
        verified.bytes(),
        verified.sha256()
    );
    assert!(verified.bytes() > 0);
    // Windows adds a Zone.Identifier stream when the downloading program asks for it; the
    // updater writes the file itself and must not carry one.
    #[cfg(windows)]
    {
        let stream = format!("{}:Zone.Identifier", verified.path().display());
        assert!(fs::File::open(stream).is_err());
    }
}
