//! `worktree.remove`: remove a linked checkout in the background, then close its workspace.

use super::{
    super::responses::{api_error, workspace_not_found},
    info::worktree_info_for_membership,
    operation_in_progress,
    operations::{Operation, Pending, RemoveOperation, Responder},
};
use crate::{
    app::App,
    git::{
        discovery::canonicalize_best_effort_path,
        worktree::{self, WorktreeError},
    },
};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, ResponseResult, WorktreeRemoveParams},
};
use maestro_workspace::terminal_id::TerminalId;
use std::path::PathBuf;

/// Whether the processes of a workspace stop before its checkout is removed: always when the
/// removal is forced, and on Windows, where a running process locks its directory.
fn stops_processes_first(force: bool) -> bool {
    force || cfg!(windows)
}

/// A remove request that passed validation, with what it needs once Git is done.
struct RemovePlan {
    /// The request as the loop will hold it while Git works.
    operation: RemoveOperation,
    /// Repository the Git command runs in.
    repo_root: PathBuf,
    /// Terminals of the workspace, stopped first when the policy asks.
    terminals: Vec<TerminalId>,
}

impl<C: FnMut() -> u128> App<C> {
    /// Validates a remove request and starts `git worktree remove` in the background; the
    /// answer reaches `respond_to` once Git is done, or at once when the request is refused.
    pub(in crate::app) async fn start_worktree_remove(
        &mut self,
        params: WorktreeRemoveParams,
        respond_to: Responder,
    ) {
        match self.plan_worktree_remove(&params) {
            Ok(plan) => self.spawn_worktree_remove(plan, respond_to).await,
            Err(error) => {
                let _ = respond_to.send(Err(error));
            }
        }
    }

    /// Checks that the request names a linked checkout nothing else is working on and takes
    /// the snapshots its events will carry.
    fn plan_worktree_remove(&self, params: &WorktreeRemoveParams) -> Result<RemovePlan, ErrorBody> {
        let index = self.target_workspace(&params.workspace_id)?;
        let workspace = &self.state.topology.workspaces[index];
        let membership = workspace.worktree_space().ok_or_else(|| {
            api_error(
                "not_linked_worktree",
                "workspace is not a Maestro-managed worktree checkout",
            )
        })?;
        if !membership.is_linked_worktree {
            return Err(api_error(
                "not_linked_worktree",
                "workspace is not a linked worktree checkout",
            ));
        }
        if cfg!(windows)
            && !params.force
            && worktree::checkout_has_dirty_files(&membership.checkout_path).unwrap_or(false)
        {
            return Err(api_error(
                "dirty_worktree_requires_force",
                WorktreeError::dirty_checkout(&membership.checkout_path).to_string(),
            ));
        }
        let checkout_key = canonicalize_best_effort_path(&membership.checkout_path);
        if self
            .worktree_operations
            .is_busy(&checkout_key, Some(&workspace.id))
        {
            return Err(operation_in_progress());
        }
        let snapshot = self
            .workspace_info(index)
            .ok_or_else(|| workspace_not_found(&params.workspace_id))?;
        Ok(RemovePlan {
            repo_root: membership.repo_root.clone(),
            terminals: self.state.workspace_terminals(&[index]),
            operation: RemoveOperation {
                workspace_id: workspace.id.clone(),
                checkout_path: membership.checkout_path.clone(),
                checkout_key,
                workspace: snapshot,
                worktree: worktree_info_for_membership(membership, None),
                forced: params.force,
            },
        })
    }

    /// Stops the workspace's processes when the policy asks, registers the request and runs
    /// `git worktree remove` away from the loop.
    async fn spawn_worktree_remove(&mut self, plan: RemovePlan, respond_to: Responder) {
        let RemovePlan {
            operation,
            repo_root,
            terminals,
        } = plan;
        if stops_processes_first(operation.forced) {
            for terminal in &terminals {
                self.shutdown_terminal_runtime(terminal).await;
            }
        }
        let (path, force) = (operation.checkout_path.clone(), operation.forced);
        let id = self.worktree_operations.start(Pending {
            respond_to,
            operation: Operation::Remove(operation),
        });
        self.run_worktree_git(id, move || worktree::remove(&repo_root, &path, force));
    }

    /// Closes the workspace of a removed checkout and builds the answer of the request.
    ///
    /// # Errors
    /// Returns Git's failure; a dirty checkout needs the request repeated with force.
    pub(super) async fn finish_worktree_remove(
        &mut self,
        remove: RemoveOperation,
        result: Result<(), WorktreeError>,
    ) -> Result<ResponseResult, ErrorBody> {
        if let Err(error) = result {
            let code = if !remove.forced && error.is_dirty_checkout() {
                "dirty_worktree_requires_force"
            } else {
                "worktree_remove_failed"
            };
            return Err(api_error(code, error.to_string()));
        }
        let still_linked = self
            .state
            .topology
            .workspaces
            .iter()
            .position(|workspace| workspace.id == remove.workspace_id)
            .filter(|index| {
                self.state.topology.workspaces[*index]
                    .worktree_space()
                    .is_some_and(|space| {
                        space.is_linked_worktree && space.checkout_path == remove.checkout_path
                    })
            });
        if let Some(index) = still_linked {
            self.close_removed_linked_worktree_workspace(index);
            self.shutdown_detached_terminal_runtimes().await;
            self.event_hub.push(EventData::WorkspaceClosed {
                workspace_id: remove.workspace_id.clone(),
                workspace: Some(remove.workspace.clone()),
            });
        }
        self.event_hub.push(EventData::WorktreeRemoved {
            workspace_id: remove.workspace_id.clone(),
            workspace: Some(remove.workspace),
            worktree: remove.worktree,
            forced: remove.forced,
        });
        Ok(ResponseResult::WorktreeRemoved {
            workspace_id: remove.workspace_id,
            path: remove.checkout_path.display().to_string(),
            forced: remove.forced,
        })
    }

    /// Closes a workspace whose checkout is gone and focuses the repository's own workspace.
    fn close_removed_linked_worktree_workspace(&mut self, index: usize) {
        let parent_key = self
            .state
            .topology
            .workspaces
            .get(index)
            .and_then(|workspace| workspace.worktree_space())
            .filter(|space| space.is_linked_worktree)
            .map(|space| space.key.clone());
        self.state.selected = index;
        self.state.close_selected_workspace();
        let parent = parent_key.and_then(|key| {
            self.state.topology.workspaces.iter().position(|workspace| {
                workspace
                    .worktree_space()
                    .is_some_and(|space| !space.is_linked_worktree && space.key == key)
            })
        });
        if let Some(parent) = parent {
            self.state.switch_workspace(parent);
        }
    }
}
