//! Helpers that the workspace command tests share between the integration program and the unit tests.

use maestro_protocol::schema::{ErrorBody, ResponseResult, WorkspaceInfo};

/// The workspace identities a list result holds, in order.
///
/// # Errors
/// Returns the answer that was not a workspace list.
pub fn listed(result: &Result<ResponseResult, ErrorBody>) -> Result<Vec<String>, String> {
    match result {
        Ok(ResponseResult::WorkspaceList { workspaces }) => Ok(ids_of(workspaces)),
        other => Err(format!("expected a workspace list: {other:?}")),
    }
}

/// The identities of `workspaces`, in order.
#[must_use]
pub fn ids_of(workspaces: &[WorkspaceInfo]) -> Vec<String> {
    workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect()
}
