//! `pane.close`: closes a pane, and its tab or workspace when it was the last one there.

use super::super::responses::{api_error, pane_not_found};
use crate::app::App;
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, ResponseResult},
};

impl<C: FnMut() -> u128> App<C> {
    /// Closes one pane with its terminal; the last pane of a tab closes the tab, and the
    /// last pane of a workspace closes the workspace.
    ///
    /// # Errors
    /// Returns `confirmation_required` when closing would close a worktree group that asks first.
    pub(super) async fn handle_pane_close(
        &mut self,
        id: &str,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.target_pane(id)?;
        let pane_id = self
            .public_pane(workspace, pane)
            .map_err(|_| pane_not_found(id))?;
        let workspace_id = self.state.topology.workspaces[workspace].id.clone();
        let layout_update = self.layout_update_target_after_pane_removal(workspace, pane);
        if self.state.close_pane_would_close_workspace(workspace, pane)
            && self.state.confirm_implicit_worktree_group_close(workspace)
        {
            return Err(api_error(
                "confirmation_required",
                "closing this pane would close a worktree group",
            ));
        }
        let record = self.workspace_info(workspace);
        let closed_workspace = self.state.close_pane_in_workspace(workspace, pane);
        self.state.session_dirty = true;
        self.shutdown_detached_terminal_runtimes().await;
        self.event_hub.push(EventData::PaneClosed {
            pane_id,
            workspace_id: workspace_id.clone(),
        });
        if closed_workspace {
            self.event_hub.push(EventData::WorkspaceClosed {
                workspace_id,
                workspace: record,
            });
        } else if let Some((workspace, tab)) = layout_update {
            self.emit_layout_updated_event(workspace, tab);
        }
        Ok(ResponseResult::Ok {})
    }
}
