//! The update check of an in-process allotd against a release server of the test's own. Nothing
//! here reaches github.com, and no test starts an update of a real install: the run a start
//! launches is a stand-in program that leaves a marker.

mod common;

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use allot_shared::{
    config::Config,
    update::{OutcomeState, UpdateCheck, UpdateOutcome},
};
use allotd::UpdateRun;
use common::{ALICE_PID, Daemon, eventually, releases::ReleaseServer, send};
use reqwest::StatusCode;
use serde_json::{Value, json};

/// The `update` object of `GET /ui/state`.
async fn update_of(daemon: &Daemon) -> Value {
    let state: Value = daemon.get("/ui/state").await.json().await.unwrap();
    state["update"].clone()
}

/// Waits until the `update` of the UI state says `state`, and returns it.
async fn wait_for_state(daemon: &Daemon, state: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let update = update_of(daemon).await;
        if update["state"] == state {
            return update;
        }
        assert!(
            Instant::now() < deadline,
            "the update state stayed {} instead of {state}",
            update["state"]
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// The safety net behind every test that spawns the built allotd: `.cargo/config.toml` points
/// `ALLOT_UPDATE_BASE` at a closed port, so no process cargo starts can reach github.com.
#[test]
fn test_update_base_under_cargo_is_never_github() {
    let source = allot_updater::Source::from_env().unwrap();

    assert_ne!(
        source.base(),
        allot_updater::GITHUB_BASE,
        "ALLOT_UPDATE_BASE must point at a local server or a closed port while testing"
    );
}

fn off() -> Config {
    Config::from_toml("[update]\ncheck = false\n").unwrap()
}

/// A program that sleeps a little, leaves `marker`, and prints a line to its log.
fn stand_in(marker: &Path) -> UpdateRun {
    let marker = marker.display();
    #[cfg(unix)]
    let (program, args) = (
        PathBuf::from("/bin/sh"),
        vec![
            OsString::from("-c"),
            OsString::from(format!("sleep 1; echo done > '{marker}'; echo ran")),
        ],
    );
    #[cfg(windows)]
    let (program, args) = (
        PathBuf::from(std::env::var("SystemRoot").unwrap()).join(r"System32\cmd.exe"),
        vec![
            OsString::from("/C"),
            // No quotes around the marker: cmd does not read the `\"` Rust escapes them to, and
            // a temp folder's path has no spaces.
            OsString::from(format!(
                "ping -n 3 127.0.0.1 >NUL & echo done> {marker} & echo ran"
            )),
        ],
    );
    UpdateRun {
        program,
        args,
        log: PathBuf::new(),
    }
}

#[tokio::test]
async fn test_update_a_newer_release_appears_in_the_ui_state() {
    let server = ReleaseServer::start("v99.0.0").await;

    let daemon = Daemon::start_updating(Config::default(), |dir| server.options(dir)).await;

    let update = wait_for_state(&daemon, "available").await;
    assert_eq!(update["installed"], env!("CARGO_PKG_VERSION"));
    assert_eq!(update["latest"], "99.0.0");
    assert_eq!(
        update["release_url"].as_str().unwrap(),
        format!("{}/tag/v99.0.0", server.base)
    );
    assert_eq!(update["reason"], Value::Null);
    assert_eq!(update["announce"], "99.0.0");
    assert!(update["checked_at_ms"].as_i64().unwrap() > 0);
    assert_eq!(server.requests(), ["/releases/latest"]);
    let file = UpdateCheck::read(&daemon.dir.path().join("update-check.json")).unwrap();
    assert_eq!(file.latest.as_deref(), Some("99.0.0"));
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_the_installed_release_is_up_to_date() {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let server = ReleaseServer::start(&tag).await;

    let daemon = Daemon::start_updating(Config::default(), |dir| server.options(dir)).await;

    let update = wait_for_state(&daemon, "up_to_date").await;
    assert_eq!(update["latest"], env!("CARGO_PKG_VERSION"));
    assert_eq!(update["announce"], Value::Null);
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_with_the_setting_off_the_server_sees_no_request() {
    let server = ReleaseServer::start("v99.0.0").await;
    let daemon = Daemon::start_updating(off(), |dir| server.options(dir)).await;

    tokio::time::sleep(Duration::from_millis(400)).await;

    let update = update_of(&daemon).await;
    assert_eq!(update["state"], "disabled");
    assert!(server.requests().is_empty(), "{:?}", server.requests());
    for route in ["/update/check", "/update/start"] {
        let response = send(daemon.post(route)).await;
        assert_eq!(response.status(), StatusCode::CONFLICT, "{route}");
        let text = response.text().await.unwrap();
        assert!(text.contains("update checks are off"), "{text}");
    }
    assert!(server.requests().is_empty(), "{:?}", server.requests());

    // Turning it on in the file takes effect without a restart.
    daemon.write_config("");
    wait_for_state(&daemon, "available").await;
    assert_eq!(server.requests(), ["/releases/latest"]);
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_a_failed_check_reports_why_and_is_tried_again() {
    let server = ReleaseServer::failing(503).await;
    let daemon = Daemon::start_updating(Config::default(), |dir| allotd::UpdateOptions {
        retry_after: Duration::from_millis(50),
        ..server.options(dir)
    })
    .await;

    let update = wait_for_state(&daemon, "check_failed").await;

    let reason = update["reason"].as_str().unwrap();
    assert!(reason.contains("503"), "{reason}");
    assert!(reason.contains(&server.base), "{reason}");
    assert_eq!(update["latest"], Value::Null);
    assert!(update["checked_at_ms"].as_i64().unwrap() > 0);
    eventually("a second request after the failure", || async {
        server.requests().len() >= 2
    })
    .await;

    // The server recovers: the next retry finds the release and the failure is gone.
    server.publish("v99.0.0");
    let update = wait_for_state(&daemon, "available").await;
    assert_eq!(update["reason"], Value::Null);
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_a_bad_update_base_is_a_failed_check_with_its_reason() {
    let daemon = Daemon::start_updating(Config::default(), |dir| allotd::UpdateOptions {
        updater: Err("ALLOT_UPDATE_BASE is not a usable address".to_owned()),
        first_check_after: Duration::ZERO,
        poll: Duration::from_millis(20),
        ..allotd::UpdateOptions::inert(dir)
    })
    .await;

    let update = wait_for_state(&daemon, "check_failed").await;

    assert_eq!(
        update["reason"],
        "ALLOT_UPDATE_BASE is not a usable address"
    );
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_a_restart_does_not_ask_again_and_remembers_the_release() {
    let server = ReleaseServer::start("v99.0.0").await;
    let daemon = Daemon::start_updating(Config::default(), |dir| server.options(dir)).await;
    let first = wait_for_state(&daemon, "available").await;

    let daemon = daemon.restart_updating(|dir| server.options(dir)).await;

    let update = update_of(&daemon).await;
    assert_eq!(update["state"], "available");
    assert_eq!(update["latest"], "99.0.0");
    assert_eq!(update["checked_at_ms"], first["checked_at_ms"]);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(server.requests(), ["/releases/latest"]);
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_check_now_answers_checking_at_once_and_then_finds_the_release() {
    let server = ReleaseServer::start("v99.0.0").await;
    let daemon = Daemon::start_updating(Config::default(), |dir| allotd::UpdateOptions {
        first_check_after: Duration::from_secs(3_600),
        ..server.options(dir)
    })
    .await;
    assert_eq!(update_of(&daemon).await["state"], "unchecked");

    let response = send(daemon.post("/update/check")).await;

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let answered: Value = response.json().await.unwrap();
    assert_eq!(answered["state"], "checking");
    let update = wait_for_state(&daemon, "available").await;
    assert_eq!(update["latest"], "99.0.0");
    assert_eq!(server.requests(), ["/releases/latest"]);
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_announced_marks_the_release_once_across_restarts() {
    let server = ReleaseServer::start("v99.0.0").await;
    let daemon = Daemon::start_updating(Config::default(), |dir| server.options(dir)).await;
    wait_for_state(&daemon, "available").await;

    let response = send(
        daemon
            .post("/update/announced")
            .json(&json!({"version": "99.0.0"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let answered: Value = response.json().await.unwrap();
    assert_eq!(answered["state"], "available");
    assert_eq!(answered["announce"], Value::Null);
    let file = UpdateCheck::read(&daemon.dir.path().join("update-check.json")).unwrap();
    assert_eq!(file.notified.as_deref(), Some("99.0.0"));
    let daemon = daemon.restart_updating(|dir| server.options(dir)).await;
    assert_eq!(update_of(&daemon).await["announce"], Value::Null);

    // A newer release is announced again.
    server.publish("v99.1.0");
    send(daemon.post("/update/check")).await;
    let update = wait_for_state(&daemon, "available").await;
    assert_eq!(update["latest"], "99.1.0");
    assert_eq!(update["announce"], "99.1.0");
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_announced_refuses_what_it_cannot_record() {
    let server = ReleaseServer::start("v99.0.0").await;
    let daemon = Daemon::start_updating(Config::default(), |dir| allotd::UpdateOptions {
        first_check_after: Duration::from_secs(3_600),
        ..server.options(dir)
    })
    .await;

    let before_any_check = send(
        daemon
            .post("/update/announced")
            .json(&json!({"version": "99.0.0"})),
    )
    .await;
    assert_eq!(before_any_check.status(), StatusCode::CONFLICT);

    send(daemon.post("/update/check")).await;
    wait_for_state(&daemon, "available").await;
    let not_a_version = send(
        daemon
            .post("/update/announced")
            .json(&json!({"version": "latest"})),
    )
    .await;
    assert_eq!(not_a_version.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let no_body = send(daemon.post("/update/announced")).await;
    assert!(no_body.status().is_client_error());
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_start_runs_a_detached_update_that_outlives_a_daemon_restart() {
    let server = ReleaseServer::start("v99.0.0").await;
    let marker_dir = tempfile::tempdir().unwrap();
    let marker = marker_dir.path().join("ran.txt");
    let options = |dir: &Path| allotd::UpdateOptions {
        run: UpdateRun {
            log: dir.join("update.log"),
            ..stand_in(&marker)
        },
        ..server.options(dir)
    };
    let daemon = Daemon::start_updating(Config::default(), options).await;
    wait_for_state(&daemon, "available").await;

    let response = send(daemon.post("/update/start")).await;

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let started: Value = response.json().await.unwrap();
    assert_eq!(started["state"], "installing");
    assert_eq!(started["latest"], "99.0.0");
    assert_eq!(update_of(&daemon).await["state"], "installing");
    let again = send(daemon.post("/update/start")).await;
    assert_eq!(again.status(), StatusCode::CONFLICT);

    // The daemon stops while the run is still sleeping, and a new one takes over.
    let daemon = daemon.restart_updating(options).await;

    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() {
        let log = fs::read_to_string(daemon.dir.path().join("update.log"));
        assert!(
            Instant::now() < deadline,
            "the run did not outlive allotd; its log: {log:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let log = fs::read_to_string(daemon.dir.path().join("update.log")).unwrap();
    assert!(log.contains("ran"), "{log}");
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_start_refuses_without_a_known_newer_release() {
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let server = ReleaseServer::start(&tag).await;
    let daemon = Daemon::start_updating(Config::default(), |dir| server.options(dir)).await;
    wait_for_state(&daemon, "up_to_date").await;

    let response = send(daemon.post("/update/start")).await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let text = response.text().await.unwrap();
    assert!(text.contains("no newer release is known"), "{text}");
    daemon.stop().await;
}

#[tokio::test]
async fn test_update_start_without_an_allot_to_run_says_to_install_it() {
    let server = ReleaseServer::start("v99.0.0").await;
    let daemon = Daemon::start_updating(Config::default(), |dir| server.options(dir)).await;
    wait_for_state(&daemon, "available").await;

    let response = send(daemon.post("/update/start")).await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let text = response.text().await.unwrap();
    assert!(text.contains("allot install"), "{text}");
    assert_eq!(update_of(&daemon).await["state"], "available");
    daemon.stop().await;
}

fn write_outcome(daemon: &Daemon, state: OutcomeState, pid: u32, reason: Option<&str>) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    UpdateOutcome {
        state,
        version: "99.0.0".into(),
        started_at_ms: now - 5_000,
        finished_at_ms: None,
        pid,
        reason: reason.map(Into::into),
        notes: Vec::new(),
    }
    .write(&daemon.dir.path().join("update-install.json"))
    .unwrap();
}

#[tokio::test]
async fn test_update_the_record_of_a_run_shows_installing_then_why_it_failed() {
    let server = ReleaseServer::start("v99.0.0").await;
    let daemon = Daemon::start_updating(Config::default(), |dir| server.options(dir)).await;
    wait_for_state(&daemon, "available").await;

    // A running process (the fake machine's first session) is a live run.
    write_outcome(&daemon, OutcomeState::Installing, ALICE_PID, None);
    let update = wait_for_state(&daemon, "installing").await;
    assert_eq!(update["latest"], "99.0.0");

    write_outcome(
        &daemon,
        OutcomeState::Failed,
        ALICE_PID,
        Some("the download does not match its checksum"),
    );
    let update = wait_for_state(&daemon, "install_failed").await;
    assert_eq!(update["reason"], "the download does not match its checksum");
    assert_eq!(update["latest"], "99.0.0");

    // A run that says it installs while its process is gone died.
    write_outcome(&daemon, OutcomeState::Installing, 999_999, None);
    let update = wait_for_state(&daemon, "install_failed").await;
    assert_eq!(update["reason"], "the update did not finish");
    daemon.stop().await;
}
