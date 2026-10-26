//! Killing a planned process tree is not supported on Unix yet: every target is reported as
//! failed with that reason, so `allot kill` shows which processes are left running.

use allot_core::kill::{KillFailure, KillPlan, KillReport};

use crate::table::process_table;

const UNSUPPORTED: &str = "killing a process tree is not supported on this OS yet";

/// Reports every process `plan` covers as failed, without signalling any of them.
pub fn kill_tree(plan: &mut KillPlan) -> KillReport {
    let targets = plan.sweep(&process_table());
    KillReport {
        failed: targets
            .into_iter()
            .map(|target| KillFailure {
                target,
                error: UNSUPPORTED.into(),
            })
            .collect(),
        spared: plan.spared().to_vec(),
        ..KillReport::default()
    }
}
