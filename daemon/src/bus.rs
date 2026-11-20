//! The D-Bus interface of the GNOME extension. extensions.gnome.org asks extensions to reach
//! background services over D-Bus rather than HTTP, so on Linux allotd also owns
//! `tech.mehran.Allot` on the session bus. Its methods run the same handlers as the HTTP routes
//! and answer the same JSON, which `contract/` describes; a refusal is a D-Bus error named
//! after its HTTP status. Every other client keeps HTTP.
//!
//! The session bus only admits this user's processes, and allotd still asks the bus who calls
//! and refuses any other user, so no token is needed.

use std::{
    collections::HashSet,
    sync::{Mutex, PoisonError},
};

use allot_shared::{
    api::{ClassifyRequest, PreviewRequest, SettingsPatch, ShutdownRequest},
    stop::StoppedBy,
};
use axum::{
    Json,
    body::to_bytes,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use tokio::{sync::watch, task::JoinHandle};
use zbus::{
    Connection, DBusError,
    connection::Builder,
    fdo::{RequestNameFlags, RequestNameReply},
    interface,
    message::Header,
    object_server::SignalEmitter,
};

use crate::{routes, settings, state::AppState, ui};

/// The well-known name allotd owns on the session bus.
pub const NAME: &str = "tech.mehran.Allot";
/// The object that carries the interface.
pub const PATH: &str = "/tech/mehran/Allot";
/// The interface; the trailing 1 is its version, raised only for a change old panels cannot
/// follow.
pub const INTERFACE: &str = "tech.mehran.Allot1";

const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";

/// Which bus allotd offers the interface on.
#[derive(Debug, Clone)]
pub enum PanelBus {
    /// No interface, as in the HTTP tests.
    Off,
    /// The user's session bus.
    Session,
    /// The bus at this D-Bus address, such as a test's private bus.
    Address(String),
}

/// The interface while allotd serves it.
#[derive(Debug)]
pub(crate) struct Offered {
    connection: Connection,
    announcer: JoinHandle<()>,
}

impl Offered {
    /// Gives up the name, so a panel sees allotd stop at once rather than when the process
    /// ends.
    pub(crate) async fn close(self) {
        self.announcer.abort();
        if let Err(error) = self.connection.release_name(NAME).await {
            tracing::warn!("giving up {NAME} on D-Bus: {error}");
        }
    }
}

/// Serves the interface on `bus`. When the bus is missing (as inside Claude's sandbox) or the
/// name is taken, allotd logs why and keeps serving HTTP; only the GNOME extension then cannot
/// reach it.
pub(crate) async fn offer(state: AppState, bus: &PanelBus) -> Option<Offered> {
    let builder = match bus {
        PanelBus::Off => return None,
        PanelBus::Session => Builder::session(),
        PanelBus::Address(address) => Builder::address(address.as_str()),
    };
    match connect(state, builder).await {
        Ok(offered) => {
            tracing::info!("serving {INTERFACE} on D-Bus as {NAME}");
            Some(offered)
        }
        Err(error) => {
            tracing::warn!("not on D-Bus, so the GNOME extension cannot reach allotd: {error}");
            None
        }
    }
}

async fn connect(state: AppState, builder: Builder<'_>) -> zbus::Result<Offered> {
    let connection = builder.build().await?;
    let own_name = connection
        .unique_name()
        .ok_or_else(|| zbus::Error::Failure("the bus gave allotd no name".to_owned()))?;
    let own_uid = unix_user(&connection, own_name.as_str()).await?;
    let changes = state.changes.subscribe();
    let panel = Panel {
        state,
        own_uid,
        callers: Mutex::new(HashSet::new()),
    };
    connection.object_server().at(PATH, panel).await?;
    // Another allotd keeps its name; replacing it would leave its panels talking to the wrong
    // daemon.
    match connection
        .request_name_with_flags(NAME, RequestNameFlags::DO_NOT_QUEUE)
        .await?
    {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
        RequestNameReply::InQueue | RequestNameReply::Exists => return Err(zbus::Error::NameTaken),
    }
    let announcer = tokio::spawn(announce_jobs(connection.clone(), changes));
    Ok(Offered {
        connection,
        announcer,
    })
}

/// Emits `JobsChanged` after each change of the leases, so a panel polls at once instead of at
/// its next tick. Changes that come faster than the signals go out are sent as one.
async fn announce_jobs(connection: Connection, mut changes: watch::Receiver<u64>) {
    let emitter = match SignalEmitter::new(&connection, PATH) {
        Ok(emitter) => emitter,
        Err(error) => {
            tracing::warn!("cannot emit JobsChanged: {error}");
            return;
        }
    };
    while changes.changed().await.is_ok() {
        if let Err(error) = Panel::jobs_changed(&emitter).await {
            tracing::warn!("JobsChanged not sent: {error}");
        }
    }
}

/// The user id the bus knows for the connection `name`.
async fn unix_user(connection: &Connection, name: &str) -> zbus::Result<u32> {
    let reply = connection
        .call_method(
            Some(DBUS),
            DBUS_PATH,
            Some(DBUS),
            "GetConnectionUnixUser",
            &(name,),
        )
        .await?;
    reply.body().deserialize()
}

/// Why a call was refused. The name says what went wrong; the message is allotd's text, or for
/// the settings methods a `SettingsRefusal` as JSON.
#[derive(Debug, DBusError)]
#[zbus(prefix = "tech.mehran.Allot1.Error")]
pub enum BusError {
    /// The bus itself failed.
    #[zbus(error)]
    ZBus(zbus::Error),
    /// The caller runs as another user.
    AccessDenied(String),
    /// No such lease.
    NotFound(String),
    /// The lease belongs to another session.
    Forbidden(String),
    /// The lease's state does not allow it, or `config.toml` changed since its revision.
    Conflict(String),
    /// A value allotd rejects, or an argument that is not the JSON the method takes.
    Invalid(String),
    /// Another program holds `config.toml`.
    Held(String),
    /// No machine sample or no config file yet.
    Unavailable(String),
    /// Anything else; the details are in allotd's log.
    Failed(String),
}

impl BusError {
    fn from_status(status: StatusCode, message: String) -> Self {
        match status {
            StatusCode::NOT_FOUND => Self::NotFound(message),
            StatusCode::FORBIDDEN => Self::Forbidden(message),
            StatusCode::CONFLICT => Self::Conflict(message),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => Self::Invalid(message),
            StatusCode::LOCKED => Self::Held(message),
            StatusCode::SERVICE_UNAVAILABLE => Self::Unavailable(message),
            _ => Self::Failed(message),
        }
    }
}

fn not_json(rejection: JsonRejection) -> BusError {
    BusError::Invalid(rejection.body_text())
}

/// A handler's answer as the method's reply: its JSON body, or the error its status names.
async fn answer(response: impl IntoResponse) -> Result<String, BusError> {
    let response = response.into_response();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| BusError::Failed(format!("reading the answer: {error}")))?;
    let text = String::from_utf8_lossy(&body).into_owned();
    if status.is_success() {
        Ok(text)
    } else {
        Err(BusError::from_status(status, text))
    }
}

