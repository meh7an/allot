//! HTTP routes. Every route except `/health` requires the bearer token.

use std::time::{Duration, Instant};

use anyhow::anyhow;
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post},
};
use rh_core::ledger::{LedgerError, Transition};
use rh_shared::{
    api::{
        AcquireRequest, AcquireResponse, CanFitRequest, HealthResponse, HeartbeatResponse,
        LeaseOwner, LeaseQuery, LeasesResponse, ProfileSpec, RegisterSessionRequest,
        ReleaseResponse, SessionsResponse, Verdict,
    },
    config::Config,
    types::{JobClass, JobProfile, Lease, LeaseId, LeaseState, SessionKey, Snapshot},
};

use crate::{
    db::repo::{self, LeaseDraft},
    error::AppError,
    state::{AppState, now_ms},
};

// Longer waits risk client and proxy timeouts; the caller can heartbeat and ask again.
const MAX_ACQUIRE_WAIT: Duration = Duration::from_secs(120);

/// Every route, with the token check on all but `/health`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/snapshot", get(snapshot))
        .route("/can-fit", post(can_fit))
        .route("/leases", get(list_leases).post(acquire))
        .route("/leases/{id}", delete(release))
        .route("/leases/{id}/heartbeat", post(heartbeat))
        .route("/sessions", get(sessions).post(register_session))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token))
        .route("/health", get(health))
        .with_state(state)
}

async fn require_token(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let presented = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if presented != Some(state.token.as_str()) {
        return Err(AppError::Unauthorized);
    }
    Ok(next.run(request).await)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        pid: std::process::id(),
        uptime_secs: state.started.elapsed().as_secs(),
    })
}

async fn snapshot(State(state): State<AppState>) -> Result<Json<Snapshot>, AppError> {
    let latest = state.latest().ok_or(AppError::Unavailable)?;
    Ok(Json(latest.snapshot.clone()))
}

async fn sessions(State(state): State<AppState>) -> Result<Json<SessionsResponse>, AppError> {
    let latest = state.latest().ok_or(AppError::Unavailable)?;
    Ok(Json(SessionsResponse {
        sessions: latest.snapshot.sessions.clone(),
    }))
}

async fn register_session(
    State(state): State<AppState>,
    Json(request): Json<RegisterSessionRequest>,
) -> StatusCode {
    state.register(request.holder);
    StatusCode::NO_CONTENT
}

async fn can_fit(
    State(state): State<AppState>,
    Json(request): Json<CanFitRequest>,
) -> Result<Json<Verdict>, AppError> {
    let (profile, _) = resolve_profile(&state.config, &request.profile)?;
    let latest = state.latest().ok_or(AppError::Unavailable)?;
    let ledger = state.ledger.lock().await;
    Ok(Json(ledger.can_fit(
        &profile,
        request.kind,
        &latest.snapshot,
        now_ms(),
    )))
}

async fn list_leases(
    State(state): State<AppState>,
    Query(query): Query<LeaseQuery>,
) -> Result<Json<LeasesResponse>, AppError> {
    let leases = state
        .with_db(move |conn| repo::list_leases(conn, &query))
        .await?;
    Ok(Json(LeasesResponse { leases }))
}

async fn acquire(
    State(state): State<AppState>,
    Json(request): Json<AcquireRequest>,
) -> Result<Json<AcquireResponse>, AppError> {
    let (profile, class) = resolve_profile(&state.config, &request.profile)?;
    state.register(request.holder.clone());
    let now = now_ms();
    let ttl_ms = i64::try_from(state.config.leases.heartbeat_ttl_secs.saturating_mul(1_000))
        .unwrap_or(i64::MAX);
    let draft = LeaseDraft {
        holder: request.holder,
        kind: request.kind,
        class,
        profile,
        label: request.label,
        requested_at_ms: now,
        expires_at_ms: now.saturating_add(ttl_ms),
    };

    let mut changes = state.changes.subscribe();
    let lease = state
        .with_db(move |conn| repo::insert_lease(conn, &draft))
        .await?;
    let id = lease.id;
    {
        let mut ledger = state.ledger.lock().await;
        ledger.enqueue(lease)?;
        state.tick(&mut ledger).await?;
    }

    let wait = Duration::from_secs(request.wait_timeout_secs).min(MAX_ACQUIRE_WAIT);
    let deadline = Instant::now() + wait;
    while is_queued(&state, id).await {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match tokio::time::timeout(remaining, changes.changed()).await {
            Ok(Ok(())) => {}
            _ => break,
        }
    }

    let lease = state
        .with_db(move |conn| repo::get_lease(conn, id))
        .await?
        .ok_or_else(|| anyhow!("lease {} vanished from the database", id.0))?;
    let verdict = lease_verdict(&state, &lease).await;
    Ok(Json(AcquireResponse { lease, verdict }))
}

