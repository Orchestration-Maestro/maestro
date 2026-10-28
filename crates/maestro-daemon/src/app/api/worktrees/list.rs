//! `worktree.list`: the checkouts of a repository and where each one is open.

use super::{super::responses::api_error, source::SourceRule};
use crate::{app::App, git::worktree};
use maestro_protocol::schema::{ErrorBody, ResponseResult, WorktreeListParams};

impl<C: FnMut() -> u128> App<C> {
    /// Lists the worktrees of the repository the request names.
    ///
    /// # Errors
    /// Returns why the request names no usable repository, or Git's failure to list.
    pub(in crate::app) fn handle_worktree_list(
        &self,
        params: &WorktreeListParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let source = self.resolve_worktree_source(
            params.workspace_id.as_deref(),
            params.cwd.as_deref(),
            SourceRule::AnyCheckout,
        )?;
        let entries = worktree::list(&source.source_repo_root)
            .map_err(|error| api_error("worktree_list_failed", error.to_string()))?;
        let worktrees = entries
            .into_iter()
            .map(|entry| self.worktree_info_for_entry(&source, entry))
            .collect();
        Ok(ResponseResult::WorktreeList {
            source: self.worktree_source_info(&source),
            worktrees,
        })
    }
}
