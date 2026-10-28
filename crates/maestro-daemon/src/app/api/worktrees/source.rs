//! The repository a worktree command starts from.

use super::{super::responses::api_error, absolute_user_path, info::git_space_of};
use crate::{
    app::App,
    git::{
        discovery::{GitSpaceMetadata, git_space_metadata},
        worktree,
    },
};
use maestro_protocol::schema::ErrorBody;
use maestro_workspace::workspace::WorktreeSpaceMembership;
use std::path::PathBuf;

/// The repository a worktree command works on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorktreeSource {
    /// Workspace showing the source checkout, when one is open.
    pub workspace_idx: Option<usize>,
    /// Checkout the command starts from.
    pub source_checkout_path: PathBuf,
    /// Repository root Git commands run in.
    pub source_repo_root: PathBuf,
    /// Repository grouping key.
    pub repo_key: String,
    /// Repository grouping name.
    pub repo_name: String,
}

impl WorktreeSource {
    /// The membership of `checkout_path` in this repository's group.
    pub(super) fn membership(
        &self,
        checkout_path: PathBuf,
        is_linked_worktree: bool,
    ) -> WorktreeSpaceMembership {
        WorktreeSpaceMembership {
            key: self.repo_key.clone(),
            label: self.repo_name.clone(),
            repo_root: self.source_repo_root.clone(),
            checkout_path,
            is_linked_worktree,
        }
    }
}

/// Which checkouts a command may start from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceRule {
    /// Only the repository's own checkout; a linked checkout is refused.
    ParentOnly,
    /// Any checkout; a linked one stands for the checkout it was added from.
    AnyCheckout,
}

/// The refusal for a linked checkout where only the repository's own is accepted.
fn linked_source() -> ErrorBody {
    api_error(
        "linked_worktree_source",
        "New and open worktree actions start from the repo parent workspace.",
    )
}

/// The repository's own checkout, found by listing the worktrees of a linked one.
fn parent_checkout_path(space: &GitSpaceMetadata) -> PathBuf {
    worktree::list(&space.repo_root)
        .ok()
        .and_then(|entries| {
            entries.into_iter().find_map(|entry| {
                let entry_space = git_space_metadata(&entry.path)?;
                (entry_space.key == space.key && !entry_space.is_linked_worktree)
                    .then_some(entry_space.repo_root)
            })
        })
        .unwrap_or_else(|| space.repo_root.clone())
}

impl<C: FnMut() -> u128> App<C> {
    /// Resolves the repository a command names, or the active workspace's when it names none.
    ///
    /// # Errors
    /// Returns why the request names no usable repository.
    pub(super) fn resolve_worktree_source(
        &self,
        workspace_id: Option<&str>,
        cwd: Option<&str>,
        rule: SourceRule,
    ) -> Result<WorktreeSource, ErrorBody> {
        match (workspace_id, cwd) {
            (Some(_), Some(_)) => Err(api_error(
                "invalid_request",
                "only one of workspace_id or cwd may be supplied",
            )),
            (Some(id), None) => self.source_from_workspace(self.target_workspace(id)?, rule),
            (None, Some(cwd)) => self.source_from_directory(cwd, rule),
            (None, None) => {
                let workspaces = &self.state.topology.workspaces;
                let active = self.state.active.or_else(|| {
                    workspaces
                        .get(self.state.selected)
                        .map(|_| self.state.selected)
                });
                let index = active.ok_or_else(|| {
                    api_error(
                        "invalid_request",
                        "workspace_id or cwd is required when no workspace is active",
                    )
                })?;
                self.source_from_workspace(index, rule)
            }
        }
    }

    /// The repository holding a directory the caller named.
    fn source_from_directory(
        &self,
        cwd: &str,
        rule: SourceRule,
    ) -> Result<WorktreeSource, ErrorBody> {
        let path = absolute_user_path(cwd)?;
        let space = git_space_metadata(&path).ok_or_else(|| {
            api_error(
                "not_git_worktree",
                "Maestro worktree actions require a path inside a Git work tree",
            )
        })?;
        self.source_from_space(space, None, rule)
    }

    /// The repository a workspace shows, by its membership or its repository identity.
    fn source_from_workspace(
        &self,
        index: usize,
        rule: SourceRule,
    ) -> Result<WorktreeSource, ErrorBody> {
        let workspace = &self.state.topology.workspaces[index];
        if let Some(membership) = workspace.worktree_space() {
            return self.source_from_membership(index, membership, rule);
        }
        let cwd = self.workspace_cwd(workspace);
        let space = git_space_of(workspace, cwd.as_deref()).ok_or_else(|| {
            api_error(
                "not_git_worktree",
                "Maestro worktree actions require a workspace inside a Git work tree",
            )
        })?;
        self.source_from_space(space, Some(index), rule)
    }

    /// The repository a workspace's recorded membership names.
    fn source_from_membership(
        &self,
        index: usize,
        membership: &WorktreeSpaceMembership,
        rule: SourceRule,
    ) -> Result<WorktreeSource, ErrorBody> {
        let (workspace_idx, source_checkout_path) = if membership.is_linked_worktree {
            if rule == SourceRule::ParentOnly {
                return Err(linked_source());
            }
            (
                self.open_workspace_idx_for_checkout(&membership.repo_root),
                membership.repo_root.clone(),
            )
        } else {
            (Some(index), membership.checkout_path.clone())
        };
        Ok(WorktreeSource {
            workspace_idx,
            source_checkout_path,
            source_repo_root: membership.repo_root.clone(),
            repo_key: membership.key.clone(),
            repo_name: membership.label.clone(),
        })
    }

    /// The repository a discovered checkout belongs to; `shown_in` is the workspace that
    /// already showed it, when the caller started from one.
    fn source_from_space(
        &self,
        space: GitSpaceMetadata,
        shown_in: Option<usize>,
        rule: SourceRule,
    ) -> Result<WorktreeSource, ErrorBody> {
        let (workspace_idx, source_checkout_path) = if space.is_linked_worktree {
            if rule == SourceRule::ParentOnly {
                return Err(linked_source());
            }
            let parent = parent_checkout_path(&space);
            (self.open_workspace_idx_for_checkout(&parent), parent)
        } else {
            (
                shown_in.or_else(|| self.find_parent_workspace_for_space(&space)),
                space.repo_root.clone(),
            )
        };
        Ok(WorktreeSource {
            workspace_idx,
            source_repo_root: source_checkout_path.clone(),
            source_checkout_path,
            repo_key: space.key,
            repo_name: space.repo_name,
        })
    }
}
