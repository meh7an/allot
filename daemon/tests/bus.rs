//! The GNOME extension's D-Bus interface on an in-process allotd. Each test starts its own
//! `dbus-daemon` on a socket in a temp folder and stops it afterwards, so the user's session
//! bus, and the name a running allotd owns there, are never touched.

#![cfg(target_os = "linux")]

mod common;

use std::{
    fs,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};

use allot_shared::{
    api::{
        AcquireRequest, AcquireResponse, ProfileSpec, RemoveResponse, SettingsPatch,
        SettingsRefusal, SettingsState,
    },
    config_write::{Change, Revision},
    stop::{StopRecord, StoppedBy},
    types::{EndCause, Holder, JobClass, LeaseKind, LeaseState},
    ui::UiState,
};
use allotd::bus::{INTERFACE, NAME, PATH};
use common::{Daemon, alice, bob, send};
use futures_util::StreamExt;
use reqwest::StatusCode;
use tempfile::TempDir;
use zbus::{
    Connection, MatchRule, MessageStream, connection::Builder, message::Type, wire::Variant,
};

const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
const ERROR_PREFIX: &str = "tech.mehran.Allot1.Error.";

/// A `dbus-daemon` of the test's own, ended when dropped.
struct PrivateBus {
    daemon: Child,
    address: String,
    _dir: TempDir,
}

