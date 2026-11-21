//! The settings routes: read and change `config.toml`, and explain unsaved values. allotd is
//! the file's only writer. A write goes into effect at once, so the config poll later finds
//! nothing new.
//!
//! Every refusal answers a [`SettingsRefusal`] as JSON, so a panel can show each rejected
//! value under its field.

use std::path::PathBuf;

use allot_core::explain::{self, Invalid};
use allot_shared::{
    Error,
    api::{ClassifyRequest, PreviewRequest, SettingsPatch, SettingsRefusal, SettingsState},
    config::{Config, FieldError},
    config_write::{self, Change, Revision, WriteError},
    explain::{CommandTest, FitPreview},
    platform::DAEMON_LOG_TEXT,
    settings::{SettingValue, SettingsSpec},
};
use anyhow::anyhow;
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    error::AppError,
    state::{AppState, Shared},
};

/// The only setting that takes effect at a restart: the listener stays bound to its port.
const PORT_KEY: &str = "daemon.port";

/// Why a settings request was refused.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// Reading or writing the file failed, or the changes were refused.
    #[error(transparent)]
    Write(#[from] WriteError),
    /// The proposed values hold a value allotd rejects.
    #[error(transparent)]
    Invalid(#[from] Invalid),
    /// The daemon was started without a config file.
    #[error("allotd runs without a config file, so its settings cannot be read or changed")]
    NoFile,
    /// No machine sample has arrived yet.
    #[error(transparent)]
    App(#[from] AppError),
    /// The body is not the JSON the route takes, such as a change kind this allotd does not
    /// know yet.
    #[error("{}", .0.body_text())]
    Malformed(#[from] JsonRejection),
}

impl IntoResponse for SettingsError {
    fn into_response(self) -> Response {
        let message = self.to_string();
        let refusal = |status: StatusCode, problems: Vec<FieldError>, revision| {
            let body = SettingsRefusal {
                message: message.clone(),
                problems,
                revision,
            };
            (status, Json(body)).into_response()
        };
        match self {
            Self::Write(WriteError::Invalid(problems)) | Self::Invalid(Invalid { problems }) => {
                refusal(StatusCode::UNPROCESSABLE_ENTITY, problems, None)
            }
            Self::Write(WriteError::File(_)) => {
                refusal(StatusCode::UNPROCESSABLE_ENTITY, Vec::new(), None)
            }
            Self::Write(WriteError::Conflict { current }) => {
                refusal(StatusCode::CONFLICT, Vec::new(), Some(current))
            }
            Self::Write(WriteError::Held(_)) => refusal(StatusCode::LOCKED, Vec::new(), None),
            Self::Malformed(rejection) => refusal(rejection.status(), Vec::new(), None),
            Self::NoFile => refusal(StatusCode::SERVICE_UNAVAILABLE, Vec::new(), None),
            Self::App(AppError::Unavailable) => {
                refusal(StatusCode::SERVICE_UNAVAILABLE, Vec::new(), None)
            }
            Self::Write(error) => internal(&error.into()),
            Self::App(AppError::Internal(error)) => internal(&error),
            Self::App(error) => error.into_response(),
        }
    }
}

/// A failure that is logged, never shown: the client learns only where to look.
fn internal(error: &anyhow::Error) -> Response {
    tracing::error!("settings request failed: {error:#}");
    let body = SettingsRefusal {
        message: format!("internal error; the details are in {DAEMON_LOG_TEXT}"),
        problems: Vec::new(),
        revision: None,
    };
    (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
}

/// `GET /settings`: the spec, the file's values and revision, and what waits for a restart.
pub async fn read(State(state): State<AppState>) -> Result<Json<SettingsState>, SettingsError> {
    let path = config_path(&state)?;
    let snapshot = blocking({
        let path = path.clone();
        move || config_write::read(&path)
    })
    .await?;
    let (config, file_error) = match Config::from_toml(&snapshot.text) {
        Ok(config) => (config, None),
        Err(reason) => {
            let rejected = Error::ConfigFile {
                path,
                reason: Box::new(reason),
            };
            (state.settings().config.clone(), Some(rejected.to_string()))
        }
    };
    Ok(Json(settings_state(
        &state,
        &config,
        snapshot.revision,
        file_error,
    )))
}

/// `PATCH /settings`: writes the changes if the file still has the request's revision, and
/// puts the result into effect.
pub async fn change(
    State(state): State<AppState>,
    patch: Result<Json<SettingsPatch>, JsonRejection>,
) -> Result<Json<SettingsState>, SettingsError> {
    let Json(patch) = patch?;
    let path = config_path(&state)?;
    let writing = state.config_writes.lock().await;
    let applied = blocking({
        let path = path.clone();
        move || config_write::apply(&path, patch.revision, &patch.changes)
    })
    .await?;
    if applied.written {
        tracing::info!(revision = %applied.revision, "settings written to {}", path.display());
        state
            .reload_config(applied.config.clone(), "the settings routes")
            .await;
    }
    drop(writing);
    Ok(Json(settings_state(
        &state,
        &applied.config,
        applied.revision,
        None,
    )))
}

/// `POST /settings/preview`: what fits on this machine if the changes were saved.
pub async fn preview(
    State(state): State<AppState>,
    request: Result<Json<PreviewRequest>, JsonRejection>,
) -> Result<Json<FitPreview>, SettingsError> {
    let Json(request) = request?;
    let proposed = proposal(&state, request.changes).await?;
    let latest = state.latest().ok_or(AppError::Unavailable)?;
    let ledger = state.ledger.lock().await;
    Ok(Json(explain::fit_preview(
        &proposed,
        &ledger,
        &latest.snapshot,
    )?))
}

/// `POST /classify`: how the hook treats a command line if the changes were saved.
pub async fn classify(
    State(state): State<AppState>,
    request: Result<Json<ClassifyRequest>, JsonRejection>,
) -> Result<Json<CommandTest>, SettingsError> {
    let Json(request) = request?;
    let proposed = proposal(&state, request.changes).await?;
    Ok(Json(explain::test_command(
        &proposed,
        &request.line,
        request.shell,
    )?))
}

fn config_path(state: &Shared) -> Result<PathBuf, SettingsError> {
    state.config_path.clone().ok_or(SettingsError::NoFile)
}

/// The file's config with `changes` applied, without writing anything.
async fn proposal(state: &Shared, changes: Vec<Change>) -> Result<Config, SettingsError> {
    let path = config_path(state)?;
    let edited = blocking(move || {
        let snapshot = config_write::read(&path)?;
        config_write::edit(&snapshot.text, &changes)
    })
    .await?;
    Ok(edited.config)
}

/// Runs file work off the async runtime: a held file is retried for about a second.
async fn blocking<T, F>(work: F) -> Result<T, SettingsError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, WriteError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::Internal(anyhow!("the settings task failed: {error}")))?
        .map_err(SettingsError::from)
}

fn settings_state(
    state: &Shared,
    config: &Config,
    revision: Revision,
    file_error: Option<String>,
) -> SettingsState {
    let spec = SettingsSpec::new();
    let pending_restart = (config.daemon.port != state.listening_port)
        .then(|| PORT_KEY.to_owned())
        .into_iter()
        .collect();
    SettingsState {
        values: SettingValue::all(&spec, config),
        spec,
        revision,
        file_error,
        listening_port: u32::from(state.listening_port),
        pending_restart,
    }
}
