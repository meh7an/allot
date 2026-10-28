//! A fake user manager and machine for the scope tests: processes with parents, sessions and
//! cgroups, which the fake bus moves and kills as systemd would.

use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use allot_shared::{platform::DAEMON_EXE, types::EnforcedLimits};

use crate::scopes::{
    ANCHOR_FLAG,
    bus::{BusError, BusFuture, ScopeCaps, SystemdBus},
    host::{CgroupReading, Host, ProcInfo},
};

/// Where the fake's scopes live.
pub(crate) const APP_SLICE: &str = "/user.slice/user-1000.slice/user@1000.service/app.slice";

/// Where processes start: an SSH or terminal session outside the user manager.
pub(crate) const SESSION_SCOPE: &str = "/user.slice/user-1000.slice/session-2.scope";

#[derive(Debug, Clone)]
pub(crate) struct FakeProc {
    pub(crate) parent: u32,
    pub(crate) comm: String,
    pub(crate) session: u32,
    pub(crate) cgroup: String,
    pub(crate) cmdline: Vec<String>,
}

/// A call the fake bus received.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Call {
    Start {
        scope: String,
        anchor: u32,
        caps: ScopeCaps,
    },
    Attach {
        scope: String,
        pids: Vec<u32>,
    },
    Kill(String),
}

#[derive(Debug, Default)]
pub(crate) struct Machine {
    pub(crate) procs: BTreeMap<u32, FakeProc>,
    pub(crate) usage: BTreeMap<String, CgroupReading>,
    pub(crate) limits: BTreeMap<String, EnforcedLimits>,
    pub(crate) next_pid: u32,
    /// Children the next attach call forks outside the scope, as a racing build would.
    pub(crate) fork_on_attach: Vec<(u32, u32)>,
    /// Processes that exit just before the next attach call reaches the manager.
    pub(crate) exit_on_attach: Vec<u32>,
    pub(crate) calls: Vec<Call>,
    pub(crate) fail_start: bool,
    /// Units another run left loaded.
    pub(crate) units: Vec<String>,
}

/// The fake machine; also the fake bus, since systemd acts on the same processes.
#[derive(Debug, Default, Clone)]
pub(crate) struct Fake(Arc<Mutex<Machine>>);

impl Fake {
    pub(crate) fn new() -> Self {
        let fake = Self::default();
        fake.machine().next_pid = 5_000;
        fake
    }

    pub(crate) fn machine(&self) -> MutexGuard<'_, Machine> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds a process in the session scope.
    pub(crate) fn add(&self, pid: u32, parent: u32, comm: &str, session: u32) {
        self.machine().procs.insert(
            pid,
            FakeProc {
                parent,
                comm: comm.to_owned(),
                session,
                cgroup: SESSION_SCOPE.to_owned(),
                cmdline: vec![comm.to_owned()],
            },
        );
    }

    pub(crate) fn cgroup(&self, pid: u32) -> Option<String> {
        self.machine()
            .procs
            .get(&pid)
            .map(|proc| proc.cgroup.clone())
    }

    pub(crate) fn calls(&self) -> Vec<Call> {
        self.machine().calls.clone()
    }

    pub(crate) fn alive(&self, pid: u32) -> bool {
        self.machine().procs.contains_key(&pid)
    }
}

fn scope_cgroup(scope: &str) -> String {
    format!("{APP_SLICE}/{scope}")
}