struct Panel {
    state: AppState,
    own_uid: u32,
    /// Connections already found to run as this user. The bus never gives a name to a second
    /// connection, so a name once checked stays checked.
    callers: Mutex<HashSet<String>>,
}

#[interface(name = "tech.mehran.Allot1")]
impl Panel {
    /// The popup's state, `UiState` as JSON; `Unavailable` before the first sample.
    async fn get_state(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<String, BusError> {
        self.check_caller(&header, connection).await?;
        let service = routes::health_of(&self.state);
        answer(ui::ui_state(&self.state, service).await.map(Json)).await
    }

    /// Ends an active lease and its commands; `KillResponse` as JSON.
    async fn kill(
        &self,
        lease: i64,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<String, BusError> {
        self.check_caller(&header, connection).await?;
        answer(routes::kill(State(self.state.clone()), Path(lease)).await).await
    }

    /// Takes a queued lease out of the queue; `RemoveResponse` as JSON.
    async fn remove(
        &self,
        lease: i64,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<String, BusError> {
        self.check_caller(&header, connection).await?;
        answer(routes::remove(State(self.state.clone()), Path(lease)).await).await
    }

    /// Stops allotd. With `record`, the stop record says the panel stopped it, so no session
    /// starts it again; a restart passes false.
    async fn shutdown(
        &self,
        record: bool,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<(), BusError> {
        self.check_caller(&header, connection).await?;
        let request = record.then_some(ShutdownRequest {
            by: StoppedBy::Panel,
        });
        let stopped = routes::stop(&self.state, request, "D-Bus").await;
        answer(stopped.map(|()| StatusCode::ACCEPTED)).await?;
        Ok(())
    }

    /// Every setting with its spec and value; `SettingsState` as JSON.
    async fn get_settings(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<String, BusError> {
        self.check_caller(&header, connection).await?;
        answer(settings::read(State(self.state.clone())).await).await
    }

    /// Writes a `SettingsPatch` (JSON) together or not at all; the new `SettingsState`.
    async fn change_settings(
        &self,
        patch: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<String, BusError> {
        self.check_caller(&header, connection).await?;
        let patch = Json::<SettingsPatch>::from_bytes(patch.as_bytes()).map_err(not_json)?;
        answer(settings::change(State(self.state.clone()), patch).await).await
    }

    /// What fits with a `PreviewRequest`'s unsaved changes (JSON); `FitPreview` as JSON.
    async fn preview_settings(
        &self,
        request: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<String, BusError> {
        self.check_caller(&header, connection).await?;
        let request = Json::<PreviewRequest>::from_bytes(request.as_bytes()).map_err(not_json)?;
        answer(settings::preview(State(self.state.clone()), request).await).await
    }

    /// How the hook treats a `ClassifyRequest`'s command line (JSON); `CommandTest` as JSON.
    async fn classify(
        &self,
        request: &str,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<String, BusError> {
        self.check_caller(&header, connection).await?;
        let request = Json::<ClassifyRequest>::from_bytes(request.as_bytes()).map_err(not_json)?;
        answer(settings::classify(State(self.state.clone()), request).await).await
    }

    /// allotd's version, which anyone on the bus may read.
    #[zbus(property(emits_changed_signal = "const"))]
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    /// A lease was queued, started, ended or killed.
    #[zbus(signal)]
    async fn jobs_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

impl Panel {
    async fn check_caller(
        &self,
        header: &Header<'_>,
        connection: &Connection,
    ) -> Result<(), BusError> {
        let sender = header
            .sender()
            .ok_or_else(|| BusError::AccessDenied("the call names no sender".to_owned()))?
            .to_string();
        if self.known_caller(&sender) {
            return Ok(());
        }
        let uid = unix_user(connection, &sender).await?;
        if uid != self.own_uid {
            return Err(BusError::AccessDenied(format!(
                "allotd serves only user {}, and the caller is user {uid}",
                self.own_uid
            )));
        }
        self.callers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(sender);
        Ok(())
    }

    fn known_caller(&self, sender: &str) -> bool {
        self.callers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(sender)
    }
}
