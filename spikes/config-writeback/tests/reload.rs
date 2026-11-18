//! The real allotd reload loop against the writer: each write goes into effect exactly once.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use allot_core::sampler::{RawSample, Sampler};
use allot_shared::{config::Config, types::MemStats};
use allotd::{DaemonOptions, Maintenance, Scopes, db::open_pool};
use config_writeback::{
    edit,
    file::{self, Retry},
    rules,
};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};
use toml_edit::DocumentMut;
use tracing_subscriber::util::SubscriberInitExt;

const POLL: Duration = Duration::from_millis(20);

/// Log lines collected from the daemon's tasks.
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

impl Logs {
    fn count(&self, needle: &str) -> usize {
        let bytes = self.0.lock().unwrap();
        String::from_utf8_lossy(&bytes).matches(needle).count()
    }

    fn reloads(&self) -> usize {
        self.count("config reloaded")
    }

    fn rejections(&self) -> usize {
        self.count("config rejected")
    }
}

impl Write for Logs {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct IdleMachine;

impl Sampler for IdleMachine {
    fn sample(&mut self) -> allot_shared::Result<RawSample> {
        let taken_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        Ok(RawSample {
            taken_at_ms,
            cpu_total_pct: 0.0,
            per_core_pct: vec![0.0; 8],
            memory: MemStats {
                total_mb: 16_384,
                used_mb: 4_096,
                available_mb: 12_288,
                commit_used_mb: 4_096,
                commit_limit_mb: 32_768,
                reclaimable_mb: 0,
            },
            processes: vec![],
            gpus: vec![],
            drives: vec![],
        })
    }
}

fn fixture() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hand_edited.toml");
    fs::read_to_string(path).unwrap()
}

fn apply(path: &Path, change: impl FnOnce(&mut DocumentMut)) {
    let (text, revision) = file::read(path).unwrap();
    let mut doc: DocumentMut = text.parse().unwrap();
    change(&mut doc);
    file::write(path, &doc.to_string(), revision, Retry::NONE).unwrap();
}

async fn wait_for(what: &str, check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(POLL).await;
    }
}

/// Long enough for many more polls, so a second pick-up of one write would show.
async fn settle() {
    tokio::time::sleep(POLL * 15).await;
}

#[tokio::test]
async fn test_each_write_reloads_exactly_once() {
    let logs = Logs::default();
    let writer = logs.clone();
    let _logging = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish()
        .set_default();

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, fixture()).unwrap();
    let options = DaemonOptions {
        config: Config::load(&path).unwrap(),
        token: "spike".into(),
        pool: open_pool(&dir.path().join("allot.db")).unwrap(),
        sampler: Box::new(|| Ok(Box::new(IdleMachine) as Box<dyn Sampler>)),
        sample_interval: POLL,
        maintenance: Maintenance {
            config_path: Some(path.clone()),
            config_poll: POLL,
            prune_every: Duration::from_secs(3_600),
        },
        scopes: Scopes::disabled(),
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(allotd::serve(listener, options, async {
        let _ = stopped.await;
    }));

    settle().await;
    assert_eq!(logs.reloads(), 0);

    let changes: [fn(&mut DocumentMut); 4] = [
        |doc| edit::set(doc, &["admission", "headroom_pct"], 30.0),
        |doc| edit::set(doc, &["estimates", "min_runs"], 5),
        |doc| edit::set(doc, &["presets", "render", "min_cores"], 2.0),
        |doc| rules::set(doc, &[]),
    ];
    for (index, change) in changes.into_iter().enumerate() {
        apply(&path, change);
        wait_for("the reload", || logs.reloads() > index).await;
        settle().await;
        assert_eq!(
            logs.reloads(),
            index + 1,
            "write {index} reloaded more than once"
        );
    }

    // A write that changes only comments changes the file but not the config: nothing applies.
    apply(&path, |doc| {
        let table = doc["admission"].as_table_mut().unwrap();
        table
            .decor_mut()
            .set_prefix("\n# set from the Settings window\n");
    });
    settle().await;
    assert_eq!(logs.reloads(), changes.len());

    // Contrast: a plain in-place save that lands in two steps is seen half-written.
    let (text, _) = file::read(&path).unwrap();
    let full = text.replace("headroom_pct   =   30.0", "headroom_pct   =   35.0");
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .unwrap();
    let half = full.len() / 2;
    file.write_all(&full.as_bytes()[..half]).unwrap();
    file.flush().unwrap();
    let before = logs.reloads() + logs.rejections();
    tokio::time::sleep(POLL * 10).await;
    file.write_all(&full.as_bytes()[half..]).unwrap();
    drop(file);
    wait_for("the second half", || {
        logs.reloads() + logs.rejections() >= before + 2
    })
    .await;
    println!(
        "in-place save: {} reloads, {} rejections",
        logs.reloads() - changes.len(),
        logs.rejections()
    );

    let _ = stop.send(());
    server.await.unwrap().unwrap();
}
