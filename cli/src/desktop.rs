//! What `allot install` and `allot uninstall` do for the Linux desktops: the `allot` icon that
//! the KDE widget and its notifications name, the hint about the panel package, and what the
//! widget leaves behind, since removing a widget package cannot.

use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use allot_shared::{autostart::CONFIG_HOME_VAR, paths::Paths};

use crate::install::remove_if_present;

const DATA_HOME_VAR: &str = "XDG_DATA_HOME";
const DESKTOP_VAR: &str = "XDG_CURRENT_DESKTOP";
/// The file the KDE widget copies into `knotifications6` at start, and the name of the user's
/// choices for it, which System Settings writes to the config folder.
const NOTIFY_RC: &str = "allot.notifyrc";
const RELEASE_URL: &str = concat!(
    "https://github.com/meh7an/allot/releases/tag/v",
    env!("CARGO_PKG_VERSION")
);

/// Pairs a path under `icons/hicolor` with its bytes from the brand folder, which `build.py`
/// writes. In the binary like the skill, so a source build installs them too.
macro_rules! icon {
    ($size:literal, $file:literal) => {
        (
            concat!($size, "/apps/", $file),
            include_bytes!(concat!(
                "../icons/hicolor/",
                $size,
                "/apps/",
                $file
            )) as &[u8],
        )
    };
}

const ICONS: [(&str, &[u8]); 10] = [
    icon!("16x16", "allot.png"),
    icon!("22x22", "allot.png"),
    icon!("24x24", "allot.png"),
    icon!("32x32", "allot.png"),
    icon!("48x48", "allot.png"),
    icon!("64x64", "allot.png"),
    icon!("128x128", "allot.png"),
    icon!("256x256", "allot.png"),
    icon!("512x512", "allot.png"),
    icon!("scalable", "allot.svg"),
];

/// Installs the icon. Reported, not returned: the install works without it.
pub fn install(paths: &Paths) {
    let icons = hicolor(&data_home(paths));
    match write_icons(&icons) {
        Ok(()) => println!("installed the allot icon into {}", icons.display()),
        Err(error) => println!("warning: the allot icon was not installed ({error})"),
    }
}

/// Removes the icon and the widget's notifications file, and with `purge` the user's choices
/// for those notifications. Reported, not returned: nothing else depends on them.
pub fn uninstall(paths: &Paths, purge: bool) {
    let data = data_home(paths);
    let mut removed = remove_icons(&hicolor(&data));
    removed = removed.and(remove_if_present(
        &data.join("knotifications6").join(NOTIFY_RC),
    ));
    if purge {
        removed = removed.and(remove_if_present(&config_home(paths).join(NOTIFY_RC)));
    }
    if let Err(error) = removed {
        println!("warning: the desktop files were not all removed ({error:#})");
    }
}

/// Says how to get the panel package of the desktop running here, unless it is installed.
pub fn print_panel_hint(paths: &Paths) {
    let desktop = env::var(DESKTOP_VAR).unwrap_or_default();
    if let Some(hint) = panel_hint(&desktop, &data_home(paths)) {
        println!("{hint}");
    }
}

/// `$XDG_DATA_HOME` when it is absolute, as the base directory spec asks, else `~/.local/share`.
fn data_home(paths: &Paths) -> PathBuf {
    xdg_home(
        env::var_os(DATA_HOME_VAR).as_deref(),
        paths.home(),
        ".local/share",
    )
}

fn config_home(paths: &Paths) -> PathBuf {
    xdg_home(
        env::var_os(CONFIG_HOME_VAR).as_deref(),
        paths.home(),
        ".config",
    )
}

fn xdg_home(value: Option<&OsStr>, home: &Path, default: &str) -> PathBuf {
    value
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .map_or_else(|| home.join(default), Path::to_owned)
}

fn hicolor(data: &Path) -> PathBuf {
    data.join("icons").join("hicolor")
}

fn write_icons(hicolor: &Path) -> std::io::Result<()> {
    for (path, bytes) in ICONS {
        let file = hicolor.join(path);
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(file, bytes)?;
    }
    Ok(())
}

/// Removes the icon files and the folders that leaves empty; other apps' icons stay.
fn remove_icons(hicolor: &Path) -> anyhow::Result<()> {
    for (path, _) in ICONS {
        let file = hicolor.join(path);
        remove_if_present(&file)?;
        let apps = file.parent().map(Path::to_owned);
        for dir in apps.iter().flat_map(|apps| apps.ancestors().take(2)) {
            let _ = fs::remove_dir(dir);
        }
    }
    let _ = fs::remove_dir(hicolor);
    Ok(())
}

