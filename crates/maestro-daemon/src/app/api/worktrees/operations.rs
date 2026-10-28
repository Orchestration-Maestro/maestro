//! Worktree requests waiting for Git: who asked, what the request holds and what it owes.

use super::source::WorktreeSource;
use crate::{app::App, events::AppEvent, git::worktree::WorktreeError};
use maestro_protocol::schema::{ErrorBody, ResponseResult, WorkspaceInfo, WorktreeInfo};
use maestro_workspace::workspace::WorktreeSpaceMembership;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::sync::oneshot;

/// The answer a worktree request is owed.
pub(in crate::app) type Responder = oneshot::Sender<Result<ResponseResult, ErrorBody>>;

/// A `worktree.create` waiting for `git worktree add`.
pub(super) struct CreateOperation {
    /// Directory Git adds the checkout at, as the caller spelled it.
    pub checkout_path: PathBuf,
    /// Canonical spelling of the checkout, shared by every operation on it.
    pub checkout_key: PathBuf,
    /// Repository the checkout belongs to.
    pub source: WorktreeSource,
    /// Identity of the source workspace when the request arrived.
    pub source_workspace_id: Option<String>,
    /// Membership the source workspace had when the request arrived.
    pub source_membership: Option<WorktreeSpaceMembership>,
    /// Name for the new workspace.
    pub label: Option<String>,
    /// Whether the new workspace takes focus.
    pub focus: bool,
}

/// A `worktree.remove` waiting for `git worktree remove`.
pub(super) struct RemoveOperation {
    /// Identity of the workspace holding the checkout.
    pub workspace_id: String,
    /// Checkout directory as the workspace's membership spelled it.
    pub checkout_path: PathBuf,
    /// Canonical spelling of the checkout, shared by every operation on it.
    pub checkout_key: PathBuf,
    /// The workspace as it was when the request arrived.
    pub workspace: WorkspaceInfo,
    /// The checkout as it was when the request arrived.
    pub worktree: WorktreeInfo,
    /// Whether Git was told to discard local changes.
    pub forced: bool,
}

/// What Git is doing for one request.
pub(super) enum Operation {
    /// A checkout is being added.
    Create(CreateOperation),
    /// A checkout is being removed.
    Remove(RemoveOperation),
}

impl Operation {
    /// The canonical checkout this operation owns.
    fn checkout_key(&self) -> &Path {
        match self {
            Self::Create(create) => &create.checkout_key,
            Self::Remove(remove) => &remove.checkout_key,
        }
    }

    /// The workspace this operation removes the checkout of, if any.
    fn removed_workspace(&self) -> Option<&str> {
        match self {
            Self::Create(_) => None,
            Self::Remove(remove) => Some(&remove.workspace_id),
        }
    }
}

/// A request and the answer it is owed once Git is done.
pub(super) struct Pending {
    /// Receives the answer.
    pub respond_to: Responder,
    /// What Git is doing.
    pub operation: Operation,
}

/// Every worktree request in flight, and the Git results not yet applied.
#[derive(Default)]
pub(in crate::app) struct WorktreeOperations {
    /// Identity handed to the next operation.
    next_id: u64,
    /// Requests whose Git command has not been applied yet.
    pending: HashMap<u64, Pending>,
    /// Git outcomes waiting for the loop to apply them.
    finished: Vec<(u64, Result<(), WorktreeError>)>,
}

impl WorktreeOperations {
    /// Registers a request and returns the identity its Git result will carry.
    pub(super) fn start(&mut self, pending: Pending) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.pending.insert(id, pending);
        id
    }

    /// Records a Git outcome for the loop to apply.
    pub(in crate::app) fn finish(&mut self, id: u64, result: Result<(), WorktreeError>) {
        self.finished.push((id, result));
    }

    /// Hands over the outcomes recorded since the last call, in arrival order.
    pub(super) fn take_finished(&mut self) -> Vec<(u64, Result<(), WorktreeError>)> {
        std::mem::take(&mut self.finished)
    }

    /// Releases the request an outcome belongs to.
    pub(super) fn take(&mut self, id: u64) -> Option<Pending> {
        self.pending.remove(&id)
    }

    /// Whether another request owns the checkout or is already removing `workspace_id`.
    pub(super) fn is_busy(&self, checkout_key: &Path, workspace_id: Option<&str>) -> bool {
        self.pending.values().any(|pending| {
            let operation = &pending.operation;
            operation.checkout_key() == checkout_key
                || workspace_id.is_some_and(|id| operation.removed_workspace() == Some(id))
        })
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// Runs a Git command away from the loop and delivers its outcome as an observation.
    pub(super) fn run_worktree_git(
        &self,
        operation_id: u64,
        work: impl FnOnce() -> Result<(), WorktreeError> + Send + 'static,
    ) {
        let events = self.launch.events.clone();
        tokio::task::spawn_blocking(move || {
            let result = work();
            let _ = events.blocking_send(AppEvent::WorktreeOperationFinished {
                operation_id,
                result,
            });
        });
    }

    /// Applies every Git outcome the loop has received and answers the requests waiting on them.
    pub async fn settle_worktree_operations(&mut self) {
        let finished = self.worktree_operations.take_finished();
        if finished.is_empty() {
            return;
        }
        for (id, result) in finished {
            let Some(Pending {
                respond_to,
                operation,
            }) = self.worktree_operations.take(id)
            else {
                continue;
            };
            let answer = match operation {
                Operation::Create(create) => self.finish_worktree_add(create, result).await,
                Operation::Remove(remove) => self.finish_worktree_remove(remove, result).await,
            };
            let _ = respond_to.send(answer);
        }
        self.launch.render_dirty.request_generic();
        self.launch.render_notify.notify_one();
    }
}
