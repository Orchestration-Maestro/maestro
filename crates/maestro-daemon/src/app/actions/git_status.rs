//! Repository status observations applied to retained workspaces.

use crate::{
    app::state::DaemonState, terminal::runtime_registry::TerminalRuntimeRegistry,
    workspace::resolved_identity_cwd_from,
};
use maestro_workspace::workspace::{Workspace, WorkspaceGitStatus};

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Apply observations to the workspaces they still describe and report visible changes.
    ///
    /// An observation applies only while its workspace still follows the directory it was
    /// requested for. Fields that were not demanded keep their admitted values, and worktree
    /// membership is never changed.
    pub fn apply_workspace_git_statuses(
        &mut self,
        runtimes: &TerminalRuntimeRegistry,
        results: Vec<WorkspaceGitStatus>,
    ) -> bool {
        let mut changed = false;
        for result in results {
            let Some(workspace) = self
                .topology
                .workspaces
                .iter_mut()
                .find(|workspace| workspace.id == result.workspace_id)
            else {
                continue;
            };
            let current = resolved_identity_cwd_from(workspace, &self.terminals, runtimes);
            if current.as_ref() == Some(&result.resolved_identity_cwd) {
                changed |= apply_status(workspace, result);
            }
        }
        changed
    }
}

/// Admit one observation into its workspace and report whether a visible value changed.
fn apply_status(workspace: &mut Workspace, result: WorkspaceGitStatus) -> bool {
    let mut changed = false;
    workspace.cached_identity_cwd = result.resolved_identity_cwd;
    workspace.cached_git_status_key = result.status_cache_key;
    if workspace.cached_auto_label != result.auto_label {
        workspace.cached_auto_label = result.auto_label;
        changed |= workspace.custom_name.is_none();
    }
    if result.demand.branch && workspace.cached_git_branch != result.branch {
        workspace.cached_git_branch = result.branch;
        changed = true;
    }
    if result.demand.ahead_behind && workspace.cached_git_ahead_behind != result.ahead_behind {
        workspace.cached_git_ahead_behind = result.ahead_behind;
        changed = true;
    }
    if workspace.cached_git_space != result.space {
        workspace.cached_git_space = result.space;
        changed = true;
    }
    changed
}
