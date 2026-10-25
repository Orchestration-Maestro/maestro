//! Live foreground jobs and native process observations.

/// Source of a terminal's foreground process group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessDetectionMode {
    /// Use native terminal foreground information only.
    Native,
    /// Fall back to direct child groups when native information is absent.
    ChildGroups,
}

/// Selects native detection unless the child-group fallback is explicit.
///
/// # Errors
/// Returns the original unknown operand.
pub fn parse_process_detection_mode(value: Option<&str>) -> Result<ProcessDetectionMode, &str> {
    match value {
        None | Some("" | "native") => Ok(ProcessDetectionMode::Native),
        Some("child-groups") => Ok(ProcessDetectionMode::ChildGroups),
        Some(value) => Err(value),
    }
}

/// A live group member before argv observations are attached.
#[derive(Debug, PartialEq, Eq)]
struct ProcGroupMember {
    /// Native process identity.
    pid: u32,
    /// Process name from its stat record.
    comm: String,
}

/// Collects each reachable positive PID once across roots and thread children.
fn process_tree_pids(
    roots: impl IntoIterator<Item = u32>,
    mut task_ids: impl FnMut(u32) -> Vec<u32>,
    mut task_children: impl FnMut(u32, u32) -> Vec<u32>,
) -> Vec<u32> {
    use std::collections::{HashSet, VecDeque};
    let mut visited = HashSet::new();
    let mut pending = VecDeque::new();
    for pid in roots {
        if pid > 0 && visited.insert(pid) {
            pending.push_back(pid);
        }
    }
    let mut pids = Vec::new();
    while let Some(pid) = pending.pop_front() {
        pids.push(pid);
        for child in task_ids(pid)
            .into_iter()
            .flat_map(|tid| task_children(pid, tid))
        {
            if child > 0 && visited.insert(child) {
                pending.push_back(child);
            }
        }
    }
    pids
}

/// Filters one fresh tree walk to live members of the selected group.
fn foreground_process_group_members_with(
    child_pid: u32,
    group: u32,
    task_ids: impl FnMut(u32) -> Vec<u32>,
    task_children: impl FnMut(u32, u32) -> Vec<u32>,
    mut live_member: impl FnMut(u32, u32) -> Option<ProcGroupMember>,
) -> Option<Vec<ProcGroupMember>> {
    let mut members: Vec<_> = process_tree_pids([child_pid, group], task_ids, task_children)
        .into_iter()
        .filter_map(|pid| live_member(group, pid))
        .collect();
    members.sort_unstable_by_key(|member| member.pid);
    (!members.is_empty()).then_some(members)
}

/// Maximum direct-child records inspected by the explicit fallback.
const CHILD_GROUPS_SCAN_LIMIT: usize = 64;

/// Chooses the largest positive non-shell group, failing closed on excess records.
fn child_groups_foreground_process_group_with(
    child_pid: u32,
    shell_group: u32,
    mut task_ids: impl FnMut(u32) -> Vec<u32>,
    mut task_children: impl FnMut(u32, u32) -> Vec<u32>,
    mut group_id: impl FnMut(u32) -> Option<i32>,
) -> Option<u32> {
    let mut scanned = 0;
    let mut job = None;
    for tid in task_ids(child_pid) {
        for child in task_children(child_pid, tid) {
            if scanned == CHILD_GROUPS_SCAN_LIMIT {
                return None;
            }
            scanned += 1;
            if let Some(group) = group_id(child)
                .and_then(|group| u32::try_from(group).ok())
                .filter(|group| *group > 0 && *group != shell_group)
            {
                job = Some(job.map_or(group, |current: u32| current.max(group)));
            }
        }
    }
    job.or(Some(shell_group))
}

mod linux;
pub use linux::{
    foreground_group_leader_job, foreground_job, foreground_process_group_id,
    foreground_process_group_id_for_tty_fd, process_cwd, process_environment_value,
    session_processes, session_processes_by_id,
};

pub use crate::foreground::{ForegroundJob, ForegroundProcess};

#[cfg(test)]
mod tests;
