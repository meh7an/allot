//! Linux: the GNOME extension and the KDE widget that Allot owns are replaced from the release,
//! and the ones a store owns are left alone. Neither shell loads the new files before the next
//! login, and Allot never restarts the desktop to make it (M0.3: restarting plasmashell hung a
//! panel, and gnome-shell cannot reload an extension).

use std::{fs, io::ErrorKind, path::Path, process::Command};

use allot_shared::{paths::Paths, release::Asset};
use allot_updater::{Release, Updater};
use serde_json::Value;

use crate::{
    desktop::{GNOME_EXTENSION_UUID, KDE_WIDGET_ID},
    xdg::data_home,
};

/// A desktop panel `allot update` can replace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Gnome,
    Kde,
}

/// What a panel's folder says about who put it there.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Owner {
    /// There is no copy for this user.
    Nobody,
    /// Copied from a release (or by hand), at `version` when it says.
    Allot { version: Option<String> },
    /// Installed from a store, which updates it.
    Store,
}

impl Panel {
    const ALL: [Self; 2] = [Self::Gnome, Self::Kde];

    fn name(self) -> &'static str {
        match self {
            Self::Gnome => "GNOME extension",
            Self::Kde => "KDE widget",
        }
    }

    fn asset(self) -> Asset {
        match self {
            Self::Gnome => Asset::GnomeExtension,
            Self::Kde => Asset::KdeWidget,
        }
    }

    /// The tool that installs a panel's file over the copy there.
    fn tool(self) -> &'static str {
        match self {
            Self::Gnome => "gnome-extensions",
            Self::Kde => "kpackagetool6",
        }
    }

    fn command(self, file: &Path) -> Command {
        let mut command = Command::new(self.tool());
        match self {
            Self::Gnome => command.args(["install", "--force"]),
            Self::Kde => command.args(["-t", "Plasma/Applet", "-u"]),
        };
        command.arg(file);
        command
    }

    /// What the user has to do before the new version runs.
    fn loads(self) -> &'static str {
        match self {
            Self::Gnome => "it loads at your next login",
            Self::Kde => "it loads at your next login; do not restart Plasma",
        }
    }

    fn owner(self, data: &Path) -> Owner {
        match self {
            Self::Gnome => gnome_owner(data),
            Self::Kde => kde_owner(data),
        }
    }
}

/// An extension copied from a release has `version-name` and no `version`: extensions.gnome.org
/// gives every upload an integer `version`, so one with that key came from the store, which
/// updates it.
fn gnome_owner(data: &Path) -> Owner {
    let folder = data
        .join("gnome-shell")
        .join("extensions")
        .join(GNOME_EXTENSION_UUID);
    if !folder.is_dir() {
        return Owner::Nobody;
    }
    let metadata = read_json(&folder.join("metadata.json"));
    if metadata
        .as_ref()
        .is_some_and(|json| json.get("version").is_some())
    {
        return Owner::Store;
    }
    let version = metadata
        .as_ref()
        .and_then(|json| json["version-name"].as_str())
        .map(str::to_owned);
    Owner::Allot { version }
}

/// A widget the KDE Store installed has an entry in KNewStuff's registry, which names the files it
/// put down.
fn kde_owner(data: &Path) -> Owner {
    let folder = data.join("plasma").join("plasmoids").join(KDE_WIDGET_ID);
    if !folder.is_dir() {
        return Owner::Nobody;
    }
    if registry_names_widget(&data.join("knewstuff3")) {
        return Owner::Store;
    }
    let version = read_json(&folder.join("metadata.json"))
        .and_then(|json| json["KPlugin"]["Version"].as_str().map(str::to_owned));
    Owner::Allot { version }
}

fn registry_names_widget(registry: &Path) -> bool {
    let Ok(entries) = fs::read_dir(registry) else {
        return false;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "knsregistry"))
        .filter_map(|path| fs::read_to_string(path).ok())
        .any(|text| text.contains(KDE_WIDGET_ID))
}

fn read_json(file: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(file).ok()?).ok()
}

/// What to do about each panel, given the version the release brings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Replace(Panel),
    LeaveToStore(Panel),
}

fn plan(data: &Path, release: &str) -> Vec<Step> {
    Panel::ALL
        .into_iter()
        .filter_map(|panel| match panel.owner(data) {
            Owner::Nobody => None,
            Owner::Store => Some(Step::LeaveToStore(panel)),
            Owner::Allot { version } if version.as_deref() == Some(release) => None,
            Owner::Allot { .. } => Some(Step::Replace(panel)),
        })
        .collect()
}

/// Replaces the panels of this user that Allot owns, and returns what to tell them about each,
/// for the outcome record and the panels' banners. A panel that cannot be replaced is a note,
/// not a failure of the update: the binaries are installed by then.
pub(super) fn update(paths: &Paths, updater: &Updater, release: &Release) -> Vec<String> {
    let version = release.version.to_string();
    plan(&data_home(paths), &version)
        .into_iter()
        .map(|step| match step {
            Step::LeaveToStore(panel) => format!(
                "the {} is left to the store it came from, which updates it",
                panel.name()
            ),
            Step::Replace(panel) => match replace(paths, updater, release, panel) {
                Ok(()) => format!("the {} is now {version}; {}", panel.name(), panel.loads()),
                Err(reason) => format!("the {} was not updated: {reason}", panel.name()),
            },
        })
        .collect()
}

