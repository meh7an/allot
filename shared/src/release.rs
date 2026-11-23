//! The names of the files a release carries and of its tag. The updater, `allot update` and the
//! panels' hand-offs take every name from here, and the tests hold them to what the release and
//! package workflows build, so a rename shows up as a failing test rather than a 404 on a user's
//! machine.
//!
//! A `version` is the one in `Cargo.toml`, without the leading `v`: `0.6.1`.

use std::env::consts::{ARCH, OS};

/// The ending of a checksum file's name, appended to the name of the file it covers.
pub const CHECKSUM_SUFFIX: &str = ".sha256";

/// The prefix of every file name a release carries.
const PREFIX: &str = "allot";

/// A platform the release builds `allot`, `allotd` and `allot-mcp` for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Target {
    /// 64-bit Windows on x86.
    WindowsX64,
    /// 64-bit Linux on x86 with glibc.
    LinuxX64,
    /// 64-bit Linux on ARM with glibc.
    LinuxArm64,
    /// macOS on Apple Silicon.
    MacosArm64,
}

impl Target {
    /// Every platform with a release archive.
    pub const ALL: [Self; 4] = [
        Self::WindowsX64,
        Self::LinuxX64,
        Self::LinuxArm64,
        Self::MacosArm64,
    ];

    /// The Rust target triple the archive is named for.
    pub const fn triple(self) -> &'static str {
        match self {
            Self::WindowsX64 => "x86_64-pc-windows-msvc",
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
            Self::LinuxArm64 => "aarch64-unknown-linux-gnu",
            Self::MacosArm64 => "aarch64-apple-darwin",
        }
    }

    /// What this program runs on, or `None` where the release has no archive for it.
    pub fn current() -> Option<Self> {
        match (OS, ARCH) {
            ("windows", "x86_64") => Some(Self::WindowsX64),
            ("linux", "x86_64") => Some(Self::LinuxX64),
            ("linux", "aarch64") => Some(Self::LinuxArm64),
            ("macos", "aarch64") => Some(Self::MacosArm64),
            _ => None,
        }
    }

    /// How the archive is packed: a zip on Windows, a gzipped tarball elsewhere.
    pub const fn archive_extension(self) -> &'static str {
        match self {
            Self::WindowsX64 => "zip",
            Self::LinuxX64 | Self::LinuxArm64 | Self::MacosArm64 => "tar.gz",
        }
    }

    /// The folder at the root of the archive, which holds the executables, and the archive's
    /// name without its extension.
    pub fn folder_name(self, version: &str) -> String {
        format!("{PREFIX}-{version}-{}", self.triple())
    }
}

/// One file of a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Asset {
    /// The executables and `allot install`'s payload for one platform.
    Archive(Target),
    /// The Windows installer, which also holds the tray app.
    Setup,
    /// `Allot.app` zipped, which the macOS app updates itself from.
    MacApp,
    /// The disk image users drag `Allot.app` out of.
    Dmg,
    /// The GNOME Shell extension, for `gnome-extensions install`.
    GnomeExtension,
    /// The KDE Plasma widget, for `kpackagetool6`.
    KdeWidget,
}

impl Asset {
    /// Every file a release carries, each without its checksum.
    pub const ALL: [Self; 9] = [
        Self::Archive(Target::WindowsX64),
        Self::Archive(Target::LinuxX64),
        Self::Archive(Target::LinuxArm64),
        Self::Archive(Target::MacosArm64),
        Self::Setup,
        Self::MacApp,
        Self::Dmg,
        Self::GnomeExtension,
        Self::KdeWidget,
    ];

    /// The file's name in the release of `version`.
    pub fn file_name(self, version: &str) -> String {
        match self {
            Self::Archive(target) => {
                format!(
                    "{}.{}",
                    target.folder_name(version),
                    target.archive_extension()
                )
            }
            Self::Setup => format!("{PREFIX}-{version}-setup.exe"),
            Self::MacApp => format!("{PREFIX}-{version}-macos-app.zip"),
            Self::Dmg => format!("{PREFIX}-{version}.dmg"),
            Self::GnomeExtension => format!("{PREFIX}-{version}-gnome-extension.zip"),
            Self::KdeWidget => format!("{PREFIX}-{version}-kde-widget.plasmoid"),
        }
    }

