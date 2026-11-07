//! The XDG base directories `allot install` writes to on Linux and macOS: the icon and the KDE
//! widget's files, and the bash and fish completions.

use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use allot_shared::{autostart::CONFIG_HOME_VAR, paths::Paths};

const DATA_HOME_VAR: &str = "XDG_DATA_HOME";

/// `$XDG_DATA_HOME` when it is absolute, as the base directory spec asks, else `~/.local/share`.
pub fn data_home(paths: &Paths) -> PathBuf {
    xdg_home(
        env::var_os(DATA_HOME_VAR).as_deref(),
        paths.home(),
        ".local/share",
    )
}

/// `$XDG_CONFIG_HOME` when it is absolute, else `~/.config`.
pub fn config_home(paths: &Paths) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
