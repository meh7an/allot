use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use allot_shared::release::Asset;
use flate2::read::GzDecoder;
use tar::{Archive, Entry, EntryType};

use crate::{
    error::{Result, UnsafeReason, UpdateError},
    fetch::Verified,
    updater::Updater,
};

/// What the tar headers and padding of one entry may add to the data, when the unpacked size
/// is capped.
const HEADER_ALLOWANCE: u64 = 2048;

impl Updater {
    /// Unpacks a verified release archive next to itself and returns the folder at its root,
    /// `allot-<version>-<triple>`, which holds `allot`, `allotd` and `allot-mcp`.
    ///
    /// Every entry is checked before anything is written for it. Only files, folders and links
    /// that stay inside the root folder are unpacked: a path that is absolute, climbs with
    /// `..` or leaves the root folder is refused, and so is a link that points absolutely,
    /// upwards or at a file outside it, or any other kind of entry. Links must therefore only
    /// descend, which keeps a chain of them inside the folder too. Permissions are reduced to
    /// 0755 or 0644, so no setuid bit and no write access for others survives. A folder from an
    /// earlier unpack is replaced, and a failed unpack leaves none.
    ///
    /// # Errors
    ///
    /// Fails with [`UpdateError::NotATarball`] for any file but a release's `.tar.gz`, with
    /// [`UpdateError::UnsafeEntry`] for a refused entry or a limit crossed, and with
    /// [`UpdateError::BadArchive`] when the archive is damaged.
    pub fn unpack(&self, verified: &Verified) -> Result<PathBuf> {
        let version = verified.version.to_string();
        let file = verified.asset.file_name(&version);
        let Asset::Archive(target) = verified.asset else {
            return Err(UpdateError::NotATarball { file });
        };
        let folder = verified
            .path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        if target.archive_extension() != "tar.gz" {
            return Err(UpdateError::NotATarball { file });
        }
        let root = target.folder_name(&version);
        let root_dir = folder.join(&root);
        remove_if_present(&root_dir)?;
        let result = self.extract(verified, &file, &folder, &root);
        if result.is_err() {
            // Whatever was unpacked before the refusal must not stay behind.
            let _ = fs::remove_dir_all(&root_dir);
        }
        result.map(|()| root_dir)
    }

    fn extract(&self, verified: &Verified, file: &str, folder: &Path, root: &str) -> Result<()> {
        let source = File::open(&verified.path)
            .map_err(|error| UpdateError::io("open", &verified.path, error))?;
        let cap = (self.limits.max_entries as u64)
            .saturating_mul(HEADER_ALLOWANCE)
            .saturating_add(self.limits.max_unpacked_bytes);
        let mut archive = Archive::new(GzDecoder::new(source).take(cap));
        let mut unpacker = Unpacker {
            file,
            folder,
            root,
            entries_left: self.limits.max_entries,
            bytes_left: self.limits.max_unpacked_bytes,
        };
        for entry in archive
            .entries()
            .map_err(|error| bad_archive(file, &error))?
        {
            let entry = entry.map_err(|error| bad_archive(file, &error))?;
            unpacker.unpack(entry)?;
        }
        if !folder.join(root).is_dir() {
            return Err(UpdateError::BadArchive {
                file: file.to_owned(),
                reason: format!("it has no {root} folder"),
            });
        }
        Ok(())
    }
}

struct Unpacker<'a> {
    file: &'a str,
    folder: &'a Path,
    root: &'a str,
    entries_left: usize,
    bytes_left: u64,
}

