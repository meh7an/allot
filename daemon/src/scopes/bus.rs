//! The systemd user manager calls a lease scope needs, behind a trait so tests can fake the
//! bus.

use std::{fmt, future::Future, pin::Pin};

/// The result of a call on the user manager.
pub(crate) type BusFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, BusError>> + Send + 'a>>;

/// A user manager call that failed, with what the manager or the bus said.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{method} failed: {message}")]
pub(crate) struct BusError {
    /// The manager method called.
    pub(crate) method: &'static str,
    /// The D-Bus error name and message, or why the call never reached the manager.
    pub(crate) message: String,
}

/// Caps a new scope starts with; `None` leaves that resource uncapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ScopeCaps {
    /// `CPUQuotaPerSecUSec`: CPU time per second of wall time.
    pub(crate) cpu_usec_per_sec: Option<u64>,
    /// `MemoryHigh`: memory above which the kernel throttles and reclaims.
    pub(crate) memory_high_bytes: Option<u64>,
}

/// The `org.freedesktop.systemd1.Manager` methods of the user manager that lease scopes use.
pub(crate) trait SystemdBus: Send + Sync + fmt::Debug {
    /// Starts the transient scope `scope` holding `anchor`, with `Delegate=yes` so processes
    /// of the same user can be moved into it later.
    fn start_scope<'a>(&'a self, scope: &'a str, anchor: u32, caps: ScopeCaps)
    -> BusFuture<'a, ()>;

    /// Moves `pids` into the running scope `scope`. Their children stay where they are.
    fn attach<'a>(&'a self, scope: &'a str, pids: &'a [u32]) -> BusFuture<'a, ()>;

    /// Sends SIGKILL to every process of `scope`.
    fn kill<'a>(&'a self, scope: &'a str) -> BusFuture<'a, ()>;

    /// The cgroup path of the loaded unit `scope`; `None` when no such unit is loaded.
    fn control_group<'a>(&'a self, scope: &'a str) -> BusFuture<'a, Option<String>>;

    /// The loaded units whose names match the glob `pattern`.
    fn list_units<'a>(&'a self, pattern: &'a str) -> BusFuture<'a, Vec<String>>;
}