    /// The name of the checksum file that sits beside [`file_name`](Self::file_name).
    pub fn checksum_name(self, version: &str) -> String {
        format!("{}{CHECKSUM_SUFFIX}", self.file_name(version))
    }
}

/// The git tag, and so the release, of `version`.
pub fn tag(version: &str) -> String {
    format!("v{version}")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    #[test]
    fn test_asset_file_names_match_the_v0_6_1_release() {
        let expected = [
            "allot-0.6.1-x86_64-pc-windows-msvc.zip",
            "allot-0.6.1-x86_64-unknown-linux-gnu.tar.gz",
            "allot-0.6.1-aarch64-unknown-linux-gnu.tar.gz",
            "allot-0.6.1-aarch64-apple-darwin.tar.gz",
            "allot-0.6.1-setup.exe",
            "allot-0.6.1-macos-app.zip",
            "allot-0.6.1.dmg",
            "allot-0.6.1-gnome-extension.zip",
            "allot-0.6.1-kde-widget.plasmoid",
        ];
        let names: Vec<String> = Asset::ALL
            .iter()
            .map(|asset| asset.file_name("0.6.1"))
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_asset_checksum_name_adds_the_suffix_to_the_file_name() {
        for asset in Asset::ALL {
            assert_eq!(
                asset.checksum_name("0.6.1"),
                format!("{}.sha256", asset.file_name("0.6.1"))
            );
        }
    }

    #[test]
    fn test_asset_file_names_are_all_different() {
        let mut names: Vec<String> = Asset::ALL
            .iter()
            .flat_map(|asset| [asset.file_name("1.2.3"), asset.checksum_name("1.2.3")])
            .collect();
        let total = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), total);
    }

    #[test]
    fn test_asset_file_name_keeps_a_pre_release_suffix() {
        assert_eq!(
            Asset::Setup.file_name("0.7.0-beta.1"),
            "allot-0.7.0-beta.1-setup.exe"
        );
    }

    #[test]
    fn test_target_folder_name_is_the_archive_name_without_its_extension() {
        for target in Target::ALL {
            let archive = Asset::Archive(target).file_name("0.6.1");
            let stem = archive.strip_suffix(&format!(".{}", target.archive_extension()));
            assert_eq!(stem, Some(target.folder_name("0.6.1").as_str()));
        }
    }

    #[test]
    fn test_target_current_names_the_architecture_this_test_runs_on() {
        if let Some(target) = Target::current() {
            assert!(target.triple().starts_with(ARCH));
        }
    }

    #[test]
    fn test_tag_prefixes_the_version_with_v() {
        assert_eq!(tag("0.6.1"), "v0.6.1");
    }

    fn workflow(file: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(".github")
            .join("workflows")
            .join(file);
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
    }

    /// Fails unless `file` spells `expected`, which the names above say it should.
    fn assert_spells(workflows: &str, file: &str, expected: &str) {
        assert!(
            workflows.contains(expected),
            "{file} no longer spells `{expected}`; release.rs and the workflow disagree"
        );
    }

    /// How a workflow refers to what it builds: the file it is in, how it writes the version,
    /// and the variable that holds the file's name when it writes the checksum.
    #[derive(Debug)]
    struct Spelling {
        asset: Asset,
        workflow: &'static str,
        version: &'static str,
        name_variable: &'static str,
    }

    const SPELLINGS: [Spelling; 9] = [
        Spelling {
            asset: Asset::Archive(Target::WindowsX64),
            workflow: "release.yml",
            version: "$env:VERSION",
            name_variable: "$name",
        },
        Spelling {
            asset: Asset::Archive(Target::LinuxX64),
            workflow: "package-linux.yml",
            version: "$version",
            name_variable: "$name",
        },
        Spelling {
            asset: Asset::Archive(Target::LinuxArm64),
            workflow: "package-linux.yml",
            version: "$version",
            name_variable: "$name",
        },
        Spelling {
            asset: Asset::Archive(Target::MacosArm64),
            workflow: "package-macos.yml",
            version: "$VERSION",
            name_variable: "$name",
        },
        Spelling {
            asset: Asset::Setup,
            workflow: "release.yml",
            version: "$env:VERSION",
            name_variable: "$setup",
        },
        Spelling {
            asset: Asset::MacApp,
            workflow: "package-macos.yml",
            version: "$VERSION",
            name_variable: "$asset",
        },
        Spelling {
            asset: Asset::Dmg,
            workflow: "package-macos.yml",
            version: "$VERSION",
            name_variable: "$dmg",
        },
        Spelling {
            asset: Asset::GnomeExtension,
            workflow: "package-linux.yml",
            version: "$version",
            name_variable: "$asset",
        },
        Spelling {
            asset: Asset::KdeWidget,
            workflow: "package-linux.yml",
            version: "$version",
            name_variable: "$asset",
        },
    ];

    #[test]
    fn test_workflow_spellings_cover_every_asset() {
        let covered: Vec<Asset> = SPELLINGS.iter().map(|spelling| spelling.asset).collect();
        assert_eq!(covered, Asset::ALL);
    }

    #[test]
    fn test_workflows_build_the_files_that_are_not_archives() {
        for spelling in SPELLINGS
            .iter()
            .filter(|spelling| !matches!(spelling.asset, Asset::Archive(_)))
        {
            let text = workflow(spelling.workflow);
            let file = spelling.asset.file_name(spelling.version);
            assert_spells(&text, spelling.workflow, &format!("\"{file}\""));
            assert_spells(
                &text,
                spelling.workflow,
                &format!("{}{CHECKSUM_SUFFIX}", spelling.name_variable),
            );
        }
    }

    #[test]
    fn test_workflows_build_the_archives_and_run_on_their_target() {
        let archives = SPELLINGS
            .iter()
            .filter_map(|spelling| match spelling.asset {
                Asset::Archive(target) => Some((spelling, target)),
                _ => None,
            });
        for (spelling, target) in archives {
            let text = workflow(spelling.workflow);
            let folder = target.folder_name(spelling.version);
            let extension = target.archive_extension();
            let variable = spelling.name_variable;
            // Only Windows spells its triple in the name; the Unix packages take it from a
            // variable that a matrix entry or the job's env sets.
            let name = if target == Target::WindowsX64 {
                folder
            } else {
                folder.replace(target.triple(), "$TARGET")
            };
            assert_spells(&text, spelling.workflow, &format!("\"{name}\""));
            assert_spells(&text, spelling.workflow, &format!("{variable}.{extension}"));
            assert_spells(
                &text,
                spelling.workflow,
                &format!("{variable}.{extension}{CHECKSUM_SUFFIX}"),
            );
            if target != Target::WindowsX64 {
                let triple = target.triple();
                assert!(
                    text.contains(&format!("target: {triple}"))
                        || text.contains(&format!("TARGET: {triple}")),
                    "{} does not build for {triple}",
                    spelling.workflow
                );
            }
        }
    }

    #[test]
    fn test_workflows_upload_the_files_by_the_names_release_rs_gives() {
        for (asset, workflow_file) in [
            (Asset::MacApp, "package-macos.yml"),
            (Asset::Dmg, "package-macos.yml"),
            (Asset::GnomeExtension, "package-linux.yml"),
            (Asset::KdeWidget, "package-linux.yml"),
        ] {
            let text = workflow(workflow_file);
            assert_spells(&text, workflow_file, &asset.file_name("*"));
            assert_spells(&text, workflow_file, &asset.checksum_name("*"));
        }
    }

    #[test]
    fn test_release_workflow_checks_the_tag_this_module_builds() {
        let text = workflow("release.yml");
        assert_spells(
            &text,
            "release.yml",
            &format!("\"{}\" -ne $env:GITHUB_REF_NAME", tag("$version")),
        );
    }
}
