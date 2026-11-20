//! `POST /shutdown` on an in-process allotd: a panel's Stop writes the stop record before
//! allotd stops, and a body allotd cannot read stops nothing.

mod common;

use std::time::Duration;

use allot_shared::{
    api::ShutdownRequest,
    stop::{StopRecord, StoppedBy},
};
use common::Daemon;
use reqwest::StatusCode;

impl Daemon {
    fn stop_file(&self) -> std::path::PathBuf {
        self.dir.path().join("stopped.json")
    }

    /// Waits for the server to end by itself, as it does after an accepted shutdown.
    async fn wait_stopped(&mut self) {
        let server = self.server.take().unwrap();
        tokio::time::timeout(Duration::from_secs(10), server)
            .await
            .expect("allotd stops after the shutdown")
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn test_shutdown_from_a_panel_writes_the_stop_record_then_stops() {
    let mut daemon = Daemon::start().await;

    let request = ShutdownRequest {
        by: StoppedBy::Panel,
    };
    let response = daemon
        .post("/shutdown")
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let record = StopRecord::read(&daemon.stop_file()).expect("a record before the answer");
    assert_eq!(record.by, StoppedBy::Panel);
    daemon.wait_stopped().await;
}

#[tokio::test]
async fn test_shutdown_without_a_body_stops_without_a_record() {
    let mut daemon = Daemon::start().await;

    let response = daemon.post("/shutdown").send().await.unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    daemon.wait_stopped().await;
    assert!(!daemon.stop_file().exists());
}

#[tokio::test]
async fn test_shutdown_with_an_unreadable_body_keeps_running() {
    let daemon = Daemon::start().await;

    let response = daemon
        .post("/shutdown")
        .header("content-type", "application/json")
        .body(r#"{"by":"someone"}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!daemon.stop_file().exists());
    assert_eq!(daemon.get("/snapshot").await.status(), StatusCode::OK);
    daemon.stop().await;
}