impl Unpacker<'_> {
    fn unpack<R: Read>(&mut self, mut entry: Entry<'_, R>) -> Result<()> {
        let kind = entry.header().entry_type();
        if kind == EntryType::XGlobalHeader {
            return Ok(());
        }
        let raw = entry
            .path()
            .map_err(|error| bad_archive(self.file, &error))?
            .into_owned();
        let shown = raw.to_string_lossy().into_owned();
        let refuse = |reason| UpdateError::UnsafeEntry {
            path: shown.clone(),
            reason,
        };

        self.entries_left = self
            .entries_left
            .checked_sub(1)
            .ok_or_else(|| refuse(UnsafeReason::TooManyEntries))?;
        let Some(relative) = inside_root(&raw, self.root).map_err(&refuse)? else {
            // The folder the archive is unpacked into, which exists already.
            return if kind == EntryType::Directory {
                Ok(())
            } else {
                Err(refuse(UnsafeReason::OddName))
            };
        };
        let dest = self.folder.join(&relative);

        match kind {
            EntryType::Directory => {
                fs::create_dir_all(&dest).map_err(|error| UpdateError::io("create", &dest, error))
            }
            EntryType::Regular | EntryType::Continuous => {
                let size = entry.size();
                self.bytes_left = self
                    .bytes_left
                    .checked_sub(size)
                    .ok_or_else(|| refuse(UnsafeReason::TooMuchData))?;
                let executable = entry.header().mode().is_ok_and(|mode| mode & 0o111 != 0);
                let mut file = create_new(&dest, &refuse)?;
                io::copy(&mut entry, &mut file).map_err(|error| match error.kind() {
                    io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => {
                        bad_archive(self.file, &error)
                    }
                    _ => UpdateError::io("write", &dest, error),
                })?;
                restrict(&file, executable, &dest)
            }
            EntryType::Symlink => {
                let target = link_target(&entry, self.file)?;
                if !descends(&target) {
                    return Err(refuse(UnsafeReason::LinkOutside));
                }
                symlink(&target, &dest, &refuse)
            }
            EntryType::Link => {
                let target = link_target(&entry, self.file)?;
                let source = match inside_root(&target, self.root) {
                    Ok(Some(relative)) => self.folder.join(relative),
                    _ => return Err(refuse(UnsafeReason::LinkOutside)),
                };
                hard_link(&source, &dest, &refuse)
            }
            _ => Err(refuse(UnsafeReason::UnsupportedType)),
        }
    }
}

/// `path` without its `.` parts, when it is plain, relative, stays inside and starts with the
/// archive's root folder; `None` for the extraction folder itself.
fn inside_root(path: &Path, root: &str) -> Result<Option<PathBuf>, UnsafeReason> {
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => return Err(UnsafeReason::Absolute),
            Component::ParentDir => return Err(UnsafeReason::ParentDir),
            Component::CurDir => {}
            Component::Normal(name) if is_plain(name) => relative.push(name),
            Component::Normal(_) => return Err(UnsafeReason::OddName),
        }
    }
    match relative.components().next() {
        None => Ok(None),
        Some(first) if first.as_os_str() == root => Ok(Some(relative)),
        Some(_) => Err(UnsafeReason::OutsideRoot),
    }
}

/// `true` for a symlink target that only goes down from the link's own folder: no root, no
/// drive, no `..`. Such a link, and any chain of them, cannot reach outside the folder the link
/// is in, which a lexical check of `..` cannot promise once a link in the path is followed.
fn descends(target: &Path) -> bool {
    let mut any = false;
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(name) if is_plain(name) => any = true,
            _ => return false,
        }
    }
    any
}

/// A part of a path that is readable text and cannot name a drive, a stream or, on Windows, a
/// second level.
fn is_plain(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|text| !text.is_empty() && !text.contains([':', '\\']))
}

fn link_target<R: Read>(entry: &Entry<'_, R>, file: &str) -> Result<PathBuf> {
    let target = entry
        .link_name()
        .map_err(|error| bad_archive(file, &error))?;
    Ok(target.map(|target| target.into_owned()).unwrap_or_default())
}

fn create_new(dest: &Path, refuse: &impl Fn(UnsafeReason) -> UpdateError) -> Result<File> {
    create_parents(dest)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|error| creation_error(dest, error, refuse))
}

fn create_parents(dest: &Path) -> Result<()> {
    match dest.parent() {
        Some(parent) => {
            fs::create_dir_all(parent).map_err(|error| UpdateError::io("create", parent, error))
        }
        None => Ok(()),
    }
}

