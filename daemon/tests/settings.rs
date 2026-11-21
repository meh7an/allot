//! End-to-end tests of the settings routes on an in-process allotd with its own config file.

mod common;

use std::fs;

use allot_shared::{
    api::{
        CanFitRequest, ClassifyRequest, PreviewRequest, ProfileSpec, SettingsPatch,
        SettingsRefusal, SettingsState, Verdict,
    },
    config::{Config, HeavyRule},
    config_write::{Change, Revision},
    explain::{CommandKind, CommandTest, FitPreview, Shell},
    settings::SettingValue,
    types::{JobClass, JobProfile, LeaseKind},
};
use common::Daemon;
use reqwest::{Response, StatusCode};

impl Daemon {
    /// Writes `text` as the config file, with `[daemon] port` set to the port the daemon
    /// listens on, so nothing waits for a restart.
    fn write_settings(&self, text: &str) -> Vec<u8> {
        let text = format!("[daemon]\nport = {}\n\n{text}", self.port());
        fs::write(self.config_file(), &text).unwrap();
        text.into_bytes()
    }

    fn config_text(&self) -> String {
        fs::read_to_string(self.config_file()).unwrap()
    }

    async fn settings(&self) -> SettingsState {
        ok(self.get("/settings").await).json().await.unwrap()
    }

    async fn patch_settings(&self, revision: Revision, changes: Vec<Change>) -> Response {
        let patch = SettingsPatch { revision, changes };
        self.patch("/settings").json(&patch).send().await.unwrap()
    }

    async fn preview(&self, changes: Vec<Change>) -> Response {
        let request = PreviewRequest { changes };
        self.post("/settings/preview")
            .json(&request)
            .send()
            .await
            .unwrap()
    }

    async fn classify(&self, line: &str, shell: Shell, changes: Vec<Change>) -> Response {
        let request = ClassifyRequest {
            line: line.into(),
            shell,
            changes,
        };
        self.post("/classify").json(&request).send().await.unwrap()
    }

    async fn cpu_heavy_fits(&self) -> bool {
        let request = CanFitRequest {
            profile: ProfileSpec::Class(JobClass::CpuHeavy),
            kind: LeaseKind::Normal,
            min_cores: None,
            label: None,
            cwd: None,
        };
        let response = self.post("/can-fit").json(&request).send().await.unwrap();
        ok(response).json::<Verdict>().await.unwrap().fits
    }
}

fn ok(response: Response) -> Response {
    assert_eq!(response.status(), StatusCode::OK);
    response
}

async fn expect_refusal(response: Response, status: StatusCode) -> SettingsRefusal {
    assert_eq!(response.status(), status);
    response.json().await.unwrap()
}

fn number(state: &SettingsState, key: &str) -> f64 {
    let value = state.values.iter().find(|value| value.key() == key);
    match value {
        Some(SettingValue::Number { value, .. }) => *value,
        other => panic!("{key} is not a number: {other:?}"),
    }
}

fn set_number(key: &str, value: f64) -> Change {
    Change::SetNumber {
        key: key.into(),
        value,
    }
}

fn default_headroom() -> f64 {
    f64::from(Config::default().admission.headroom_pct)
}

#[tokio::test]
async fn test_get_settings_reads_the_spec_and_the_file_values_with_their_revision() {
    let daemon = Daemon::start().await;
    let written = daemon.write_settings("[admission]\nheadroom_pct = 25\n");

    let state = daemon.settings().await;

    assert!(!state.spec.pages.is_empty());
    assert_eq!(state.values.len(), state.spec.settings().count());
    assert_eq!(state.revision, Revision::of(&written));
    assert_eq!(number(&state, "admission.headroom_pct"), 25.0);
    assert_eq!(state.file_error, None);
    assert_eq!(state.listening_port, u32::from(daemon.port()));
    assert!(state.pending_restart.is_empty());
    let anonymous = daemon.client.get(daemon.url("/settings")).send().await;
    assert_eq!(anonymous.unwrap().status(), StatusCode::UNAUTHORIZED);
    daemon.stop().await;
}

#[tokio::test]
async fn test_get_settings_without_a_file_reads_the_defaults_and_the_port_waits() {
    let daemon = Daemon::start().await;

    let state = daemon.settings().await;

    assert_eq!(state.revision, Revision::of(b""));
    assert_eq!(number(&state, "admission.headroom_pct"), default_headroom());
    // The default port is not the one the test daemon was given.
    assert_eq!(state.pending_restart, ["daemon.port"]);
    daemon.stop().await;
}