/// The hint for `desktop`, the value of `$XDG_CURRENT_DESKTOP`: a colon-separated list such as
/// `ubuntu:GNOME`. `None` on other desktops and when the package is already in `data`.
fn panel_hint(desktop: &str, data: &Path) -> Option<String> {
    let names = || desktop.split(':').map(str::to_ascii_uppercase);
    if names().any(|name| name == "GNOME") {
        let installed = data.join("gnome-shell/extensions/allot@mehran.tech");
        return (!installed.exists()).then(|| {
            format!(
                "for the panel extension, get the gnome-extension.zip from {RELEASE_URL}, run \
                 `gnome-extensions install --force <zip>`, log out and in, and run \
                 `gnome-extensions enable allot@mehran.tech`"
            )
        });
    }
    if names().any(|name| name == "KDE") {
        let installed = data.join("plasma/plasmoids/tech.mehran.allot");
        return (!installed.exists()).then(|| {
            format!(
                "for the panel widget, get the kde-widget.plasmoid from {RELEASE_URL}, run \
                 `kpackagetool6 -t Plasma/Applet -i <file>`, and add Allot with Add Widgets"
            )
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use std::process;

    use super::*;

    struct Folder(PathBuf);

    impl Folder {
        fn new(name: &str) -> Self {
            let root = env::temp_dir().join(format!("allot-desktop-{}-{name}", process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn files_under(dir: &Path) -> usize {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .map(|path| if path.is_dir() { files_under(&path) } else { 1 })
            .sum()
    }

    #[test]
    fn test_icons_table_lists_every_file_in_the_brand_folder() {
        let brand = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/hicolor");

        assert_eq!(files_under(&brand), ICONS.len());
    }

    #[test]
    fn test_write_icons_writes_each_file_under_hicolor() {
        let folder = Folder::new("write");

        write_icons(&folder.0).unwrap();

        assert_eq!(files_under(&folder.0), ICONS.len());
        assert!(folder.0.join("scalable/apps/allot.svg").is_file());
        assert!(folder.0.join("48x48/apps/allot.png").is_file());
    }

    #[test]
    fn test_remove_icons_keeps_other_apps_icons() {
        let folder = Folder::new("remove");
        let hicolor = folder.0.join("hicolor");
        write_icons(&hicolor).unwrap();
        let other = hicolor.join("48x48/apps/other.png");
        fs::write(&other, "other").unwrap();

        remove_icons(&hicolor).unwrap();

        assert!(other.is_file());
        assert!(!hicolor.join("48x48/apps/allot.png").exists());
        assert!(!hicolor.join("scalable").exists());
    }

    #[test]
    fn test_remove_icons_with_nothing_else_removes_hicolor() {
        let folder = Folder::new("remove-all");
        let hicolor = folder.0.join("hicolor");
        write_icons(&hicolor).unwrap();

        remove_icons(&hicolor).unwrap();

        assert!(!hicolor.exists());
    }

    #[test]
    fn test_remove_icons_with_no_icons_succeeds() {
        let folder = Folder::new("remove-none");

        remove_icons(&folder.0.join("hicolor")).unwrap();
    }

    #[test]
    fn test_xdg_home_with_relative_value_uses_the_default() {
        let home = Path::new("/home/me");

        assert_eq!(
            xdg_home(Some(OsStr::new("relative")), home, ".local/share"),
            home.join(".local/share")
        );
        assert_eq!(xdg_home(None, home, ".config"), home.join(".config"));
    }

    #[test]
    fn test_xdg_home_with_absolute_value_uses_it() {
        let value = env::temp_dir();

        assert_eq!(
            xdg_home(
                Some(value.as_os_str()),
                Path::new("/home/me"),
                ".local/share"
            ),
            value
        );
    }

    #[test]
    fn test_panel_hint_on_gnome_names_the_extension_commands() {
        let folder = Folder::new("hint-gnome");

        let hint = panel_hint("ubuntu:GNOME", &folder.0).unwrap();

        assert!(hint.contains("gnome-extensions install --force"), "{hint}");
        assert!(hint.contains(RELEASE_URL), "{hint}");
    }

    #[test]
    fn test_panel_hint_on_kde_names_the_widget_commands() {
        let folder = Folder::new("hint-kde");

        let hint = panel_hint("KDE", &folder.0).unwrap();

        assert!(hint.contains("kpackagetool6 -t Plasma/Applet -i"), "{hint}");
    }

    #[test]
    fn test_panel_hint_with_the_package_installed_says_nothing() {
        let folder = Folder::new("hint-installed");
        fs::create_dir_all(folder.0.join("plasma/plasmoids/tech.mehran.allot")).unwrap();
        fs::create_dir_all(folder.0.join("gnome-shell/extensions/allot@mehran.tech")).unwrap();

        assert_eq!(panel_hint("KDE", &folder.0), None);
        assert_eq!(panel_hint("GNOME", &folder.0), None);
    }

    #[test]
    fn test_panel_hint_on_other_desktops_says_nothing() {
        let folder = Folder::new("hint-other");

        assert_eq!(panel_hint("XFCE", &folder.0), None);
        assert_eq!(panel_hint("", &folder.0), None);
    }
}
