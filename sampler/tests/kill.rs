//! Kills real process trees that these tests spawn, and nothing else.

use std::{
    env,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[cfg(unix)]
use std::{
    io,
    os::unix::process::{CommandExt, ExitStatusExt},
};

use allot_core::{
    kill::{KillPlan, KillReport, Protected},
    process::ProcessTable,
};
use allot_sampler::{kill_tree, process_table};
use allot_shared::types::{BoundProcess, Holder, LeaseBinding, SessionKey};

/// Levels left below a helper process; the helper at level 1 starts nothing.
const DEPTH_VAR: &str = "ALLOT_KILL_TEST_DEPTH";

/// What a session helper does: `leader`, `middle` or `leaf`.
#[cfg(unix)]
const ROLE_VAR: &str = "ALLOT_KILL_TEST_ROLE";

/// How long a helper lives when nobody kills it, so a failed test leaves nothing behind.
const HELPER_LIFETIME: Duration = Duration::from_secs(60);

#[test]
#[ignore = "helper process for the kill tests; does nothing without ALLOT_KILL_TEST_DEPTH"]
fn tree_helper() {
    let Ok(depth) = env::var(DEPTH_VAR) else {
        return;
    };
    let depth: u32 = depth.parse().unwrap();
    let _child = (depth > 1).then(|| spawn_level(depth - 1));
    thread::sleep(HELPER_LIFETIME);
}

/// A session leader whose child starts a leaf and exits, so the leaf is re-parented away
/// from the tree and only its session still ties it to the leader.
#[cfg(unix)]
#[test]
#[ignore = "helper process for the kill tests; does nothing without ALLOT_KILL_TEST_ROLE"]
fn session_helper() {
    let Ok(role) = env::var(ROLE_VAR) else {
        return;
    };
    match role.as_str() {
        "leader" => {
            spawn_role("middle").wait().unwrap();
            thread::sleep(HELPER_LIFETIME);
        }
        "middle" => {
            #[expect(
                clippy::zombie_processes,
                reason = "the middle exits at once, so the leaf is re-parented and reaped there"
            )]
            spawn_role("leaf");
        }
        _ => thread::sleep(HELPER_LIFETIME),
    }
}

fn helper(test: &str) -> Command {
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .args([test, "--exact", "--ignored"])
        .stdout(Stdio::null());
    command
}

/// Starts this test binary as a helper with `depth` levels including itself.
fn spawn_level(depth: u32) -> Child {
    helper("tree_helper")
        .env(DEPTH_VAR, depth.to_string())
        .spawn()
        .unwrap()
}

#[cfg(unix)]
fn spawn_role(role: &str) -> Child {
    helper("session_helper")
        .env(ROLE_VAR, role)
        .spawn()
        .unwrap()
}

/// Waits until `root` has a single chain of `depth` levels and returns its PIDs, root first,
/// with the table that shows it.
fn wait_for_chain(root: u32, depth: usize) -> (Vec<u32>, ProcessTable) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let table = process_table();
        let mut chain = vec![root];
        while let Some(child) = table.children(*chain.last().unwrap()).next() {
            chain.push(child.pid);
        }
        if chain.len() == depth {
            return (chain, table);
        }
        assert!(
            Instant::now() < deadline,
            "the tree has {} of {depth} levels",
            chain.len()
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// Waits until every terminated process is gone; on Unix one whose parent died lingers as
/// a zombie until whoever adopted it reaps it.
fn assert_all_gone(report: &KillReport) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let table = process_table();
        let running: Vec<u32> = report
            .terminated
            .iter()
            .filter(|target| table.is_running(target.pid, target.start_time))
            .map(|target| target.pid)
            .collect();
        if running.is_empty() {
            return;
        }
        assert!(Instant::now() < deadline, "still running: {running:?}");
        thread::sleep(Duration::from_millis(50));
    }
}

/// Asserts that `status` is how a process `kill_tree` terminated ends.
fn assert_killed(status: ExitStatus) {
    #[cfg(windows)]
    assert_eq!(status.code(), Some(u32::MAX as i32));
    #[cfg(unix)]
    assert_eq!(status.signal(), Some(libc::SIGTERM));
}

fn now_ms() -> i64 {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    i64::try_from(elapsed.as_millis()).unwrap()
}

