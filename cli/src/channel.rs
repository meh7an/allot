//! Package-manager copies of `allot` on `PATH`: which channel one came from, and the notice for
//! one that is older than the install in `~/.allot/bin`, which `allot update` and `allot doctor`
//! both print. `~/.allot/bin` updates itself, but the copy a package manager put first on `PATH`
//! only changes when that package manager upgrades it.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use allot_shared::{paths::Paths, platform::CLI_EXE};
use allot_updater::Version;

use crate::guard::{self, Installed};

/// Set by the npm package's launcher (`npm/bin/allot.js`) on the allot it runs.
pub(crate) const NPM_LAUNCHER_ENV: &str = "ALLOT_NPM_LAUNCHER";
const CARGO_HOME_VAR: &str = "CARGO_HOME";

/// A package manager that puts its own `allot` on `PATH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Channel {
    Homebrew,
    Npm,
    Cargo,
}

impl Channel {
    fn name(self) -> &'static str {
        match self {
            Self::Homebrew => "Homebrew",
            Self::Npm => "npm",
            Self::Cargo => "cargo",
        }
    }

    /// The command that upgrades this channel's copy, as `docs/install.md` names it.
    pub(crate) fn upgrade(self) -> &'static str {
        match self {
            Self::Homebrew => "brew upgrade allot",
            Self::Npm => "npm install -g @meh7an/allot@latest",
            Self::Cargo => "cargo install --locked allot-cli allotd allot-mcp",
        }
    }
}

/// An `allot` from a package manager that is older than the one in `~/.allot/bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OlderCopy {
    pub(crate) channel: Channel,
    /// What `PATH` finds first: the file, or npm's launcher.
    pub(crate) found: PathBuf,
    pub(crate) version: Version,
    /// The version in `~/.allot/bin`.
    pub(crate) installed: Version,
}

impl OlderCopy {
    /// What is wrong, for `allot doctor`'s detail.
    pub(crate) fn detail(&self) -> String {
        format!(
            "allot on PATH is {} ({} {}), older than this install's {}",
            self.found.display(),
            self.channel.name(),
            self.version,
            self.installed
        )
    }

    /// What to say after an update.
    pub(crate) fn notice(&self) -> String {
        format!(
            "{}; upgrade it with `{}`",
            self.detail(),
            self.channel.upgrade()
        )
    }
}

/// The older package-manager copy a shell would run for `allot`, if there is one. `installed`
/// is the version in `~/.allot/bin`.
pub(crate) fn older_copy_on_path(paths: &Paths, installed: &Version) -> Option<OlderCopy> {
    if let Some(launcher) = env::var_os(NPM_LAUNCHER_ENV).filter(|value| !value.is_empty()) {
        return older_npm_copy(Path::new(&launcher), &guard::running_version(), installed);
    }
    let path = env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = env::split_paths(&path).collect();
    let found = first_on_path(&dirs, CLI_EXE)?;
    if is_this_install(&found, &paths.bin_dir().join(CLI_EXE)) {
        return None;
    }
    older_copy_of(&found, installed, guard::version_of)
}

/// The notice for [`older_copy_on_path`], when the install can be read.
pub(crate) fn notice(paths: &Paths) -> Option<String> {
    let Installed::Version(installed) = guard::installed(paths) else {
        return None;
    };
    older_copy_on_path(paths, &installed).map(|copy| copy.notice())
}

/// The npm copy that is running, which `launcher` started, when it is older than `installed`.
pub(crate) fn older_npm_copy(
    launcher: &Path,
    running: &Version,
    installed: &Version,
) -> Option<OlderCopy> {
    (running < installed).then(|| OlderCopy {
        channel: Channel::Npm,
        found: launcher.to_owned(),
        version: running.clone(),
        installed: installed.clone(),
    })
}

/// `found`, an `allot` on `PATH`, when a package manager put it there and it is older than
/// `installed`. `read_version` says what version a file is.
pub(crate) fn older_copy_of(
    found: &Path,
    installed: &Version,
    read_version: impl FnOnce(&Path) -> Option<Version>,
) -> Option<OlderCopy> {
    let channel = classify(&fs::canonicalize(found).ok()?, &cargo_bin())?;
    let version = read_version(found).filter(|version| version < installed)?;
    Some(OlderCopy {
        channel,
        found: found.to_owned(),
        version,
        installed: installed.clone(),
    })
}

/// Where `cargo install` puts executables.
fn cargo_bin() -> PathBuf {
    let home = env::var_os(CARGO_HOME_VAR)
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .or_else(|| {
            env::var_os(allot_shared::platform::HOME_VAR)
                .map(|home| PathBuf::from(home).join(".cargo"))
        })
        .unwrap_or_default();
    fs::canonicalize(home.join("bin")).unwrap_or_else(|_| home.join("bin"))
}

/// The channel of an `allot` whose real path is `resolved`: Homebrew keeps every formula in a
/// `Cellar` folder and links it into `bin`, and cargo writes to its own `bin` folder.
fn classify(resolved: &Path, cargo_bin: &Path) -> Option<Channel> {
    if resolved
        .components()
        .any(|part| part.as_os_str() == "Cellar")
    {
        return Some(Channel::Homebrew);
    }
    (resolved.parent() == Some(cargo_bin)).then_some(Channel::Cargo)
}

/// The first `name` in `dirs`, as a shell would find it.
pub(crate) fn first_on_path(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter()
        .map(|dir| dir.join(name))
        .find(|file| file.is_file())
}

