//! A stand-in for github.com's releases, which the update check of an in-process allotd reads
//! through its `ALLOT_UPDATE_BASE` equivalent: `GET /releases/latest` redirects to a tag's
//! page, and every request that arrives is written down.

use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use allot_updater::{Limits, Source, Updater};
use allotd::UpdateOptions;
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{StatusCode, Uri, header::LOCATION},
    response::Response,
};
use tokio::{net::TcpListener, task::JoinHandle};

/// What the server answers to every request.
#[derive(Debug, Clone)]
enum Answer {
    /// `releases/latest` redirects to this tag, everything else is a 404.
    Tag(String),
    /// Every request fails with this status.
    Failing(u16),
}

struct Shared {
    answer: Mutex<Answer>,
    requests: Mutex<Vec<String>>,
}

/// The server, stopped when dropped.
pub struct ReleaseServer {
    /// The address to read releases from, as `ALLOT_UPDATE_BASE` would name it.
    pub base: String,
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl ReleaseServer {
    /// A server whose newest release is `tag`, such as `v99.0.0`.
    pub async fn start(tag: &str) -> Self {
        Self::start_with(Answer::Tag(tag.to_owned())).await
    }

    /// A server that answers every request with `status`.
    pub async fn failing(status: u16) -> Self {
        Self::start_with(Answer::Failing(status)).await
    }

    async fn start_with(answer: Answer) -> Self {
        let shared = Arc::new(Shared {
            answer: Mutex::new(answer),
            requests: Mutex::new(Vec::new()),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/releases", listener.local_addr().unwrap());
        let app = Router::new().fallback(respond).with_state(shared.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { base, shared, task }
    }

    /// Makes `tag` the newest release from now on.
    pub fn publish(&self, tag: &str) {
        *self.shared.answer.lock().unwrap() = Answer::Tag(tag.to_owned());
    }

    /// The paths asked for so far, in order.
    pub fn requests(&self) -> Vec<String> {
        self.shared.requests.lock().unwrap().clone()
    }

    /// Update options that read this server, check at once and then poll quickly. A failed
    /// check is tried again only after an hour, so a test sees each request it expects.
    pub fn options(&self, folder: &Path) -> UpdateOptions {
        let source = Source::new(&self.base).unwrap();
        UpdateOptions {
            updater: Ok(Arc::new(Updater::new(source, Limits::default()))),
            first_check_after: Duration::ZERO,
            poll: Duration::from_millis(20),
            ..UpdateOptions::inert(folder)
        }
    }
}

impl Drop for ReleaseServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn respond(State(shared): State<Arc<Shared>>, uri: Uri) -> Response {
    shared.requests.lock().unwrap().push(uri.path().to_owned());
    let answer = shared.answer.lock().unwrap().clone();
    let (status, location) = match answer {
        Answer::Tag(tag) if uri.path() == "/releases/latest" => {
            (StatusCode::FOUND, Some(format!("/releases/tag/{tag}")))
        }
        Answer::Tag(_) => (StatusCode::NOT_FOUND, None),
        Answer::Failing(status) => (StatusCode::from_u16(status).unwrap(), None),
    };
    let mut response = Response::builder().status(status);
    if let Some(location) = location {
        response = response.header(LOCATION, location);
    }
    response.body(Body::empty()).unwrap()
}