#[tokio::test]
async fn test_patch_settings_writes_keeps_comments_and_takes_effect_at_once() {
    let daemon = Daemon::start_unpolled().await;
    let written =
        daemon.write_settings("# Kept for the build box.\n[admission]\nheadroom_pct = 25\n");
    assert!(daemon.cpu_heavy_fits().await);

    let changes = vec![
        set_number("admission.headroom_pct", 30.0),
        Change::SetRow {
            table: "classes".into(),
            name: "cpu_heavy".into(),
            profile: JobProfile {
                cores: 8.0,
                ram_mb: 40_000,
                gpu_pct: 0.0,
                vram_mb: 0,
                est_secs: 300,
            },
            min_cores: None,
        },
    ];
    let state: SettingsState = ok(daemon.patch_settings(Revision::of(&written), changes).await)
        .json()
        .await
        .unwrap();

    let text = daemon.config_text();
    // A fractional setting is written as a float.
    let kept = "# Kept for the build box.\n[admission]\nheadroom_pct = 30.0\n";
    assert!(text.contains(kept), "{text}");
    assert!(text.contains("ram_mb = 40000"), "{text}");
    assert_eq!(state.revision, Revision::of(text.as_bytes()));
    assert_eq!(number(&state, "admission.headroom_pct"), 30.0);
    // The config poll runs once an hour here, so only the write itself can have changed the
    // class: 40 GB no longer fits the 32 GB machine.
    assert!(!daemon.cpu_heavy_fits().await);
    daemon.stop().await;
}

#[tokio::test]
async fn test_patch_settings_port_reports_a_pending_restart() {
    let daemon = Daemon::start().await;
    let written = daemon.write_settings("");
    let other_port = f64::from(daemon.port().wrapping_add(1).max(1_024));

    let changes = vec![set_number("daemon.port", other_port)];
    let state: SettingsState = ok(daemon.patch_settings(Revision::of(&written), changes).await)
        .json()
        .await
        .unwrap();

    assert_eq!(state.pending_restart, ["daemon.port"]);
    assert_eq!(state.listening_port, u32::from(daemon.port()));
    assert_eq!(daemon.settings().await.pending_restart, ["daemon.port"]);
    daemon.stop().await;
}

#[tokio::test]
async fn test_patch_settings_invalid_value_names_the_key_and_leaves_the_file() {
    let daemon = Daemon::start().await;
    let written = daemon.write_settings("[admission]\nheadroom_pct = 25\n");

    let changes = vec![set_number("admission.headroom_pct", 150.0)];
    let response = daemon.patch_settings(Revision::of(&written), changes).await;

    let refusal = expect_refusal(response, StatusCode::UNPROCESSABLE_ENTITY).await;
    let keys: Vec<&str> = refusal.problems.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(keys, ["admission.headroom_pct"]);
    assert_eq!(refusal.revision, None);
    assert_eq!(daemon.config_text().as_bytes(), written);
    daemon.stop().await;
}

#[tokio::test]
async fn test_settings_body_that_is_not_the_routes_json_is_a_settings_refusal() {
    let daemon = Daemon::start().await;
    daemon.write_settings("");
    // A newer panel's change kind this allotd does not know.
    let unknown_kind = r#"{"revision":"0000000000000000","changes":[{"kind":"rename"}]}"#;
    for (path, body) in [
        ("/settings", unknown_kind),
        ("/settings/preview", "not json"),
        ("/classify", "{}"),
    ] {
        let request = if path == "/settings" {
            daemon.patch(path)
        } else {
            daemon.post(path)
        };
        let response = request
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_client_error(), "{path}");
        let refusal: SettingsRefusal = response.json().await.unwrap();
        assert!(refusal.problems.is_empty(), "{path}");
        assert!(!refusal.message.is_empty(), "{path}");
    }
    daemon.stop().await;
}

#[tokio::test]
async fn test_patch_settings_after_a_hand_edit_is_a_conflict() {
    let daemon = Daemon::start().await;
    let read = daemon.write_settings("[admission]\nheadroom_pct = 25\n");
    let edited = daemon.write_settings("[admission]\nheadroom_pct = 20\n");

    let changes = vec![set_number("admission.headroom_pct", 30.0)];
    let response = daemon.patch_settings(Revision::of(&read), changes).await;

    let refusal = expect_refusal(response, StatusCode::CONFLICT).await;
    assert_eq!(refusal.revision, Some(Revision::of(&edited)));
    assert!(refusal.problems.is_empty());
    assert_eq!(daemon.config_text().as_bytes(), edited);
    daemon.stop().await;
}