fn replace(
    paths: &Paths,
    updater: &Updater,
    release: &Release,
    panel: Panel,
) -> Result<(), String> {
    let file_name = panel.asset().file_name(&release.version.to_string());
    println!("downloading {file_name}");
    let verified = updater
        .fetch(paths, release, panel.asset())
        .map_err(|error| error.to_string())?;
    let status = panel
        .command(verified.path())
        .status()
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => format!(
                "{} is not installed here; install {file_name} by hand",
                panel.tool()
            ),
            _ => format!("{} did not start ({error})", panel.tool()),
        })?;
    if !status.success() {
        return Err(format!("{} exited with {status}", panel.tool()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::test_support::TempDir;

    fn gnome_folder(data: &Path) -> PathBuf {
        let folder = data
            .join("gnome-shell")
            .join("extensions")
            .join(GNOME_EXTENSION_UUID);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn kde_folder(data: &Path) -> PathBuf {
        let folder = data.join("plasma").join("plasmoids").join(KDE_WIDGET_ID);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn test_plan_with_no_panels_installed_has_nothing_to_do() {
        let data = TempDir::new();
        assert_eq!(plan(data.path(), "0.7.0"), []);
    }

    #[test]
    fn test_plan_replaces_a_gnome_extension_copied_from_a_release() {
        let data = TempDir::new();
        fs::write(
            gnome_folder(data.path()).join("metadata.json"),
            r#"{"uuid":"allot@mehran.tech","version-name":"0.6.1"}"#,
        )
        .unwrap();

        assert_eq!(plan(data.path(), "0.7.0"), [Step::Replace(Panel::Gnome)]);
    }

    #[test]
    fn test_plan_leaves_a_gnome_extension_with_a_store_version_to_its_store() {
        let data = TempDir::new();
        fs::write(
            gnome_folder(data.path()).join("metadata.json"),
            r#"{"uuid":"allot@mehran.tech","version":3,"version-name":"0.6.1"}"#,
        )
        .unwrap();

        assert_eq!(
            plan(data.path(), "0.7.0"),
            [Step::LeaveToStore(Panel::Gnome)]
        );
    }

    #[test]
    fn test_plan_replaces_a_gnome_extension_whose_metadata_cannot_be_read() {
        let data = TempDir::new();
        fs::write(
            gnome_folder(data.path()).join("metadata.json"),
            "{ not json",
        )
        .unwrap();

        assert_eq!(plan(data.path(), "0.7.0"), [Step::Replace(Panel::Gnome)]);
    }

    #[test]
    fn test_plan_skips_a_panel_that_is_already_at_the_release() {
        let data = TempDir::new();
        fs::write(
            gnome_folder(data.path()).join("metadata.json"),
            r#"{"version-name":"0.7.0"}"#,
        )
        .unwrap();
        fs::write(
            kde_folder(data.path()).join("metadata.json"),
            r#"{"KPlugin":{"Version":"0.7.0"}}"#,
        )
        .unwrap();

        assert_eq!(plan(data.path(), "0.7.0"), []);
    }

    #[test]
    fn test_plan_replaces_a_kde_widget_with_no_registry_entry() {
        let data = TempDir::new();
        fs::write(
            kde_folder(data.path()).join("metadata.json"),
            r#"{"KPlugin":{"Id":"tech.mehran.allot","Version":"0.6.1"}}"#,
        )
        .unwrap();
        let registry = data.path().join("knewstuff3");
        fs::create_dir_all(&registry).unwrap();
        fs::write(
            registry.join("plasmoids.knsregistry"),
            "<hotnewstuffregistry><stuff name=\"Another widget\"/></hotnewstuffregistry>",
        )
        .unwrap();

        assert_eq!(plan(data.path(), "0.7.0"), [Step::Replace(Panel::Kde)]);
    }

    #[test]
    fn test_plan_leaves_a_kde_widget_a_registry_entry_names_to_the_store() {
        let data = TempDir::new();
        kde_folder(data.path());
        let registry = data.path().join("knewstuff3");
        fs::create_dir_all(&registry).unwrap();
        fs::write(
            registry.join("plasmoids.knsregistry"),
            "<stuff><installedfile>/home/me/.local/share/plasma/plasmoids/tech.mehran.allot/x\
             </installedfile></stuff>",
        )
        .unwrap();

        assert_eq!(plan(data.path(), "0.7.0"), [Step::LeaveToStore(Panel::Kde)]);
    }

    #[test]
    fn test_plan_covers_both_panels_in_a_fixed_order() {
        let data = TempDir::new();
        gnome_folder(data.path());
        kde_folder(data.path());

        assert_eq!(
            plan(data.path(), "0.7.0"),
            [Step::Replace(Panel::Gnome), Step::Replace(Panel::Kde)]
        );
    }

    #[test]
    fn test_commands_are_the_tools_m0_3_proved() {
        let file = Path::new("panel-file");
        let args = |command: &Command| -> Vec<String> {
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect()
        };

        let gnome = Panel::Gnome.command(file);
        assert_eq!(gnome.get_program(), "gnome-extensions");
        assert_eq!(args(&gnome), ["install", "--force", "panel-file"]);

        let kde = Panel::Kde.command(file);
        assert_eq!(kde.get_program(), "kpackagetool6");
        assert_eq!(args(&kde), ["-t", "Plasma/Applet", "-u", "panel-file"]);
    }

    #[test]
    fn test_panels_say_they_load_at_the_next_login_and_never_offer_a_restart() {
        for panel in Panel::ALL {
            assert!(panel.loads().contains("next login"), "{}", panel.loads());
        }
        assert!(Panel::Kde.loads().contains("do not restart Plasma"));
    }
}
