//! A release server on a loopback port, answering from a table, for the updater's tests.

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use allot_shared::release::Asset;
use sha2::{Digest, Sha256};

/// What the server answers for one path.
pub enum Reply {
    /// 200 with this body.
    Body(Vec<u8>),
    /// 302 to this address.
    Redirect(String),
    /// This status with no body.
    Status(u16),
    /// Announces `claimed` bytes, sends `sent` and hangs up.
    Cut { claimed: usize, sent: Vec<u8> },
    /// Announces `claimed` bytes, sends `sent`, then goes quiet for `pause`.
    Stall {
        claimed: usize,
        sent: Vec<u8>,
        pause: Duration,
    },
    /// 200 with no length: sends `bytes` bytes and hangs up.
    Unbounded { bytes: usize },
}

pub struct Server {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(routes: Vec<(String, Reply)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let routes: Arc<HashMap<String, Reply>> = Arc::new(routes.into_iter().collect());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (requests, stop) = (Arc::clone(&requests), Arc::clone(&stop));
            thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let (routes, requests) = (Arc::clone(&routes), Arc::clone(&requests));
                    thread::spawn(move || serve(stream, &routes, &requests));
                }
            })
        };
        Self {
            port,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    /// The release address, the same shape as `https://github.com/meh7an/allot/releases`.
    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}/releases", self.port)
    }

    /// The paths asked for, in order.
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accept loop so it sees the flag.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The address of a port nothing listens on.
pub fn closed_base() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}/releases")
}

fn serve(mut stream: TcpStream, routes: &HashMap<String, Reply>, requests: &Mutex<Vec<String>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Some(path) = read_path(&mut stream) else {
        return;
    };
    requests.lock().unwrap().push(path.clone());
    let _ = match routes.get(&path) {
        None => respond(&mut stream, 404, &[], &[]),
        Some(Reply::Body(body)) => respond(&mut stream, 200, &[], body),
        Some(Reply::Redirect(to)) => respond(&mut stream, 302, &[("Location", to)], &[]),
        Some(Reply::Status(status)) => respond(&mut stream, *status, &[], &[]),
        Some(Reply::Cut { claimed, sent }) => announce(&mut stream, Some(*claimed), sent),
        Some(Reply::Stall {
            claimed,
            sent,
            pause,
        }) => {
            let result = announce(&mut stream, Some(*claimed), sent);
            thread::sleep(*pause);
            result
        }
        Some(Reply::Unbounded { bytes }) => announce(&mut stream, None, &vec![b'x'; *bytes]),
    };
}

fn read_path(stream: &mut TcpStream) -> Option<String> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).ok()?;
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head);
    text.lines()
        .next()?
        .split_whitespace()
        .nth(1)
        .map(str::to_owned)
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    headers: &[(&str, &String)],
    body: &[u8],
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} Test\r\nConnection: close\r\n");
    head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)
}

fn announce(stream: &mut TcpStream, claimed: Option<usize>, sent: &[u8]) -> std::io::Result<()> {
    let mut head = String::from("HTTP/1.1 200 Test\r\nConnection: close\r\n");
    if let Some(claimed) = claimed {
        head.push_str(&format!("Content-Length: {claimed}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(sent)?;
    stream.flush()
}

/// The lower-case hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The routes of `asset` and its `.sha256` in release `version`: `body` and a checksum line
/// the way the workflows write it, with `listed` as the name in it.
pub fn asset_routes(
    version: &str,
    asset: Asset,
    body: &[u8],
    listed: &str,
) -> Vec<(String, Reply)> {
    let line = format!("{}  {listed}\n", sha256_hex(body));
    vec![
        (
            asset_path(version, &asset.file_name(version)),
            Reply::Body(body.to_vec()),
        ),
        (
            asset_path(version, &asset.checksum_name(version)),
            Reply::Body(line.into_bytes()),
        ),
    ]
}

pub fn asset_path(version: &str, file: &str) -> String {
    format!("/releases/download/v{version}/{file}")
}

pub fn latest_route(tag: &str) -> (String, Reply) {
    (
        "/releases/latest".to_owned(),
        Reply::Redirect(format!("/releases/tag/{tag}")),
    )
}