#[tokio::test]
async fn test_patch_settings_reset_removes_the_key_and_its_default_applies() {
    let daemon = Daemon::start().await;
    let written = daemon.write_settings("[admission]\nheadroom_pct = 25\n");

    let changes = vec![Change::Reset {
        key: "admission.headroom_pct".into(),
    }];
    let state: SettingsState = ok(daemon.patch_settings(Revision::of(&written), changes).await)
        .json()
        .await
        .unwrap();

    assert!(!daemon.config_text().contains("headroom_pct"));
    assert_eq!(number(&state, "admission.headroom_pct"), default_headroom());
    daemon.stop().await;
}

#[tokio::test]
async fn test_rejected_file_is_reported_and_cannot_be_changed() {
    let daemon = Daemon::start().await;
    let written = daemon.write_settings("[admission]\nheadroom_pct = 150\n");

    let state = daemon.settings().await;

    let reason = state.file_error.clone().expect("the file is rejected");
    assert!(reason.contains("admission.headroom_pct"), "{reason}");
    assert_eq!(number(&state, "admission.headroom_pct"), default_headroom());
    let changes = vec![set_number("leases.heartbeat_ttl_secs", 120.0)];
    let response = daemon.patch_settings(Revision::of(&written), changes).await;
    let refusal = expect_refusal(response, StatusCode::UNPROCESSABLE_ENTITY).await;
    assert_eq!(refusal.problems[0].key, "admission.headroom_pct");
    daemon.stop().await;
}

#[tokio::test]
async fn test_preview_counts_with_the_unsaved_headroom() {
    let daemon = Daemon::start().await;
    let written = daemon.write_settings("");

    let saved: FitPreview = ok(daemon.preview(Vec::new()).await).json().await.unwrap();
    let changes = vec![set_number("admission.headroom_pct", 50.0)];
    let proposed: FitPreview = ok(daemon.preview(changes).await).json().await.unwrap();

    // The fake machine has 16 cores.
    assert_eq!(saved.kept_free.cores, 16.0 * (default_headroom() / 100.0));
    assert_eq!(proposed.kept_free.cores, 8.0);
    let names: Vec<&str> = proposed.classes.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["light", "cpu_heavy", "gpu_heavy", "mixed"]);
    let cpu_heavy = &proposed.classes[1];
    assert_eq!((cpu_heavy.idle.full, cpu_heavy.idle.shrunk), (1, 0));
    assert_eq!(
        daemon.config_text().as_bytes(),
        written,
        "a preview wrote the file"
    );
    daemon.stop().await;
}

#[tokio::test]
async fn test_preview_invalid_change_names_the_key() {
    let daemon = Daemon::start().await;

    let changes = vec![set_number("admission.headroom_pct", -1.0)];
    let response = daemon.preview(changes).await;

    let refusal = expect_refusal(response, StatusCode::UNPROCESSABLE_ENTITY).await;
    assert_eq!(refusal.problems[0].key, "admission.headroom_pct");
    daemon.stop().await;
}

#[tokio::test]
async fn test_classify_uses_the_saved_rules_or_the_edited_ones() {
    let daemon = Daemon::start().await;
    daemon.write_settings("");
    let line = "make -j8 && cargo build --release";

    let saved: CommandTest = ok(daemon.classify(line, Shell::Bash, Vec::new()).await)
        .json()
        .await
        .unwrap();
    let rules = vec![HeavyRule {
        program: "make".into(),
        args: Vec::new(),
        class: JobClass::CpuHeavy,
    }];
    let changes = vec![Change::SetRules { rules }];
    let edited: CommandTest = ok(daemon.classify(line, Shell::PowerShell, changes).await)
        .json()
        .await
        .unwrap();

    let kinds = |test: &CommandTest| -> Vec<(CommandKind, Option<u32>)> {
        let commands = test.commands.iter();
        commands.map(|c| (c.kind, c.rule_index)).collect()
    };
    assert_eq!(
        kinds(&saved),
        [(CommandKind::NotHeavy, None), (CommandKind::Heavy, Some(0))]
    );
    assert_eq!(
        kinds(&edited),
        [(CommandKind::Heavy, Some(0)), (CommandKind::NotHeavy, None)]
    );
    let lease = edited.lease.expect("make is heavy");
    assert_eq!(lease.class, JobClass::CpuHeavy);
    daemon.stop().await;
}