impl PrivateBus {
    fn start() -> Self {
        let dir = TempDir::new().unwrap();
        let listen = format!("--address=unix:path={}", dir.path().join("bus").display());
        let mut daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile", "--print-address"])
            .arg(listen)
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon runs the test's private bus (package dbus-daemon)");
        // It prints its address once it listens.
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            daemon,
            address: address.trim().to_owned(),
            _dir: dir,
        }
    }

    async fn connect(&self) -> Connection {
        Builder::address(self.address.as_str())
            .build()
            .await
            .unwrap()
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

/// Calls a method of allotd's interface that takes nothing and answers JSON.
async fn ask(bus: &Connection, method: &str) -> zbus::Result<String> {
    let reply = bus
        .call_method(Some(NAME), PATH, Some(INTERFACE), method, &())
        .await?;
    reply.body().deserialize()
}

/// Calls a method that takes a lease id and answers JSON.
async fn ask_lease(bus: &Connection, method: &str, lease: i64) -> zbus::Result<String> {
    let reply = bus
        .call_method(Some(NAME), PATH, Some(INTERFACE), method, &(lease,))
        .await?;
    reply.body().deserialize()
}

/// Calls a method that takes JSON and answers JSON.
async fn ask_json(bus: &Connection, method: &str, json: &str) -> zbus::Result<String> {
    let reply = bus
        .call_method(Some(NAME), PATH, Some(INTERFACE), method, &(json,))
        .await?;
    reply.body().deserialize()
}

/// The error name, without the interface's prefix, and its message.
fn refusal(result: zbus::Result<String>) -> (String, String) {
    match result {
        Err(zbus::Error::MethodError(name, message, _)) => {
            let name = name.as_str().trim_start_matches(ERROR_PREFIX);
            (name.to_owned(), message.unwrap_or_default())
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

async fn name_owner(bus: &Connection) -> Option<String> {
    let reply = bus
        .call_method(Some(DBUS), DBUS_PATH, Some(DBUS), "GetNameOwner", &(NAME,))
        .await
        .ok()?;
    reply.body().deserialize().ok()
}

/// A `cpu_heavy` build that cannot start on fewer than 8 cores, so a second one queues.
async fn acquire_build(daemon: &Daemon, holder: Holder) -> AcquireResponse {
    let request = AcquireRequest {
        holder,
        profile: ProfileSpec::Class(JobClass::CpuHeavy),
        kind: LeaseKind::Normal,
        min_cores: Some(8.0),
        label: "cargo build".into(),
        cwd: None,
        wait_timeout_secs: 0,
    };
    let response = send(daemon.post("/leases").json(&request)).await;
    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

#[tokio::test]
async fn test_bus_get_state_answers_the_popup_state_and_the_version() {
    let bus = PrivateBus::start();
    let daemon = Daemon::start_on_bus(&bus.address).await;
    let client = bus.connect().await;

    let state: UiState = serde_json::from_str(&ask(&client, "GetState").await.unwrap()).unwrap();
    assert_eq!(state.service.pid, std::process::id());

    let reply = client
        .call_method(
            Some(NAME),
            PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(INTERFACE, "Version"),
        )
        .await
        .unwrap();
    let body = reply.body();
    let version: Variant<'_> = body.deserialize().unwrap();
    let version = String::try_from(&version).unwrap();
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
    daemon.stop().await;
}

#[tokio::test]
async fn test_bus_kill_of_an_unknown_lease_is_not_found() {
    let bus = PrivateBus::start();
    let daemon = Daemon::start_on_bus(&bus.address).await;
    let client = bus.connect().await;

    let (name, message) = refusal(ask_lease(&client, "Kill", 999).await);
    assert_eq!(name, "NotFound");
    assert!(message.contains("999"), "{message}");
    daemon.stop().await;
}

#[tokio::test]
async fn test_bus_jobs_changed_follows_a_queued_lease_and_remove_takes_it_out() {
    let bus = PrivateBus::start();
    let daemon = Daemon::start_on_bus(&bus.address).await;
    let client = bus.connect().await;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .interface(INTERFACE)
        .member("JobsChanged")
        .build()
        .unwrap();
    let mut signals = MessageStream::for_match_rule(rule, &client, None)
        .await
        .unwrap();

    acquire_build(&daemon, alice()).await;
    let queued = acquire_build(&daemon, bob()).await.lease;
    assert_eq!(queued.state, LeaseState::Queued);
    tokio::time::timeout(Duration::from_secs(3), signals.next())
        .await
        .expect("JobsChanged after the leases changed")
        .unwrap()
        .unwrap();

    let removed: RemoveResponse =
        serde_json::from_str(&ask_lease(&client, "Remove", queued.id.0).await.unwrap()).unwrap();
    assert_eq!(removed.lease.end_cause, Some(EndCause::Removed));
    let (name, _) = refusal(ask_lease(&client, "Remove", queued.id.0).await);
    assert_eq!(name, "Conflict");
    daemon.stop().await;
}

#[tokio::test]
async fn test_bus_change_settings_after_a_hand_edit_is_a_conflict_with_the_new_revision() {
    let bus = PrivateBus::start();
    let daemon = Daemon::start_on_bus(&bus.address).await;
    let client = bus.connect().await;
    let config = |headroom: u32| {
        let port = daemon.port();
        let text = format!("[daemon]\nport = {port}\n\n[admission]\nheadroom_pct = {headroom}\n");
        fs::write(daemon.config_file(), &text).unwrap();
        text.into_bytes()
    };
    config(25);
    let read: SettingsState =
        serde_json::from_str(&ask(&client, "GetSettings").await.unwrap()).unwrap();
    let edited = config(20);

    let patch = SettingsPatch {
        revision: read.revision,
        changes: vec![Change::SetNumber {
            key: "admission.headroom_pct".into(),
            value: 30.0,
        }],
    };
    let patch = serde_json::to_string(&patch).unwrap();
    let (name, message) = refusal(ask_json(&client, "ChangeSettings", &patch).await);

    assert_eq!(name, "Conflict");
    let refusal: SettingsRefusal = serde_json::from_str(&message).unwrap();
    assert_eq!(refusal.revision, Some(Revision::of(&edited)));
    daemon.stop().await;
}

#[tokio::test]
async fn test_bus_settings_argument_that_is_not_json_is_invalid() {
    let bus = PrivateBus::start();
    let daemon = Daemon::start_on_bus(&bus.address).await;
    let client = bus.connect().await;

    let (name, message) = refusal(ask_json(&client, "ChangeSettings", "not json").await);
    assert_eq!(name, "Invalid");
    // The same SettingsRefusal JSON as every other settings refusal, so a panel can read it.
    let refusal: SettingsRefusal = serde_json::from_str(&message).unwrap();
    assert!(refusal.problems.is_empty());
    assert!(!refusal.message.is_empty());
    daemon.stop().await;
}

#[tokio::test]
async fn test_bus_shutdown_from_the_panel_writes_the_stop_record_and_gives_up_the_name() {
    let bus = PrivateBus::start();
    let mut daemon = Daemon::start_on_bus(&bus.address).await;
    let client = bus.connect().await;

    client
        .call_method(Some(NAME), PATH, Some(INTERFACE), "Shutdown", &(true,))
        .await
        .unwrap();

    let server = daemon.server.take().unwrap();
    tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .expect("allotd stops after the shutdown")
        .unwrap()
        .unwrap();
    let record = StopRecord::read(&daemon.dir.path().join("stopped.json"));
    assert_eq!(record.map(|record| record.by), Some(StoppedBy::Panel));
    assert_eq!(name_owner(&client).await, None);
}

#[tokio::test]
async fn test_bus_name_taken_by_another_program_leaves_allotd_on_http() {
    let bus = PrivateBus::start();
    let other = bus.connect().await;
    other.request_name(NAME).await.unwrap();

    let daemon = Daemon::start_on_bus(&bus.address).await;

    assert_eq!(daemon.health().await.pid, std::process::id());
    let owner = name_owner(&other).await;
    let other_name = other.unique_name().map(|name| name.as_str());
    assert_eq!(owner.as_deref(), other_name);
    daemon.stop().await;
}
