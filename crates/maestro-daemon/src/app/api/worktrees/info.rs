//! Finding workspaces by checkout, recording worktree membership and describing checkouts.

use super::{super::responses::api_error, source::WorktreeSource};
use crate::{
    app::App,
    git::{
        discovery::{
            GitSpaceMetadata, canonicalize_best_effort_path, git_branch, git_space_metadata,
        },
        worktree::ExistingWorktree,
    },
};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, PaneInfo, TabInfo, WorkspaceInfo, WorktreeInfo, WorktreeSourceInfo},
};
use maestro_workspace::workspace::{Workspace, WorktreeSpaceMembership};
use std::path::{Path, PathBuf};

/// The repository identity a workspace shows: the admitted one, else what its directory holds.
pub(super) fn git_space_of(workspace: &Workspace, cwd: Option<&Path>) -> Option<GitSpaceMetadata> {
    workspace
        .git_space()
        .cloned()
        .or_else(|| cwd.and_then(git_space_metadata))
}

/// Describes the checkout a membership records.
pub(super) fn worktree_info_for_membership(
    membership: &WorktreeSpaceMembership,
    open_workspace_id: Option<String>,
) -> WorktreeInfo {
    let branch = git_branch(&membership.checkout_path);
    WorktreeInfo {
        path: membership.checkout_path.display().to_string(),
        is_detached: branch.is_none(),
        branch,
        is_bare: false,
        is_prunable: false,
        is_linked_worktree: membership.is_linked_worktree.into(),
        open_workspace_id,
        label: membership.label.clone(),
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// The directory a workspace's first root process is in, else its retained one.
    pub(super) fn workspace_cwd(&self, workspace: &Workspace) -> Option<PathBuf> {
        crate::workspace::resolved_identity_cwd_from(
            workspace,
            &self.state.terminals,
            &self.terminal_runtimes,
        )
    }

    /// The stable identity of the workspace at a display position, when there is one.
    pub(super) fn workspace_id_at(&self, index: Option<usize>) -> Option<String> {
        let workspace = self.state.topology.workspaces.get(index?)?;
        Some(workspace.id.clone())
    }

    /// The workspace showing the repository's own checkout, by group key.
    pub(super) fn find_parent_workspace_by_key(&self, repo_key: &str) -> Option<usize> {
        self.state.topology.workspaces.iter().position(|workspace| {
            workspace
                .worktree_space()
                .is_some_and(|space| space.key == repo_key && !space.is_linked_worktree)
                || workspace
                    .git_space()
                    .is_some_and(|space| space.key == repo_key && !space.is_linked_worktree)
        })
    }

    /// The workspace showing a repository's own checkout, by group key or by directory.
    pub(super) fn find_parent_workspace_for_space(
        &self,
        space: &GitSpaceMetadata,
    ) -> Option<usize> {
        self.find_parent_workspace_by_key(&space.key)
            .or_else(|| self.open_workspace_idx_for_checkout(&space.repo_root))
    }

    /// The workspace that has `checkout_path` open: by membership, by repository identity or
    /// by the directory its root process is in.
    pub(super) fn open_workspace_idx_for_checkout(&self, checkout_path: &Path) -> Option<usize> {
        let canonical = canonicalize_best_effort_path(checkout_path);
        let checkout_key = canonical.display().to_string();
        self.state.topology.workspaces.iter().position(|workspace| {
            if workspace.worktree_space().is_some_and(|space| {
                canonicalize_best_effort_path(&space.checkout_path) == canonical
            }) {
                return true;
            }
            let cwd = self.workspace_cwd(workspace);
            git_space_of(workspace, cwd.as_deref())
                .is_some_and(|space| space.checkout_key == checkout_key)
                || cwd
                    .as_deref()
                    .is_some_and(|cwd| canonicalize_best_effort_path(cwd) == canonical)
        })
    }

    /// Records a workspace's membership, announcing the change when asked and it is new.
    pub(super) fn set_worktree_membership(
        &mut self,
        index: usize,
        membership: WorktreeSpaceMembership,
        announce: bool,
    ) {
        let Some(workspace) = self.state.topology.workspaces.get_mut(index) else {
            return;
        };
        if workspace.worktree_space.as_ref() == Some(&membership) {
            return;
        }
        workspace.worktree_space = Some(membership);
        self.state.session_dirty = true;
        if announce && let Some(workspace) = self.workspace_info(index) {
            self.event_hub
                .push(EventData::WorkspaceUpdated { workspace });
        }
    }

    /// Makes sure a workspace shows the source checkout and carries its membership, creating
    /// the workspace when none exists; returns its position and whether it was created.
    ///
    /// # Errors
    /// Returns the failure to open a workspace for the source checkout.
    pub(super) async fn ensure_source_parent_membership(
        &mut self,
        source: &mut WorktreeSource,
        announce_creation: bool,
    ) -> Result<(usize, bool), ErrorBody> {
        let existing = source
            .workspace_idx
            .or_else(|| self.find_parent_workspace_by_key(&source.repo_key));
        let created = existing.is_none();
        let index = match existing {
            Some(index) => index,
            None => self
                .create_workspace_with_options(source.source_checkout_path.clone(), false)
                .map_err(|error| api_error("worktree_open_failed", error.to_string()))?,
        };
        source.workspace_idx = Some(index);
        let membership = source.membership(source.source_checkout_path.clone(), false);
        self.set_worktree_membership(index, membership, !created);
        if created && announce_creation {
            self.emit_workspace_open_events(index).await;
        }
        Ok((index, created))
    }

    /// Describes a listed checkout of `source`.
    pub(super) fn worktree_info_for_entry(
        &self,
        source: &WorktreeSource,
        entry: ExistingWorktree,
    ) -> WorktreeInfo {
        let canonical = canonicalize_best_effort_path(&entry.path);
        let repo_root = canonicalize_best_effort_path(&source.source_repo_root);
        WorktreeInfo {
            path: entry.path.display().to_string(),
            branch: entry.branch,
            is_bare: entry.is_bare,
            is_detached: entry.is_detached,
            is_prunable: entry.is_prunable,
            is_linked_worktree: (canonical != repo_root).into(),
            open_workspace_id: self
                .workspace_id_at(self.open_workspace_idx_for_checkout(&canonical)),
            label: source.repo_name.clone(),
        }
    }

    /// Describes where a listing started.
    pub(super) fn worktree_source_info(&self, source: &WorktreeSource) -> WorktreeSourceInfo {
        WorktreeSourceInfo {
            repo_key: source.repo_key.clone(),
            repo_name: source.repo_name.clone(),
            repo_root: source.source_repo_root.display().to_string(),
            source_checkout_path: source.source_checkout_path.display().to_string(),
            source_workspace_id: self.workspace_id_at(source.workspace_idx),
        }
    }

    /// The records a worktree answer carries for a workspace: it, its active tab and that
    /// tab's root pane.
    ///
    /// # Errors
    /// Returns `worktree_open_failed` when the workspace has no active root pane.
    pub(super) async fn worktree_records(
        &self,
        index: usize,
    ) -> Result<(WorkspaceInfo, TabInfo, PaneInfo), ErrorBody> {
        let tab = self
            .state
            .topology
            .workspaces
            .get(index)
            .map(|w| w.active_tab);
        let records = match tab {
            Some(tab) => (
                self.workspace_info(index),
                self.tab_info(index, tab),
                self.root_pane_info(index, tab).await,
            ),
            None => (None, None, None),
        };
        match records {
            (Some(workspace), Some(tab), Some(root_pane)) => Ok((workspace, tab, root_pane)),
            _ => Err(api_error(
                "worktree_open_failed",
                "the workspace has no active tab with a root pane",
            )),
        }
    }
}