/// The plan for killing the tree under `root`, held by this test process.
fn plan(table: &ProcessTable, root: u32, reported_at_ms: i64) -> (KillPlan, Holder) {
    let own = table.get(std::process::id()).unwrap();
    let holder = Holder {
        session: SessionKey("kill-test".into()),
        pid: own.pid,
        pid_start_time: own.start_time,
    };
    let binding = LeaseBinding {
        tool_use_id: "toolu_kill_test".into(),
        bound_at_ms: reported_at_ms,
        root: Some(BoundProcess {
            pid: root,
            start_time: table.get(root).unwrap().start_time,
        }),
    };
    let protected = Protected::new(table, &holder, &[], own.pid);
    (KillPlan::new(&binding, table, protected).unwrap(), holder)
}

#[test]
fn test_kill_tree_terminates_exactly_a_spawned_three_level_tree() {
    let reported_at_ms = now_ms();
    let mut root = spawn_level(3);
    let mut control = spawn_level(1);
    let (chain, table) = wait_for_chain(root.id(), 3);
    let control_start = table.get(control.id()).unwrap().start_time;
    let (mut plan, holder) = plan(&table, root.id(), reported_at_ms);

    // Before terminating anything, the plan must be exactly the spawned helpers.
    let exe_name = Path::new(&env::current_exe().unwrap())
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_lowercase();
    let planned: Vec<u32> = plan.planned().iter().map(|target| target.pid).collect();
    assert_eq!(planned, chain);
    // Linux cuts process names at 15 bytes.
    assert!(
        plan.planned()
            .iter()
            .all(|target| exe_name.starts_with(&target.name.to_lowercase())),
        "{:?}",
        plan.planned()
    );

    let report = kill_tree(&mut plan);
    let terminated: Vec<(u32, u32)> = report
        .terminated
        .iter()
        .map(|target| (target.pid, target.depth))
        .collect();
    assert_eq!(terminated, [(chain[0], 0), (chain[1], 1), (chain[2], 2)]);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert!(report.spared.is_empty(), "{:?}", report.spared);

    assert_killed(root.wait().unwrap());
    assert_all_gone(&report);
    let after = process_table();
    assert!(after.is_running(control.id(), control_start));
    assert!(after.is_running(holder.pid, holder.pid_start_time));

    control.kill().unwrap();
    control.wait().unwrap();
}

#[cfg(unix)]
#[test]
fn test_kill_tree_terminates_an_orphan_of_the_bound_session() {
    let reported_at_ms = now_ms();
    let mut command = helper("session_helper");
    command.env(ROLE_VAR, "leader");
    // SAFETY: setsid is async-signal-safe and touches no memory of this process.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut leader = command.spawn().unwrap();
    let (leaf, table) = wait_for_orphan(leader.id());
    let leaf_start = table.get(leaf).unwrap().start_time;
    let (mut plan, _) = plan(&table, leader.id(), reported_at_ms);
    let planned: Vec<u32> = plan.planned().iter().map(|target| target.pid).collect();
    assert_eq!(planned, [leader.id()]);

    let report = kill_tree(&mut plan);
    let terminated: Vec<(u32, u32)> = report
        .terminated
        .iter()
        .map(|target| (target.pid, target.depth))
        .collect();
    assert_eq!(terminated, [(leader.id(), 0), (leaf, 1)]);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert_eq!(report.terminated[1].start_time, leaf_start);

    assert_killed(leader.wait().unwrap());
    assert_all_gone(&report);
}

/// Waits until the session `leader` leads holds a process that is not below the leader any
/// more, and returns its PID with the table that shows it.
#[cfg(unix)]
fn wait_for_orphan(leader: u32) -> (u32, ProcessTable) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let table = process_table();
        let orphan = table.iter().find(|node| {
            let session = libc::pid_t::try_from(node.pid).map(|pid| {
                // SAFETY: plain FFI call with a PID.
                unsafe { libc::getsid(pid) }
            });
            node.pid != leader
                && node.parent_pid != Some(leader)
                && session.ok() == libc::pid_t::try_from(leader).ok()
        });
        if let Some(orphan) = orphan {
            return (orphan.pid, table);
        }
        assert!(Instant::now() < deadline, "no orphan in session {leader}");
        thread::sleep(Duration::from_millis(50));
    }
}