fn creation_error(
    dest: &Path,
    error: io::Error,
    refuse: &impl Fn(UnsafeReason) -> UpdateError,
) -> UpdateError {
    if error.kind() == io::ErrorKind::AlreadyExists {
        refuse(UnsafeReason::Duplicate)
    } else {
        UpdateError::io("create", dest, error)
    }
}

#[cfg(unix)]
fn symlink(
    target: &Path,
    dest: &Path,
    refuse: &impl Fn(UnsafeReason) -> UpdateError,
) -> Result<()> {
    create_parents(dest)?;
    std::os::unix::fs::symlink(target, dest).map_err(|error| creation_error(dest, error, refuse))
}

#[cfg(not(unix))]
fn symlink(
    _target: &Path,
    _dest: &Path,
    refuse: &impl Fn(UnsafeReason) -> UpdateError,
) -> Result<()> {
    Err(refuse(UnsafeReason::LinksUnsupported))
}

fn hard_link(
    source: &Path,
    dest: &Path,
    refuse: &impl Fn(UnsafeReason) -> UpdateError,
) -> Result<()> {
    create_parents(dest)?;
    fs::hard_link(source, dest).map_err(|error| creation_error(dest, error, refuse))
}

/// Gives the file the one of two modes an update needs, whatever the archive asked for.
#[cfg(unix)]
fn restrict(file: &File, executable: bool, dest: &Path) -> Result<()> {
    use std::{fs::Permissions, os::unix::fs::PermissionsExt};

    let mode = if executable { 0o755 } else { 0o644 };
    file.set_permissions(Permissions::from_mode(mode))
        .map_err(|error| UpdateError::io("set permissions of", dest, error))
}

#[cfg(not(unix))]
fn restrict(_file: &File, _executable: bool, _dest: &Path) -> Result<()> {
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<()> {
    let removed = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => Err(error),
    };
    removed.map_err(|error| UpdateError::io("remove", path, error))
}

