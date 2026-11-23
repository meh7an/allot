//! macOS: when Allot.app is installed, it takes the update itself (it swaps its own bundle and
//! then installs its bundled service), and `allot update` hands it the version by URL.
//!
//! Only an app that declares the URL scheme in its `Info.plist` gets the hand-off. The apps
//! before the release that added the handler have none, and a URL sent to them would go nowhere.

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use allot_shared::paths::Paths;
use allot_updater::Version;
use anyhow::{Context, bail};
use serde_json::Value;

/// The app's bundle id, which is also the scheme of its hand-off URL, so no other app claims it.
const BUNDLE_ID: &str = "tech.mehran.allot";
const APP_NAME: &str = "Allot.app";
const INFO_PLIST: &str = "Contents/Info.plist";
/// How long the app has, after the hand-off, to bring `~/.allot/bin` to the new version: it
/// downloads the app, swaps its bundle, restarts, and installs its bundled service.
pub(super) const HAND_OFF_WAIT: Duration = Duration::from_secs(180);

/// An installed Allot.app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct App {
    pub(super) path: PathBuf,
    /// Its `CFBundleShortVersionString`.
    pub(super) version: Option<String>,
    /// Whether it declares the hand-off URL scheme.
    pub(super) takes_hand_off: bool,
}

/// The Allot.app in `/Applications` or the user's `~/Applications`.
pub(super) fn find(paths: &Paths) -> Option<App> {
    let folders = [
        PathBuf::from("/Applications"),
        paths.home().join("Applications"),
    ];
    find_in(&folders, read_info)
}

fn find_in(folders: &[PathBuf], read_info: impl Fn(&Path) -> Option<Value>) -> Option<App> {
    folders.iter().find_map(|folder| {
        let path = folder.join(APP_NAME);
        let info = read_info(&path.join(INFO_PLIST))?;
        describe(path, &info)
    })
}

/// The app an `Info.plist` describes, unless the bundle is another app's.
fn describe(path: PathBuf, info: &Value) -> Option<App> {
    if info["CFBundleIdentifier"] != BUNDLE_ID {
        return None;
    }
    let mut schemes = info["CFBundleURLTypes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|kind| kind["CFBundleURLSchemes"].as_array())
        .flatten();
    Some(App {
        path,
        version: info["CFBundleShortVersionString"]
            .as_str()
            .map(str::to_owned),
        takes_hand_off: schemes.any(|scheme| scheme == BUNDLE_ID),
    })
}

/// An `Info.plist` as JSON. `plutil` reads the XML and the binary form alike.
fn read_info(plist: &Path) -> Option<Value> {
    let output = Command::new("plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(plist)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| serde_json::from_slice(&output.stdout).ok())
        .flatten()
}

/// The URL that asks the app to update itself to `version`.
fn hand_off_url(version: &Version) -> String {
    format!("{BUNDLE_ID}://update?version={version}")
}

/// Opens the hand-off URL in the app at `app`, which starts it when it is not running.
///
/// # Errors
///
/// Fails when `open` cannot run or does not deliver the URL.
pub(super) fn hand_off(app: &Path, version: &Version) -> anyhow::Result<()> {
    let url = hand_off_url(version);
    let status = Command::new("open")
        .arg("-a")
        .arg(app)
        .arg(&url)
        .status()
        .context("starting open")?;
    if !status.success() {
        bail!(
            "opening {url} in {} failed: open exited with {status}",
            app.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn info(id: &str, version: &str, schemes: &[&str]) -> Value {
        json!({
            "CFBundleIdentifier": id,
            "CFBundleShortVersionString": version,
            "CFBundleURLTypes": [{ "CFBundleURLSchemes": schemes }],
        })
    }

    #[test]
    fn test_describe_takes_an_app_that_declares_the_hand_off_scheme() {
        let app = describe(
            PathBuf::from("/Applications/Allot.app"),
            &info(BUNDLE_ID, "0.7.0", &[BUNDLE_ID]),
        )
        .unwrap();

        assert!(app.takes_hand_off);
        assert_eq!(app.version.as_deref(), Some("0.7.0"));
    }

    #[test]
    fn test_describe_does_not_hand_off_to_an_app_without_the_scheme() {
        let without_types = json!({
            "CFBundleIdentifier": BUNDLE_ID,
            "CFBundleShortVersionString": "0.6.1",
        });
        for info in [
            without_types,
            info(BUNDLE_ID, "0.6.1", &[]),
            info(BUNDLE_ID, "0.6.1", &["other-scheme"]),
        ] {
            let app = describe(PathBuf::from("Allot.app"), &info).unwrap();
            assert!(!app.takes_hand_off, "{info}");
            assert_eq!(app.version.as_deref(), Some("0.6.1"));
        }
    }

    #[test]
    fn test_describe_refuses_a_bundle_that_is_not_allot() {
        let other = info("com.example.Allot", "1.0", &[BUNDLE_ID]);
        assert_eq!(describe(PathBuf::from("Allot.app"), &other), None);
        assert_eq!(describe(PathBuf::from("Allot.app"), &json!({})), None);
    }

    #[test]
    fn test_find_in_takes_the_first_folder_with_a_real_allot() {
        let folders = [
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        let read = |plist: &Path| match plist.to_str().unwrap() {
            p if p.starts_with("/a") => None,
            p if p.starts_with("/b") => Some(info("com.example.Allot", "1.0", &[])),
            _ => Some(info(BUNDLE_ID, "0.7.0", &[BUNDLE_ID])),
        };

        let app = find_in(&folders, read).unwrap();

        assert_eq!(app.path, Path::new("/c").join(APP_NAME));
        assert_eq!(find_in(&folders[..2], read), None);
    }

    #[test]
    fn test_hand_off_url_names_the_version_under_the_apps_own_scheme() {
        let version = Version::parse("0.7.0").unwrap();
        assert_eq!(
            hand_off_url(&version),
            "tech.mehran.allot://update?version=0.7.0"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_read_info_reads_an_xml_plist_through_plutil() {
        use std::fs;

        use crate::test_support::TempDir;

        let dir = TempDir::new();
        let plist = dir.path().join("Info.plist");
        fs::write(
            &plist,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>tech.mehran.allot</string>
<key>CFBundleURLTypes</key><array><dict>
<key>CFBundleURLSchemes</key><array><string>tech.mehran.allot</string></array>
</dict></array>
</dict></plist>"#,
        )
        .unwrap();

        let app = describe(PathBuf::from("Allot.app"), &read_info(&plist).unwrap()).unwrap();

        assert!(app.takes_hand_off);
        assert_eq!(read_info(&dir.path().join("missing.plist")), None);
    }
}
