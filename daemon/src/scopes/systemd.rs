//! [`SystemdBus`] on the session bus, which the systemd user manager serves.

use zbus::{
    Connection,
    wire::{OwnedObjectPath, Variant},
};

use crate::scopes::bus::{BusError, BusFuture, ScopeCaps, SystemdBus};

const SYSTEMD: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const SCOPE: &str = "org.freedesktop.systemd1.Scope";
/// `KillUnit` takes the signal number as a plain integer.
const SIGKILL: i32 = 9;

/// One row of `ListUnitsByPatterns`: name, description, load, active and sub state,
/// follower, unit path, job ID, job type, and job path.
type UnitRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    OwnedObjectPath,
    u32,
    String,
    OwnedObjectPath,
);

/// The user manager, over a connection held for the daemon's lifetime.
#[derive(Debug)]
pub(crate) struct UserManager {
    connection: Connection,
}

impl UserManager {
    /// Connects to the session bus. Must run on the Tokio runtime.
    ///
    /// # Errors
    ///
    /// Fails when there is no session bus, as without a systemd user session.
    pub(crate) async fn connect() -> Result<Self, BusError> {
        let connection = Connection::session()
            .await
            .map_err(|error| failure("connecting to the session bus", &error))?;
        Ok(Self { connection })
    }

    async fn unit_path(&self, scope: &str) -> Result<OwnedObjectPath, BusError> {
        let reply = self
            .connection
            .call_method(
                Some(SYSTEMD),
                MANAGER_PATH,
                Some(MANAGER),
                "GetUnit",
                &(scope,),
            )
            .await
            .map_err(|error| failure("GetUnit", &error))?;
        reply
            .body()
            .deserialize()
            .map_err(|error| failure("GetUnit", &error))
    }

    async fn units(&self, pattern: &str) -> Result<Vec<String>, BusError> {
        let states: Vec<&str> = Vec::new();
        let reply = self
            .connection
            .call_method(
                Some(SYSTEMD),
                MANAGER_PATH,
                Some(MANAGER),
                "ListUnitsByPatterns",
                &(states, vec![pattern]),
            )
            .await
            .map_err(|error| failure("ListUnitsByPatterns", &error))?;
        let rows: Vec<UnitRow> = reply
            .body()
            .deserialize()
            .map_err(|error| failure("ListUnitsByPatterns", &error))?;
        Ok(rows.into_iter().map(|row| row.0).collect())
    }
}

impl SystemdBus for UserManager {
    fn start_scope<'a>(
        &'a self,
        scope: &'a str,
        anchor: u32,
        caps: ScopeCaps,
    ) -> BusFuture<'a, ()> {
        Box::pin(async move {
            let mut properties: Vec<(&str, Variant<'_>)> = vec![
                ("PIDs", Variant::from(vec![anchor])),
                ("Delegate", Variant::from(true)),
                ("Description", Variant::from("Allot lease")),
            ];
            if let Some(usec) = caps.cpu_usec_per_sec {
                properties.push(("CPUQuotaPerSecUSec", Variant::from(usec)));
            }
            if let Some(bytes) = caps.memory_high_bytes {
                properties.push(("MemoryHigh", Variant::from(bytes)));
            }
            let auxiliary: Vec<(&str, Vec<(&str, Variant<'_>)>)> = Vec::new();
            self.connection
                .call_method(
                    Some(SYSTEMD),
                    MANAGER_PATH,
                    Some(MANAGER),
                    "StartTransientUnit",
                    &(scope, "fail", properties, auxiliary),
                )
                .await
                .map_err(|error| failure("StartTransientUnit", &error))?;
            Ok(())
        })
    }

    fn attach<'a>(&'a self, scope: &'a str, pids: &'a [u32]) -> BusFuture<'a, ()> {
        Box::pin(async move {
            self.connection
                .call_method(
                    Some(SYSTEMD),
                    MANAGER_PATH,
                    Some(MANAGER),
                    "AttachProcessesToUnit",
                    &(scope, "", pids.to_vec()),
                )
                .await
                .map_err(|error| failure("AttachProcessesToUnit", &error))?;
            Ok(())
        })
    }

    fn kill<'a>(&'a self, scope: &'a str) -> BusFuture<'a, ()> {
        Box::pin(async move {
            self.connection
                .call_method(
                    Some(SYSTEMD),
                    MANAGER_PATH,
                    Some(MANAGER),
                    "KillUnit",
                    &(scope, "all", SIGKILL),
                )
                .await
                .map_err(|error| failure("KillUnit", &error))?;
            Ok(())
        })
    }

    fn control_group<'a>(&'a self, scope: &'a str) -> BusFuture<'a, Option<String>> {
        Box::pin(async move {
            // GetUnit fails for a unit that is not loaded; listing first tells that apart
            // from a failing bus without matching on error names.
            if self.units(scope).await?.is_empty() {
                return Ok(None);
            }
            let path = self.unit_path(scope).await?;
            let reply = self
                .connection
                .call_method(
                    Some(SYSTEMD),
                    path.as_str(),
                    Some(PROPERTIES),
                    "Get",
                    &(SCOPE, "ControlGroup"),
                )
                .await
                .map_err(|error| failure("Get ControlGroup", &error))?;
            let body = reply.body();
            let value: Variant<'_> = body
                .deserialize()
                .map_err(|error| failure("Get ControlGroup", &error))?;
            let cgroup =
                String::try_from(value).map_err(|error| failure("Get ControlGroup", &error))?;
            Ok(Some(cgroup).filter(|cgroup| !cgroup.is_empty()))
        })
    }

    fn list_units<'a>(&'a self, pattern: &'a str) -> BusFuture<'a, Vec<String>> {
        Box::pin(self.units(pattern))
    }
}

fn failure(method: &'static str, error: &impl std::fmt::Display) -> BusError {
    BusError {
        method,
        message: error.to_string(),
    }
}
