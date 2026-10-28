//! `worktree.create`: add a checkout in the background, then open it in a workspace.

use super::{
    super::responses::api_error,
    absolute_user_path, home,
    info::{git_space_of, worktree_info_for_membership},
    operation_in_progress,
    operations::{CreateOperation, Operation, Pending, Responder},
    source::{SourceRule, WorktreeSource},
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
    schema::{ErrorBody, ResponseResult, WorktreeCreateParams},
};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// A create request that passed validation.
struct CreatePlan {
    /// Branch to check out, created from `base` when it does not exist.
    branch: String,
    /// Revision a new branch starts at.
    base: String,
    /// Directory Git adds the checkout at.
    checkout_path: PathBuf,
    /// Canonical spelling of `checkout_path`.
    checkout_key: PathBuf,
    /// Repository the checkout is added to.
    source: WorktreeSource,
    /// Name for the new workspace.
    label: Option<String>,
    /// Whether the new workspace takes focus.
    focus: bool,
}

/// A branch name for a request that gave none.
fn generated_branch() -> String {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
        });
    worktree::generated_branch_slug(micros)
}

/// Adds the checkout, creating the directory above it first.
fn add_checkout(
    repo_root: &Path,
    path: &Path,
    branch: &str,
    base: &str,
) -> Result<(), WorktreeError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| WorktreeError::new(error.to_string()))?;
    }
    worktree::add(repo_root, path, branch, base)
}

impl<C: FnMut() -> u128> App<C> {
    /// Validates a create request and starts `git worktree add` in the background; the
    /// answer reaches `respond_to` once Git is done, or at once when the request is refused.
    pub(in crate::app) fn start_worktree_create(
        &mut self,
        params: WorktreeCreateParams,
        respond_to: Responder,
    ) {
        match self.plan_worktree_create(params) {
            Ok(plan) => self.spawn_worktree_add(plan, respond_to),
            Err(error) => {
                let _ = respond_to.send(Err(error));
            }
        }
    }

    /// Checks the request, resolves its repository and picks the checkout directory.
    fn plan_worktree_create(&self, params: WorktreeCreateParams) -> Result<CreatePlan, ErrorBody> {
        let branch = params
            .branch
            .unwrap_or_else(generated_branch)
            .trim()
            .to_owned();
        if branch.is_empty() {
            return Err(api_error("invalid_request", "branch is required"));
        }
        let source = self.resolve_worktree_source(
            params.workspace_id.as_deref(),
            params.cwd.as_deref(),
            SourceRule::ParentOnly,
        )?;
        let checkout_path = if let Some(path) = params.path {
            absolute_user_path(&path)?
        } else {
            let root = worktree::worktrees_root(&self.config.worktrees.directory, home);
            worktree::default_checkout_path(&root, &source.repo_name, &branch)
        };
        let checkout_key = canonicalize_best_effort_path(&checkout_path);
        if self.worktree_operations.is_busy(&checkout_key, None) {
            return Err(operation_in_progress());
        }
        Ok(CreatePlan {
            branch,
            base: params.base.unwrap_or_else(|| "HEAD".into()),
            checkout_path,
            checkout_key,
            source,
            label: params.label,
            focus: params.focus,
        })
    }

    /// Registers the request and runs `git worktree add` away from the loop.
    fn spawn_worktree_add(&mut self, plan: CreatePlan, respond_to: Responder) {
        let CreatePlan {
            branch,
            base,
            checkout_path,
            checkout_key,
            source,
            label,
            focus,
        } = plan;
        let source_workspace = source
            .workspace_idx
            .and_then(|index| self.state.topology.workspaces.get(index));
        let operation = CreateOperation {
            checkout_path: checkout_path.clone(),
            checkout_key,
            source_workspace_id: source_workspace.map(|workspace| workspace.id.clone()),
            source_membership: source_workspace
                .and_then(|workspace| workspace.worktree_space().cloned()),
            source,
            label,
            focus,
        };
        let repo_root = operation.source.source_checkout_path.clone();
        let id = self.worktree_operations.start(Pending {
            respond_to,
            operation: Operation::Create(operation),
        });
        self.run_worktree_git(id, move || {
            add_checkout(&repo_root, &checkout_path, &branch, &base)
        });
    }

    /// The workspace that still stands for the source of a request: the one it started from
    /// while that workspace shows the same source, else the repository's own workspace.
    fn create_source_index(&self, create: &CreateOperation) -> Option<usize> {
        let source = &create.source;
        create
            .source_workspace_id
            .as_ref()
            .and_then(|id| {
                self.state
                    .topology
                    .workspaces
                    .iter()
                    .position(|workspace| &workspace.id == id)
            })
            .filter(|index| self.still_shows_source(*index, create))
            .or_else(|| self.find_parent_workspace_by_key(&source.repo_key))
    }

    /// Whether a workspace still carries the membership, or the repository identity, it had
    /// when the request started.
    fn still_shows_source(&self, index: usize, create: &CreateOperation) -> bool {
        let workspace = &self.state.topology.workspaces[index];
        let source = &create.source;
        match (
            create.source_membership.as_ref(),
            workspace.worktree_space(),
        ) {
            (Some(expected), current) => current == Some(expected),
            (None, Some(current)) => {
                *current == source.membership(source.source_checkout_path.clone(), false)
            }
            (None, None) => {
                let cwd = self.workspace_cwd(workspace);
                git_space_of(workspace, cwd.as_deref()).is_some_and(|space| {
                    !space.is_linked_worktree
                        && space.key == source.repo_key
                        && canonicalize_best_effort_path(&space.repo_root)
                            == canonicalize_best_effort_path(&source.source_repo_root)
                })
            }
        }
    }

    /// Opens the checkout Git added and builds the answer of the request.
    ///
    /// # Errors
    /// Returns Git's failure, or the failure to open a workspace for the checkout.
    pub(super) async fn finish_worktree_add(
        &mut self,
        create: CreateOperation,
        result: Result<(), WorktreeError>,
    ) -> Result<ResponseResult, ErrorBody> {
        result.map_err(|error| api_error("worktree_create_failed", error.to_string()))?;
        let mut source = WorktreeSource {
            workspace_idx: self.create_source_index(&create),
            ..create.source.clone()
        };
        self.ensure_source_parent_membership(&mut source, true)
            .await?;
        let path = create.checkout_path;
        let (index, created) = if let Some(index) = self.open_workspace_idx_for_checkout(&path) {
            if create.focus {
                self.state.switch_workspace(index);
            }
            (index, false)
        } else {
            let index = self
                .create_workspace_with_options(path.clone(), create.focus)
                .map_err(|error| {
                    api_error(
                        "worktree_open_failed",
                        format!("created worktree but failed to open workspace: {error}"),
                    )
                })?;
            (index, true)
        };
        let membership = source.membership(path, true);
        self.set_worktree_membership(index, membership.clone(), !created);
        if let Some(label) = create.label
            && let Some(workspace) = self.state.topology.workspaces.get_mut(index)
        {
            workspace.set_custom_name(label);
        }
        self.state.session_dirty = true;
        if created {
            self.emit_workspace_open_events(index).await;
        }
        let (workspace, tab, root_pane) = self.worktree_records(index).await?;
        let worktree =
            worktree_info_for_membership(&membership, Some(workspace.workspace_id.clone()));
        self.event_hub.push(EventData::WorktreeCreated {
            workspace: workspace.clone(),
            worktree: worktree.clone(),
        });
        Ok(ResponseResult::WorktreeCreated {
            workspace,
            tab,
            root_pane,
            worktree,
        })
    }
}