impl SystemdBus for Fake {
    fn start_scope<'a>(
        &'a self,
        scope: &'a str,
        anchor: u32,
        caps: ScopeCaps,
    ) -> BusFuture<'a, ()> {
        Box::pin(async move {
            let mut machine = self.machine();
            machine.calls.push(Call::Start {
                scope: scope.to_owned(),
                anchor,
                caps,
            });
            if machine.fail_start {
                return Err(BusError {
                    method: "StartTransientUnit",
                    message: "org.freedesktop.DBus.Error.AccessDenied".into(),
                });
            }
            if let Some(proc) = machine.procs.get_mut(&anchor) {
                proc.cgroup = scope_cgroup(scope);
            }
            Ok(())
        })
    }

    fn attach<'a>(&'a self, scope: &'a str, pids: &'a [u32]) -> BusFuture<'a, ()> {
        Box::pin(async move {
            let mut machine = self.machine();
            machine.calls.push(Call::Attach {
                scope: scope.to_owned(),
                pids: pids.to_vec(),
            });
            for pid in std::mem::take(&mut machine.exit_on_attach) {
                machine.procs.remove(&pid);
            }
            if pids.iter().any(|pid| !machine.procs.contains_key(pid)) {
                return Err(BusError {
                    method: "AttachProcessesToUnit",
                    message: "org.freedesktop.DBus.Error.UnixProcessIdUnknown".into(),
                });
            }
            for pid in pids {
                if let Some(proc) = machine.procs.get_mut(pid) {
                    proc.cgroup = scope_cgroup(scope);
                }
            }
            for (pid, parent) in std::mem::take(&mut machine.fork_on_attach) {
                let mut child = machine.procs[&parent].clone();
                child.parent = parent;
                child.cgroup = SESSION_SCOPE.to_owned();
                machine.procs.insert(pid, child);
            }
            Ok(())
        })
    }

    fn kill<'a>(&'a self, scope: &'a str) -> BusFuture<'a, ()> {
        Box::pin(async move {
            let mut machine = self.machine();
            machine.calls.push(Call::Kill(scope.to_owned()));
            let cgroup = scope_cgroup(scope);
            machine.procs.retain(|_, proc| proc.cgroup != cgroup);
            Ok(())
        })
    }

    fn control_group<'a>(&'a self, scope: &'a str) -> BusFuture<'a, Option<String>> {
        Box::pin(async move {
            let machine = self.machine();
            Ok(machine
                .units
                .iter()
                .any(|unit| unit == scope)
                .then(|| scope_cgroup(scope)))
        })
    }

    fn list_units<'a>(&'a self, pattern: &'a str) -> BusFuture<'a, Vec<String>> {
        Box::pin(async move {
            let (prefix, suffix) = pattern.split_once('*').unwrap_or((pattern, ""));
            let machine = self.machine();
            Ok(machine
                .units
                .iter()
                .filter(|unit| unit.starts_with(prefix) && unit.ends_with(suffix))
                .cloned()
                .collect())
        })
    }
}

impl Host for Fake {
    fn children(&self, pid: u32) -> Vec<u32> {
        let machine = self.machine();
        machine
            .procs
            .iter()
            .filter(|(_, proc)| proc.parent == pid)
            .map(|(child, _)| *child)
            .collect()
    }

    fn process(&self, pid: u32) -> Option<ProcInfo> {
        self.machine().procs.get(&pid).map(|proc| ProcInfo {
            comm: proc.comm.clone(),
            session: proc.session,
        })
    }

    fn cgroup_of(&self, pid: u32) -> Option<String> {
        self.cgroup(pid)
    }

    fn usage(&self, cgroup: &str) -> io::Result<CgroupReading> {
        self.machine()
            .usage
            .get(cgroup)
            .copied()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn limits(&self, cgroup: &str) -> EnforcedLimits {
        self.machine()
            .limits
            .get(cgroup)
            .copied()
            .unwrap_or_default()
    }

    fn procs(&self, cgroup: &str) -> Vec<u32> {
        let machine = self.machine();
        machine
            .procs
            .iter()
            .filter(|(_, proc)| proc.cgroup == cgroup)
            .map(|(pid, _)| *pid)
            .collect()
    }

    fn cmdline(&self, pid: u32) -> Option<Vec<String>> {
        self.machine()
            .procs
            .get(&pid)
            .map(|proc| proc.cmdline.clone())
    }

    fn kill_cgroup(&self, cgroup: &str) -> io::Result<()> {
        self.machine().procs.retain(|_, proc| proc.cgroup != cgroup);
        Ok(())
    }

    fn kill_process(&self, pid: u32) -> io::Result<()> {
        self.machine()
            .procs
            .remove(&pid)
            .map(|_| ())
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn spawn_anchor(&self, scope: &str) -> io::Result<u32> {
        let mut machine = self.machine();
        machine.next_pid += 1;
        let pid = machine.next_pid;
        machine.procs.insert(
            pid,
            FakeProc {
                parent: 1,
                comm: DAEMON_EXE.into(),
                session: pid,
                cgroup: format!("{APP_SLICE}/allotd.service"),
                cmdline: vec![DAEMON_EXE.into(), ANCHOR_FLAG.into(), scope.to_owned()],
            },
        );
        Ok(pid)
    }
}