async fn heartbeat(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(owner): Query<LeaseOwner>,
) -> Result<Json<HeartbeatResponse>, AppError> {
    let id = LeaseId(id);
    let mut ledger = state.ledger.lock().await;
    match ledger.heartbeat(id, &owner.session, now_ms()) {
        Ok(lease) => {
            let (lease_state, expires_at_ms) = (lease.state, lease.expires_at_ms);
            state
                .with_db(move |conn| repo::renew_lease(conn, id, expires_at_ms))
                .await?;
            Ok(Json(HeartbeatResponse {
                state: lease_state,
                expires_at_ms,
            }))
        }
        Err(LedgerError::NotFound(_)) => {
            drop(ledger);
            let lease = stored_lease(&state, id, &owner.session).await?;
            Ok(Json(HeartbeatResponse {
                state: lease.state,
                expires_at_ms: lease.expires_at_ms,
            }))
        }
        Err(error) => Err(error.into()),
    }
}

async fn release(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(owner): Query<LeaseOwner>,
) -> Result<Json<ReleaseResponse>, AppError> {
    let id = LeaseId(id);
    let now = now_ms();
    let mut ledger = state.ledger.lock().await;
    match ledger.release(id, &owner.session, now) {
        Ok(_) => {
            let released = Transition {
                id,
                to: LeaseState::Released,
                reason: "released by holder".to_owned(),
            };
            state.persist(&[released], now).await?;
            state.notify_changes();
            // The freed room may let queued leases start right away.
            state.tick(&mut ledger).await?;
        }
        Err(LedgerError::NotFound(_)) => {}
        Err(error) => return Err(error.into()),
    }
    drop(ledger);
    let lease = stored_lease(&state, id, &owner.session).await?;
    Ok(Json(ReleaseResponse { lease }))
}

/// Resolves a class through the config and checks the values.
fn resolve_profile(
    config: &Config,
    spec: &ProfileSpec,
) -> Result<(JobProfile, Option<JobClass>), AppError> {
    let (profile, class) = match spec {
        ProfileSpec::Class(class) => (*config.classes.get(*class), Some(*class)),
        ProfileSpec::Custom(profile) => (*profile, None),
        _ => return Err(AppError::Validation("unsupported profile form".to_owned())),
    };
    if let Some(reason) = profile.invalid_reason() {
        return Err(AppError::Validation(format!("profile: {reason}")));
    }
    Ok((profile, class))
}

async fn is_queued(state: &AppState, id: LeaseId) -> bool {
    let ledger = state.ledger.lock().await;
    ledger
        .leases()
        .iter()
        .any(|lease| lease.id == id && lease.state == LeaseState::Queued)
}

/// Why `lease` runs or waits, from the ledger while it tracks the lease.
async fn lease_verdict(state: &AppState, lease: &Lease) -> Verdict {
    let explained = {
        let ledger = state.ledger.lock().await;
        state
            .latest()
            .and_then(|latest| ledger.verdict_for(lease.id, &latest.snapshot, now_ms()))
    };
    explained.unwrap_or_else(|| {
        let reason = if lease.state == LeaseState::Queued {
            "waiting for the first machine sample".to_owned()
        } else {
            format!("lease is {}", lease.state.as_str())
        };
        Verdict {
            fits: false,
            reasons: vec![reason],
            wait_estimate_secs: None,
        }
    })
}

/// A lease from the database, refused when it belongs to another session.
async fn stored_lease(
    state: &AppState,
    id: LeaseId,
    session: &SessionKey,
) -> Result<Lease, AppError> {
    let lease = state
        .with_db(move |conn| repo::get_lease(conn, id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("lease {} does not exist", id.0)))?;
    if &lease.holder.session != session {
        return Err(LedgerError::NotOwner(id).into());
    }
    Ok(lease)
}
