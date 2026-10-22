use std::collections::HashSet;

/// One process as seen by the ancestry walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessNode {
    /// Process ID.
    pub pid: u32,
    /// Parent process ID as recorded by the OS, if any.
    pub parent_pid: Option<u32>,
    /// Executable name, for example `claude.exe`.
    pub name: String,
    /// Full executable path when the OS allows reading it.
    pub exe: Option<String>,
    /// Start time in seconds since the Unix epoch.
    pub start_time: u64,
}

/// Returns the process `start` followed by its ancestors, nearest first.
///
/// The walk stops at the first parent that cannot be looked up, at a cycle, or at a parent
/// that started after its child. Windows reuses PIDs and never updates a child's parent PID,
/// so a "parent" younger than its child is an unrelated process that inherited a dead
/// parent's PID.
pub fn ancestry<F>(start: u32, lookup: F) -> Vec<ProcessNode>
where
    F: Fn(u32) -> Option<ProcessNode>,
{
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut next = lookup(start);
    while let Some(node) = next {
        if !seen.insert(node.pid) {
            break;
        }
        next = node
            .parent_pid
            .and_then(&lookup)
            .filter(|parent| parent.start_time <= node.start_time);
        chain.push(node);
    }
    chain
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn node(pid: u32, parent_pid: Option<u32>, start_time: u64) -> ProcessNode {
        ProcessNode {
            pid,
            parent_pid,
            name: format!("p{pid}"),
            exe: None,
            start_time,
        }
    }

    fn walk(start: u32, nodes: &[ProcessNode]) -> Vec<u32> {
        let table: HashMap<u32, ProcessNode> = nodes.iter().map(|n| (n.pid, n.clone())).collect();
        ancestry(start, |pid| table.get(&pid).cloned())
            .iter()
            .map(|n| n.pid)
            .collect()
    }

    #[test]
    fn test_ancestry_linear_chain_returns_nearest_first() {
        let nodes = [
            node(1, None, 10),
            node(2, Some(1), 20),
            node(3, Some(2), 30),
        ];
        assert_eq!(walk(3, &nodes), vec![3, 2, 1]);
    }

    #[test]
    fn test_ancestry_missing_parent_stops_walk() {
        let nodes = [node(2, Some(1), 20), node(3, Some(2), 30)];
        assert_eq!(walk(3, &nodes), vec![3, 2]);
    }

    #[test]
    fn test_ancestry_cycle_terminates() {
        let nodes = [node(1, Some(2), 10), node(2, Some(1), 10)];
        assert_eq!(walk(1, &nodes), vec![1, 2]);
    }

    #[test]
    fn test_ancestry_reused_parent_pid_is_rejected() {
        let nodes = [node(1, None, 50), node(3, Some(1), 30)];
        assert_eq!(walk(3, &nodes), vec![3]);
    }

    #[test]
    fn test_ancestry_parent_started_same_second_is_accepted() {
        let nodes = [node(1, None, 30), node(3, Some(1), 30)];
        assert_eq!(walk(3, &nodes), vec![3, 1]);
    }

    #[test]
    fn test_ancestry_unknown_start_returns_empty() {
        assert!(walk(9, &[]).is_empty());
    }
}