fn bad_archive(file: &str, error: &io::Error) -> UpdateError {
    UpdateError::BadArchive {
        file: file.to_owned(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use allot_shared::release::Target;
    use flate2::{Compression, write::GzEncoder};
    use semver::Version;
    use tar::{Builder, Header};
    use tempfile::TempDir;

    use super::*;
    use crate::{limits::Limits, source::Source};

    const TARGET: Target = Target::LinuxX64;
    const VERSION: &str = "0.7.0";

    fn root() -> String {
        TARGET.folder_name(VERSION)
    }

    /// One entry of a test archive, written byte by byte so a path the `tar` crate's own
    /// writer would refuse can be tested.
    struct Spec {
        path: String,
        kind: EntryType,
        data: Vec<u8>,
        link: Option<String>,
        mode: u32,
    }

    impl Spec {
        fn new(path: &str, kind: EntryType) -> Self {
            Self {
                path: path.to_owned(),
                kind,
                data: Vec::new(),
                link: None,
                mode: 0o644,
            }
        }

        fn file(path: &str, data: &[u8], mode: u32) -> Self {
            Self {
                data: data.to_vec(),
                mode,
                ..Self::new(path, EntryType::Regular)
            }
        }

        fn dir(path: &str) -> Self {
            Self {
                mode: 0o755,
                ..Self::new(path, EntryType::Directory)
            }
        }

        fn link(path: &str, kind: EntryType, target: &str) -> Self {
            Self {
                link: Some(target.to_owned()),
                ..Self::new(path, kind)
            }
        }

        fn in_root(self, name: &str) -> Self {
            Self {
                path: format!("{}/{name}", root()),
                ..self
            }
        }
    }

    fn tarball(specs: &[Spec]) -> Vec<u8> {
        let mut builder = Builder::new(Vec::new());
        for spec in specs {
            let mut header = Header::new_gnu();
            header.set_entry_type(spec.kind);
            header.set_size(spec.data.len() as u64);
            header.set_mode(spec.mode);
            header.as_old_mut().name[..spec.path.len()].copy_from_slice(spec.path.as_bytes());
            if let Some(link) = &spec.link {
                header.as_old_mut().linkname[..link.len()].copy_from_slice(link.as_bytes());
            }
            header.set_cksum();
            builder.append(&header, spec.data.as_slice()).unwrap();
        }
        gzip(&builder.into_inner().unwrap())
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn good_specs() -> Vec<Spec> {
        vec![
            Spec::dir(&root()),
            Spec::file(&format!("{}/allot", root()), b"binary", 0o755),
            Spec::file(&format!("{}/README.md", root()), b"read me", 0o664),
        ]
    }

    struct Fixture {
        dir: TempDir,
        verified: Verified,
    }

    impl Fixture {
        fn new(archive: &[u8]) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let asset = Asset::Archive(TARGET);
            let path = dir.path().join(asset.file_name(VERSION));
            fs::write(&path, archive).unwrap();
            let verified = Verified {
                asset,
                version: Version::parse(VERSION).unwrap(),
                path,
                sha256: String::new(),
                bytes: archive.len() as u64,
            };
            Self { dir, verified }
        }

        fn unpack(&self, limits: Limits) -> Result<PathBuf> {
            Updater::new(Source::github(), limits).unpack(&self.verified)
        }

        /// Everything in the folder besides the archive itself.
        fn leftovers(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(self.dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|name| *name != self.verified.asset.file_name(VERSION))
                .collect();
            names.sort();
            names
        }
    }

    fn refusal(result: Result<PathBuf>) -> UnsafeReason {
        match result {
            Err(UpdateError::UnsafeEntry { reason, .. }) => reason,
            other => panic!("expected a refused entry, got {other:?}"),
        }
    }

    #[test]
    fn test_unpack_good_archive_writes_the_release_folder() {
        let fixture = Fixture::new(&tarball(&good_specs()));
        let root_dir = fixture.unpack(Limits::default()).unwrap();
        assert_eq!(root_dir, fixture.dir.path().join(root()));
        assert_eq!(fs::read(root_dir.join("allot")).unwrap(), b"binary");
        assert_eq!(fs::read(root_dir.join("README.md")).unwrap(), b"read me");
    }

    #[cfg(unix)]
    #[test]
    fn test_unpack_good_archive_reduces_permissions_to_two_modes() {
        use std::os::unix::fs::PermissionsExt;

        let mut specs = good_specs();
        specs.push(Spec::file(&format!("{}/allotd", root()), b"daemon", 0o4777));
        specs.push(Spec::file(&format!("{}/notes", root()), b"notes", 0o666));
        let fixture = Fixture::new(&tarball(&specs));
        let root_dir = fixture.unpack(Limits::default()).unwrap();
        let mode = |name: &str| {
            fs::metadata(root_dir.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777
        };
        assert_eq!(mode("allot"), 0o755);
        assert_eq!(mode("allotd"), 0o755);
        assert_eq!(mode("README.md"), 0o644);
        assert_eq!(mode("notes"), 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn test_unpack_downward_symlink_and_inside_hard_link_are_kept() {
        let mut specs = good_specs();
        specs.push(Spec::link(
            &format!("{}/latest", root()),
            EntryType::Symlink,
            "allot",
        ));
        specs.push(Spec::link(
            &format!("{}/copy", root()),
            EntryType::Link,
            &format!("{}/allot", root()),
        ));
        let fixture = Fixture::new(&tarball(&specs));
        let root_dir = fixture.unpack(Limits::default()).unwrap();
        assert_eq!(fs::read(root_dir.join("latest")).unwrap(), b"binary");
        assert_eq!(fs::read(root_dir.join("copy")).unwrap(), b"binary");
    }

    #[test]
    fn test_unpack_path_climbing_out_is_refused_and_nothing_stays() {
        let mut specs = good_specs();
        specs.push(Spec::file(&format!("{}/../evil", root()), b"x", 0o644));
        let fixture = Fixture::new(&tarball(&specs));
        assert_eq!(
            refusal(fixture.unpack(Limits::default())),
            UnsafeReason::ParentDir
        );
        assert_eq!(fixture.leftovers(), Vec::<String>::new());
    }

    #[test]
    fn test_unpack_absolute_path_is_refused() {
        let specs = [Spec::file("/tmp/evil", b"x", 0o644)];
        let fixture = Fixture::new(&tarball(&specs));
        assert_eq!(
            refusal(fixture.unpack(Limits::default())),
            UnsafeReason::Absolute
        );
    }

    #[test]
    fn test_unpack_path_outside_the_release_folder_is_refused() {
        let specs = [Spec::file("other/evil", b"x", 0o644)];
        let fixture = Fixture::new(&tarball(&specs));
        assert_eq!(
            refusal(fixture.unpack(Limits::default())),
            UnsafeReason::OutsideRoot
        );
    }

    #[test]
    fn test_unpack_odd_names_are_refused() {
        // A backslash is a separator on Windows, where it only nests the path; elsewhere it
        // is part of a name that would split on a copy to Windows.
        let names: &[&str] = if cfg!(windows) {
            &["C:evil", "stream:ads"]
        } else {
            &["C:evil", "stream:ads", "a\\b"]
        };
        for name in names {
            let specs = [Spec::file(&format!("{}/{name}", root()), b"x", 0o644)];
            let fixture = Fixture::new(&tarball(&specs));
            assert_eq!(
                refusal(fixture.unpack(Limits::default())),
                UnsafeReason::OddName,
                "{name}"
            );
        }
    }

    #[test]
    fn test_unpack_symlinks_that_leave_or_climb_are_refused() {
        for target in [
            "/etc/passwd",
            "C:\\Windows",
            "../../etc/passwd",
            "..",
            "sub/../..",
            "",
        ] {
            let mut specs = good_specs();
            specs.push(Spec::link("", EntryType::Symlink, target).in_root("evil"));
            let fixture = Fixture::new(&tarball(&specs));
            assert_eq!(
                refusal(fixture.unpack(Limits::default())),
                UnsafeReason::LinkOutside,
                "{target:?}"
            );
            assert_eq!(fixture.leftovers(), Vec::<String>::new(), "{target:?}");
        }
    }

    #[test]
    fn test_unpack_chain_of_links_cannot_climb_through_a_link() {
        let mut specs = good_specs();
        specs.push(Spec::dir(&format!("{}/d", root())));
        specs.push(Spec::link("", EntryType::Symlink, "..").in_root("d/up"));
        let fixture = Fixture::new(&tarball(&specs));
        assert_eq!(
            refusal(fixture.unpack(Limits::default())),
            UnsafeReason::LinkOutside
        );
    }

    #[test]
    fn test_unpack_hard_links_to_outside_are_refused() {
        for target in [
            "/etc/passwd",
            "../escape",
            "other/file",
            &format!("{}/../../escape", root()),
        ] {
            let mut specs = good_specs();
            specs.push(Spec::link("", EntryType::Link, target).in_root("evil"));
            let fixture = Fixture::new(&tarball(&specs));
            assert_eq!(
                refusal(fixture.unpack(Limits::default())),
                UnsafeReason::LinkOutside,
                "{target}"
            );
        }
    }

    #[test]
    fn test_unpack_devices_and_pipes_are_refused() {
        for kind in [EntryType::Char, EntryType::Block, EntryType::Fifo] {
            let mut specs = good_specs();
            specs.push(Spec::new("", kind).in_root("dev"));
            let fixture = Fixture::new(&tarball(&specs));
            assert_eq!(
                refusal(fixture.unpack(Limits::default())),
                UnsafeReason::UnsupportedType
            );
        }
    }

    #[test]
    fn test_unpack_duplicate_path_is_refused() {
        let mut specs = good_specs();
        specs.push(Spec::file(&format!("{}/allot", root()), b"second", 0o755));
        let fixture = Fixture::new(&tarball(&specs));
        assert_eq!(
            refusal(fixture.unpack(Limits::default())),
            UnsafeReason::Duplicate
        );
    }

    #[test]
    fn test_unpack_writing_through_an_earlier_link_is_refused() {
        let mut specs = good_specs();
        specs.push(Spec::link("", EntryType::Symlink, "allot").in_root("alias"));
        specs.push(Spec::file(&format!("{}/alias", root()), b"x", 0o644));
        let fixture = Fixture::new(&tarball(&specs));
        let result = fixture.unpack(Limits::default());
        #[cfg(unix)]
        assert_eq!(refusal(result), UnsafeReason::Duplicate);
        #[cfg(not(unix))]
        assert_eq!(refusal(result), UnsafeReason::LinksUnsupported);
    }

    #[test]
    fn test_unpack_limits_on_entries_and_data_are_enforced() {
        let fixture = Fixture::new(&tarball(&good_specs()));
        let few = Limits {
            max_entries: 2,
            ..Limits::default()
        };
        assert_eq!(refusal(fixture.unpack(few)), UnsafeReason::TooManyEntries);
        let small = Limits {
            max_unpacked_bytes: 8,
            ..Limits::default()
        };
        assert_eq!(refusal(fixture.unpack(small)), UnsafeReason::TooMuchData);
        assert_eq!(fixture.leftovers(), Vec::<String>::new());
    }

    #[test]
    fn test_unpack_not_gzip_or_cut_off_is_a_bad_archive() {
        let whole = tarball(&good_specs());
        for bytes in [
            b"not an archive at all".to_vec(),
            whole[..whole.len() / 2].to_vec(),
        ] {
            let fixture = Fixture::new(&bytes);
            assert!(matches!(
                fixture.unpack(Limits::default()),
                Err(UpdateError::BadArchive { .. })
            ));
            assert_eq!(fixture.leftovers(), Vec::<String>::new());
        }
    }

    #[test]
    fn test_unpack_archive_without_the_release_folder_is_a_bad_archive() {
        let fixture = Fixture::new(&tarball(&[]));
        assert!(matches!(
            fixture.unpack(Limits::default()),
            Err(UpdateError::BadArchive { reason, .. }) if reason.contains(&root())
        ));
    }

    #[test]
    fn test_unpack_replaces_the_folder_of_an_earlier_unpack() {
        let fixture = Fixture::new(&tarball(&good_specs()));
        let stale = fixture.dir.path().join(root());
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("stale"), b"old").unwrap();
        let root_dir = fixture.unpack(Limits::default()).unwrap();
        assert!(!root_dir.join("stale").exists());
        assert!(root_dir.join("allot").is_file());
    }

    #[test]
    fn test_unpack_only_tarballs_are_unpacked() {
        let mut fixture = Fixture::new(b"x");
        for asset in [
            Asset::Setup,
            Asset::MacApp,
            Asset::Archive(Target::WindowsX64),
        ] {
            fixture.verified.asset = asset;
            assert!(matches!(
                fixture.unpack(Limits::default()),
                Err(UpdateError::NotATarball { .. })
            ));
        }
    }

    #[test]
    fn test_inside_root_ignores_current_dir_parts_and_skips_the_folder_itself() {
        let root = "allot-1";
        assert_eq!(
            inside_root(Path::new("./allot-1/./bin"), root),
            Ok(Some(PathBuf::from("allot-1").join("bin")))
        );
        assert_eq!(inside_root(Path::new("./"), root), Ok(None));
        assert_eq!(inside_root(Path::new("."), root), Ok(None));
    }

    #[test]
    fn test_descends_accepts_only_downward_targets() {
        assert!(descends(Path::new("allot")));
        assert!(descends(Path::new("./bin/allot")));
        assert!(!descends(Path::new("")));
        assert!(!descends(Path::new("../x")));
        assert!(!descends(Path::new("/x")));
    }
}
