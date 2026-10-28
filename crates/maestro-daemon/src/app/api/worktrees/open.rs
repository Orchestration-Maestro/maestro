//! `worktree.open`: show a checkout of the repository in a workspace.

use super::{
    super::responses::api_error,
    absolute_user_path,
    source::{SourceRule, WorktreeSource},
};
use crate::{
    app::App,
    git::{
        discovery::canonicalize_best_effort_path,
        worktree::{self, ExistingWorktree},
    },
};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, ResponseResult, WorktreeOpenParams},
};

/// The refusal for a checkout that cannot be found.
fn not_found(message: &str) -> ErrorBody {
    api_error("worktree_not_found", message)
}

impl<C: FnMut() -> u128> App<C> {
    /// Opens the checkout the request names in its workspace, creating one when none shows it.
    ///
    /// # Errors
    /// Returns why the request names no usable checkout, or the failure to open a workspace.
    pub(in crate::app) async fn handle_worktree_open(
        &mut self,
        params: WorktreeOpenParams,
    ) -> Result<ResponseResult, ErrorBody> {
        if params.path.is_some() == params.branch.is_some() {
            return Err(api_error(
                "invalid_request",
                "exactly one of path or branch is required",
            ));
        }
        let mut source = self.resolve_worktree_source(
            params.workspace_id.as_deref(),
            params.cwd.as_deref(),
            SourceRule::ParentOnly,
        )?;
        let entry = find_worktree_entry(&source, params.path.as_deref(), params.branch.as_deref())?;
        if entry.is_bare || entry.is_prunable {
            return Err(not_found("worktree cannot be opened"));
        }
        let canonical = canonicalize_best_effort_path(&entry.path);
        let already_open = self.open_workspace_idx_for_checkout(&canonical);
        let (index, created) = self
            .workspace_for_checkout(&mut source, &entry, already_open, params.focus)
            .await?;
        let linked = canonical != canonicalize_best_effort_path(&source.source_repo_root);
        let membership = source.membership(entry.path.clone(), linked);
        self.set_worktree_membership(index, membership, !created);
        if let Some(label) = params.label {
            self.rename_opened_workspace(index, label, created);
        }
        self.state.session_dirty = true;
        if created {
            self.emit_workspace_open_events(index).await;
        }
        let worktree = self.worktree_info_for_entry(&source, entry);
        let (workspace, tab, root_pane) = self.worktree_records(index).await?;
        self.event_hub.push(EventData::WorktreeOpened {
            workspace: workspace.clone(),
            worktree: worktree.clone(),
            already_open: already_open.is_some(),
        });
        Ok(ResponseResult::WorktreeOpened {
            workspace,
            tab,
            root_pane,
            worktree,
            already_open: already_open.is_some(),
        })
    }

    /// The workspace that shows `entry`, and whether this call created it: the one already
    /// showing it, else the source workspace when the entry is the source checkout, else a new
    /// workspace. The source workspace is created when none exists.
    async fn workspace_for_checkout(
        &mut self,
        source: &mut WorktreeSource,
        entry: &ExistingWorktree,
        already_open: Option<usize>,
        focus: bool,
    ) -> Result<(usize, bool), ErrorBody> {
        let canonical = canonicalize_best_effort_path(&entry.path);
        let is_source = canonical == canonicalize_best_effort_path(&source.source_checkout_path);
        let announce_source = !(is_source && already_open.is_none());
        let (source_idx, source_created) = self
            .ensure_source_parent_membership(source, announce_source)
            .await?;
        let (index, created) = if let Some(index) = already_open {
            (index, false)
        } else if is_source {
            (source_idx, source_created)
        } else {
            let index = self
                .create_workspace_with_options(entry.path.clone(), focus)
                .map_err(|error| api_error("worktree_open_failed", error.to_string()))?;
            return Ok((index, true));
        };
        if focus {
            self.state.switch_workspace(index);
        }
        Ok((index, created))
    }

    /// Names an opened workspace; one that already existed also announces the new name.
    fn rename_opened_workspace(&mut self, index: usize, label: String, created: bool) {
        let Some(workspace) = self.state.topology.workspaces.get_mut(index) else {
            return;
        };
        workspace.set_custom_name(label.clone());
        crate::logging::workspace_renamed(&workspace.id);
        if !created {
            let workspace_id = workspace.id.clone();
            self.event_hub.push(EventData::WorkspaceRenamed {
                workspace_id,
                label,
            });
        }
    }
}

/// The listed checkout a request names, by path or by the one branch it has checked out.
fn find_worktree_entry(
    source: &WorktreeSource,
    path: Option<&str>,
    branch: Option<&str>,
) -> Result<ExistingWorktree, ErrorBody> {
    let entries = worktree::list(&source.source_repo_root)
        .map_err(|error| api_error("worktree_list_failed", error.to_string()))?;
    if let Some(path) = path {
        let expected = canonicalize_best_effort_path(&absolute_user_path(path)?);
        return entries
            .into_iter()
            .find(|entry| canonicalize_best_effort_path(&entry.path) == expected)
            .ok_or_else(|| not_found("worktree path not found"));
    }
    let mut matches = entries.into_iter().filter(|entry| {
        !entry.is_bare
            && !entry.is_prunable
            && !entry.is_detached
            && entry.branch.as_deref() == branch
    });
    match (matches.next(), matches.next()) {
        (None, _) => Err(not_found("worktree branch not found")),
        (Some(entry), None) => Ok(entry),
        (Some(_), Some(_)) => Err(api_error(
            "ambiguous_worktree_branch",
            "multiple worktrees matched branch",
        )),
    }
}
