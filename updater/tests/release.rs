//! The updater against a release served from a loopback port: the lookup, the download, the
//! checksum and the unpack, end to end.

mod support;

use std::{fs, io::Write, path::Path, time::Duration};

use allot_shared::{
    paths::Paths,
    release::{Asset, Target},
};
use allot_updater::{
    Check, Limits, Release, Source, UnsafeReason, UpdateError, Updater, Version, folder,
};
use flate2::{Compression, write::GzEncoder};
use support::{Reply, Server, asset_path, asset_routes, closed_base, latest_route, sha256_hex};
use tempfile::TempDir;

const ASSET: Asset = Asset::Archive(Target::LinuxX64);
const VERSION: &str = "0.7.0";

fn updater(base: &str, limits: Limits) -> Updater {
    Updater::new(Source::new(base).unwrap(), limits)
}

fn installed(text: &str) -> Version {
    Version::parse(text).unwrap()
}

fn release(updater: &Updater) -> Release {
    updater.release(installed(VERSION))
}

fn throwaway_home() -> (TempDir, Paths) {
    let home = tempfile::tempdir().unwrap();
    let paths = Paths::under(home.path());
    (home, paths)
}

/// The names in the release's update folder.
fn folder_listing(paths: &Paths) -> Vec<String> {
    let dir = folder::of(paths, &installed(VERSION));
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// The lookup.

fn check(tag: &str, installed_version: &str) -> (Check, Vec<String>) {
    let server = Server::start(vec![latest_route(tag)]);
    let updater = updater(&server.base(), Limits::default());
    let outcome = updater.check(&installed(installed_version)).unwrap();
    (outcome, server.requests())
}

#[test]
fn test_check_newer_tag_is_offered_without_following_the_redirect() {
    let (outcome, requests) = check("v0.7.0", "0.6.1");
    let Check::Newer(release) = outcome else {
        panic!("expected a newer release, got {outcome:?}");
    };
    assert_eq!(release.version, installed("0.7.0"));
    assert_eq!(release.tag, "v0.7.0");
    assert!(release.page_url.ends_with("/releases/tag/v0.7.0"));
    assert_eq!(requests, ["/releases/latest"]);
}

#[test]
fn test_check_same_or_older_tag_is_not_newer() {
    for tag in ["v0.6.1", "v0.5.0"] {
        let (outcome, _) = check(tag, "0.6.1");
        let Check::NotNewer {
            latest: Some(latest),
        } = outcome
        else {
            panic!("expected the latest to be known, got {outcome:?}");
        };
        assert_eq!(latest.tag, tag);
    }
}

#[test]
fn test_check_prerelease_or_odd_tag_is_ignored() {
    for tag in ["v0.7.0-beta.1", "v0.7.0-rc.2", "nightly", "v0.7"] {
        let (outcome, _) = check(tag, "0.6.1");
        assert_eq!(outcome, Check::NotNewer { latest: None }, "{tag}");
    }
}

#[test]
fn test_check_release_after_an_installed_prerelease_is_newer() {
    let (outcome, _) = check("v0.7.0", "0.7.0-beta.1");
    assert!(matches!(outcome, Check::Newer(_)));
}

#[test]
fn test_check_answer_that_is_no_redirect_to_a_tag_is_an_error() {
    let cases = [
        Reply::Body(b"<html>a page</html>".to_vec()),
        Reply::Redirect("/releases/latest".to_owned()),
        Reply::Redirect("/releases".to_owned()),
    ];
    for reply in cases {
        let server = Server::start(vec![("/releases/latest".to_owned(), reply)]);
        let result = updater(&server.base(), Limits::default()).check(&installed("0.6.1"));
        assert!(
            matches!(result, Err(UpdateError::BadLatest { .. })),
            "{result:?}"
        );
    }
}

#[test]
fn test_check_error_status_is_reported_with_the_status() {
    let server = Server::start(vec![("/releases/latest".to_owned(), Reply::Status(503))]);
    let result = updater(&server.base(), Limits::default()).check(&installed("0.6.1"));
    assert!(matches!(
        result,
        Err(UpdateError::Status { status: 503, .. })
    ));
}

#[test]
fn test_check_nothing_listening_is_a_network_error() {
    let result = updater(&closed_base(), Limits::default()).check(&installed("0.6.1"));
    assert!(
        matches!(result, Err(UpdateError::Network { .. })),
        "{result:?}"
    );
}

// The download and its checksum.

fn serve(routes: Vec<(String, Reply)>) -> (Server, Updater) {
    let server = Server::start(routes);
    let updater = updater(&server.base(), Limits::default());
    (server, updater)
}

#[test]
fn test_fetch_good_asset_is_verified_and_kept_with_its_checksum() {
    let body = b"the archive".to_vec();
    let (server, updater) = serve(asset_routes(
        VERSION,
        ASSET,
        &body,
        &ASSET.file_name(VERSION),
    ));
    let (_home, paths) = throwaway_home();

    let verified = updater.fetch(&paths, &release(&updater), ASSET).unwrap();

    let folder = folder::of(&paths, &installed(VERSION));
    assert_eq!(verified.path(), folder.join(ASSET.file_name(VERSION)));
    assert_eq!(fs::read(verified.path()).unwrap(), body);
    assert_eq!(verified.sha256(), sha256_hex(&body));
    assert_eq!(verified.bytes(), body.len() as u64);
    assert_eq!(verified.asset(), ASSET);
    assert_eq!(verified.version(), &installed(VERSION));
    assert_eq!(
        folder_listing(&paths),
        [ASSET.file_name(VERSION), ASSET.checksum_name(VERSION)]
    );
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn test_fetch_twice_replaces_the_first_download() {
    let (_server, updater) = serve(asset_routes(
        VERSION,
        ASSET,
        b"same",
        &ASSET.file_name(VERSION),
    ));
    let (_home, paths) = throwaway_home();
    updater.fetch(&paths, &release(&updater), ASSET).unwrap();
    updater.fetch(&paths, &release(&updater), ASSET).unwrap();
    assert_eq!(folder_listing(&paths).len(), 2);
}

#[test]
fn test_fetch_wrong_hash_leaves_no_file() {
    let file = ASSET.file_name(VERSION);
    let mut routes = asset_routes(VERSION, ASSET, b"what the checksum covers", &file);
    routes[0].1 = Reply::Body(b"something else".to_vec());
    let (_server, updater) = serve(routes);
    let (_home, paths) = throwaway_home();

    let result = updater.fetch(&paths, &release(&updater), ASSET);

    let Err(UpdateError::ChecksumMismatch {
        expected, actual, ..
    }) = result
    else {
        panic!("expected a mismatch, got {result:?}");
    };
    assert_eq!(expected, sha256_hex(b"what the checksum covers"));
    assert_eq!(actual, sha256_hex(b"something else"));
    assert_eq!(folder_listing(&paths), Vec::<String>::new());
}

#[test]
fn test_fetch_checksum_naming_another_file_stops_before_the_download() {
    let routes = asset_routes(VERSION, ASSET, b"body", "allot-0.7.0-setup.exe");
    let (server, updater) = serve(routes);
    let (_home, paths) = throwaway_home();

    let result = updater.fetch(&paths, &release(&updater), ASSET);

    assert!(matches!(
        result,
        Err(UpdateError::ChecksumNamesOther { ref listed, .. }) if listed == "allot-0.7.0-setup.exe"
    ));
    assert_eq!(
        server.requests(),
        [asset_path(VERSION, &ASSET.checksum_name(VERSION))]
    );
    assert_eq!(folder_listing(&paths), Vec::<String>::new());
}

#[test]
fn test_fetch_checksum_that_is_not_a_checksum_is_refused() {
    let file = ASSET.file_name(VERSION);
    let mut routes = asset_routes(VERSION, ASSET, b"body", &file);
    routes[1].1 = Reply::Body(b"<html>Not Found</html>".to_vec());
    let (_server, updater) = serve(routes);
    let (_home, paths) = throwaway_home();
    let result = updater.fetch(&paths, &release(&updater), ASSET);
    assert!(matches!(result, Err(UpdateError::BadChecksumFile { .. })));
}

#[test]
fn test_fetch_missing_asset_or_checksum_is_a_missing_file() {
    let file = ASSET.file_name(VERSION);
    for missing in [0, 1] {
        let mut routes = asset_routes(VERSION, ASSET, b"body", &file);
        routes.remove(missing);
        let (_server, updater) = serve(routes);
        let (_home, paths) = throwaway_home();

        let result = updater.fetch(&paths, &release(&updater), ASSET);

        assert!(
            matches!(result, Err(UpdateError::AssetMissing { ref tag, .. }) if tag == "v0.7.0"),
            "{result:?}"
        );
        assert_eq!(folder_listing(&paths), Vec::<String>::new());
    }
}

#[test]
fn test_fetch_server_error_is_a_status_error() {
    let file = ASSET.file_name(VERSION);
    let mut routes = asset_routes(VERSION, ASSET, b"body", &file);
    routes[0].1 = Reply::Status(500);
    let (_server, updater) = serve(routes);
    let (_home, paths) = throwaway_home();
    let result = updater.fetch(&paths, &release(&updater), ASSET);
    assert!(matches!(
        result,
        Err(UpdateError::Status { status: 500, .. })
    ));
}

#[test]
fn test_fetch_truncated_download_fails_and_leaves_nothing() {
    let body = b"0123456789".to_vec();
    let file = ASSET.file_name(VERSION);
    let mut routes = asset_routes(VERSION, ASSET, &body, &file);
    routes[0].1 = Reply::Cut {
        claimed: body.len(),
        sent: body[..4].to_vec(),
    };
    let (_server, updater) = serve(routes);
    let (_home, paths) = throwaway_home();

    let result = updater.fetch(&paths, &release(&updater), ASSET);

    assert!(
        matches!(result, Err(UpdateError::Truncated { received: 4, .. })),
        "{result:?}"
    );
    assert_eq!(folder_listing(&paths), Vec::<String>::new());
}

#[test]
fn test_fetch_redirect_loop_fails_after_the_limit() {
    let file = ASSET.file_name(VERSION);
    let path = asset_path(VERSION, &file);
    let mut routes = asset_routes(VERSION, ASSET, b"body", &file);
    routes[0].1 = Reply::Redirect(path.clone());
    let (server, updater) = serve(routes);
    let (_home, paths) = throwaway_home();

    let result = updater.fetch(&paths, &release(&updater), ASSET);

    assert!(
        matches!(result, Err(UpdateError::TooManyRedirects { limit: 5, .. })),
        "{result:?}"
    );
    let hits = server.requests().iter().filter(|got| **got == path).count();
    assert_eq!(hits, 6);
    assert_eq!(folder_listing(&paths), Vec::<String>::new());
}

#[test]
fn test_fetch_follows_a_redirect_to_the_file_host() {
    let body = b"served from elsewhere".to_vec();
    let file = ASSET.file_name(VERSION);
    let mut routes = asset_routes(VERSION, ASSET, &body, &file);
    routes[0].1 = Reply::Redirect("/cdn/signed-file".to_owned());
    routes.push(("/cdn/signed-file".to_owned(), Reply::Body(body.clone())));
    let (_server, updater) = serve(routes);
    let (_home, paths) = throwaway_home();

    let verified = updater.fetch(&paths, &release(&updater), ASSET).unwrap();

    assert_eq!(fs::read(verified.path()).unwrap(), body);
}

#[test]
fn test_fetch_asset_over_the_size_limit_is_refused_by_length_or_by_count() {
    let file = ASSET.file_name(VERSION);
    let limits = Limits {
        max_asset_bytes: 16,
        ..Limits::default()
    };
    for reply in [
        Reply::Body(vec![b'x'; 100]),
        Reply::Unbounded { bytes: 100 },
    ] {
        let mut routes = asset_routes(VERSION, ASSET, &[b'x'; 100], &file);
        routes[0].1 = reply;
        let server = Server::start(routes);
        let updater = updater(&server.base(), limits.clone());
        let (_home, paths) = throwaway_home();

        let result = updater.fetch(&paths, &release(&updater), ASSET);

        assert!(
            matches!(result, Err(UpdateError::TooLarge { limit: 16, .. })),
            "{result:?}"
        );
        assert_eq!(folder_listing(&paths), Vec::<String>::new());
    }
}

#[test]
fn test_fetch_checksum_file_over_its_limit_is_refused() {
    let file = ASSET.file_name(VERSION);
    let routes = asset_routes(VERSION, ASSET, b"body", &file);
    let server = Server::start(routes);
    let limits = Limits {
        max_checksum_bytes: 16,
        ..Limits::default()
    };
    let updater = updater(&server.base(), limits);
    let (_home, paths) = throwaway_home();
    let result = updater.fetch(&paths, &release(&updater), ASSET);
    assert!(matches!(
        result,
        Err(UpdateError::TooLarge { limit: 16, .. })
    ));
}

#[test]
fn test_fetch_stalled_download_times_out_and_leaves_nothing() {
    let body = vec![b'x'; 1000];
    let file = ASSET.file_name(VERSION);
    let mut routes = asset_routes(VERSION, ASSET, &body, &file);
    routes[0].1 = Reply::Stall {
        claimed: body.len(),
        sent: body[..10].to_vec(),
        pause: Duration::from_secs(3),
    };
    let server = Server::start(routes);
    let limits = Limits {
        stall_timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let updater = updater(&server.base(), limits);
    let (_home, paths) = throwaway_home();

    let result = updater.fetch(&paths, &release(&updater), ASSET);

    assert!(
        matches!(result, Err(UpdateError::Timeout { .. })),
        "{result:?}"
    );
    assert_eq!(folder_listing(&paths), Vec::<String>::new());
}

#[test]
fn test_fetch_whole_download_taking_too_long_times_out() {
    let body = vec![b'x'; 1000];
    let file = ASSET.file_name(VERSION);
    let mut routes = asset_routes(VERSION, ASSET, &body, &file);
    routes[0].1 = Reply::Stall {
        claimed: body.len(),
        sent: body[..10].to_vec(),
        pause: Duration::from_secs(3),
    };
    let server = Server::start(routes);
    let limits = Limits {
        download_timeout: Duration::from_millis(400),
        ..Limits::default()
    };
    let updater = updater(&server.base(), limits);
    let (_home, paths) = throwaway_home();
    let result = updater.fetch(&paths, &release(&updater), ASSET);
    assert!(
        matches!(result, Err(UpdateError::Timeout { .. })),
        "{result:?}"
    );
}

// A real archive from download to unpacked folder.

fn release_tarball() -> Vec<u8> {
    let root = Target::LinuxX64.folder_name(VERSION);
    let mut builder = tar::Builder::new(Vec::new());
    let mut dir = tar::Header::new_gnu();
    dir.set_entry_type(tar::EntryType::Directory);
    dir.set_mode(0o755);
    dir.set_size(0);
    builder
        .append_data(&mut dir, &root, std::io::empty())
        .unwrap();
    for (name, data) in [
        ("allot", &b"cli"[..]),
        ("allotd", b"daemon"),
        ("allot-mcp", b"mcp"),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o755);
        header.set_size(data.len() as u64);
        builder
            .append_data(&mut header, format!("{root}/{name}"), data)
            .unwrap();
    }
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&builder.into_inner().unwrap()).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn test_fetch_then_unpack_gives_the_release_folder() {
    let archive = release_tarball();
    let (_server, updater) = serve(asset_routes(
        VERSION,
        ASSET,
        &archive,
        &ASSET.file_name(VERSION),
    ));
    let (_home, paths) = throwaway_home();

    let verified = updater.fetch(&paths, &release(&updater), ASSET).unwrap();
    let root = updater.unpack(&verified).unwrap();

    assert_eq!(
        root,
        folder::of(&paths, &installed(VERSION)).join(Target::LinuxX64.folder_name(VERSION))
    );
    for (name, data) in [("allot", "cli"), ("allotd", "daemon"), ("allot-mcp", "mcp")] {
        assert_eq!(fs::read_to_string(root.join(name)).unwrap(), data);
    }
}

#[test]
fn test_unpack_archive_escaping_its_folder_is_refused_after_a_verified_download() {
    let root = Target::LinuxX64.folder_name(VERSION);
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_mode(0o644);
    header.set_size(4);
    // The writer refuses a `..` path, so the name goes into the header's bytes.
    let name = format!("{root}/../escaped");
    header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
    header.set_cksum();
    builder.append(&header, &b"evil"[..]).unwrap();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&builder.into_inner().unwrap()).unwrap();
    let archive = encoder.finish().unwrap();
    let (_server, updater) = serve(asset_routes(
        VERSION,
        ASSET,
        &archive,
        &ASSET.file_name(VERSION),
    ));
    let (_home, paths) = throwaway_home();

    let verified = updater.fetch(&paths, &release(&updater), ASSET).unwrap();
    let result = updater.unpack(&verified);

    assert!(matches!(
        result,
        Err(UpdateError::UnsafeEntry {
            reason: UnsafeReason::ParentDir,
            ..
        })
    ));
    let update_folder = folder::of(&paths, &installed(VERSION));
    assert!(!update_folder.join("escaped").exists());
    assert!(!Path::new(&update_folder).join(&root).exists());
}