/// Whether `found` is `installed`, directly or through a link such as `~/.local/bin/allot`.
pub(crate) fn is_this_install(found: &Path, installed: &Path) -> bool {
    let canonical = |path: &Path| path.canonicalize().ok();
    found == installed || canonical(found).is_some_and(|found| Some(found) == canonical(installed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn test_classify_finds_homebrew_by_its_cellar_folder() {
        let cargo = Path::new("/home/me/.cargo/bin");
        for path in [
            "/opt/homebrew/Cellar/allot/0.6.1/bin/allot",
            "/usr/local/Cellar/allot/0.6.1/bin/allot",
            "/home/linuxbrew/.linuxbrew/Cellar/allot/0.6.1/bin/allot",
        ] {
            assert_eq!(
                classify(Path::new(path), cargo),
                Some(Channel::Homebrew),
                "{path}"
            );
        }
    }

    #[test]
    fn test_classify_finds_cargo_by_its_bin_folder_only() {
        let cargo = Path::new("/home/me/.cargo/bin");
        assert_eq!(
            classify(Path::new("/home/me/.cargo/bin/allot"), cargo),
            Some(Channel::Cargo)
        );
        assert_eq!(classify(Path::new("/usr/bin/allot"), cargo), None);
        assert_eq!(
            classify(Path::new("/home/me/.cargo/bin/sub/allot"), cargo),
            None
        );
        assert_eq!(
            classify(Path::new("/home/me/.allot/bin/allot"), cargo),
            None
        );
    }

    #[test]
    fn test_upgrade_commands_are_the_ones_the_install_docs_name() {
        assert_eq!(Channel::Homebrew.upgrade(), "brew upgrade allot");
        assert_eq!(
            Channel::Npm.upgrade(),
            "npm install -g @meh7an/allot@latest"
        );
        assert_eq!(
            Channel::Cargo.upgrade(),
            "cargo install --locked allot-cli allotd allot-mcp"
        );
    }

    #[test]
    fn test_older_npm_copy_needs_the_running_allot_to_be_older() {
        let launcher = Path::new("/usr/lib/node_modules/@meh7an/allot/bin/allot.js");
        let older = older_npm_copy(launcher, &version("0.6.1"), &version("0.7.0")).unwrap();
        assert_eq!(older.channel, Channel::Npm);
        assert_eq!(older.version, version("0.6.1"));
        let notice = older.notice();
        assert!(notice.contains("npm 0.6.1"), "{notice}");
        assert!(notice.contains("0.7.0"), "{notice}");
        assert!(
            notice.ends_with("`npm install -g @meh7an/allot@latest`"),
            "{notice}"
        );

        assert_eq!(
            older_npm_copy(launcher, &version("0.7.0"), &version("0.7.0")),
            None
        );
        assert_eq!(
            older_npm_copy(launcher, &version("0.8.0"), &version("0.7.0")),
            None
        );
    }

    #[test]
    fn test_older_copy_of_a_file_no_package_manager_owns_is_none() {
        let dir = TempDir::new();
        let found = dir.path().join(CLI_EXE);
        fs::write(&found, "x").unwrap();

        let copy = older_copy_of(&found, &version("0.7.0"), |_| Some(version("0.1.0")));

        assert_eq!(copy, None);
    }

    #[test]
    fn test_older_copy_of_a_homebrew_file_compares_versions() {
        let dir = TempDir::new();
        let cellar = dir
            .path()
            .join("Cellar")
            .join("allot")
            .join("0.6.1")
            .join("bin");
        fs::create_dir_all(&cellar).unwrap();
        let found = cellar.join(CLI_EXE);
        fs::write(&found, "x").unwrap();

        let older = older_copy_of(&found, &version("0.7.0"), |_| Some(version("0.6.1"))).unwrap();
        assert_eq!(older.channel, Channel::Homebrew);
        assert_eq!(older.found, found);
        assert!(older.notice().contains("`brew upgrade allot`"));

        assert_eq!(
            older_copy_of(&found, &version("0.7.0"), |_| Some(version("0.7.0"))),
            None
        );
        assert_eq!(older_copy_of(&found, &version("0.7.0"), |_| None), None);
    }

    #[test]
    fn test_older_copy_of_a_missing_file_is_none() {
        let dir = TempDir::new();
        let found = dir.path().join("Cellar").join(CLI_EXE);
        assert_eq!(
            older_copy_of(&found, &version("0.7.0"), |_| Some(version("0.1.0"))),
            None
        );
    }

    #[test]
    fn test_first_on_path_takes_the_earliest_directory() {
        let root = TempDir::new();
        let (first, second) = (root.path().join("a"), root.path().join("b"));
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        fs::write(second.join(CLI_EXE), "").unwrap();
        let dirs = [root.path().join("missing"), first.clone(), second.clone()];
        assert_eq!(first_on_path(&dirs, CLI_EXE), Some(second.join(CLI_EXE)));
        fs::write(first.join(CLI_EXE), "").unwrap();
        assert_eq!(first_on_path(&dirs, CLI_EXE), Some(first.join(CLI_EXE)));
        assert!(is_this_install(
            &first.join(CLI_EXE),
            &first.join(".").join(CLI_EXE)
        ));
        assert!(!is_this_install(
            &first.join(CLI_EXE),
            &second.join(CLI_EXE)
        ));
    }
}
