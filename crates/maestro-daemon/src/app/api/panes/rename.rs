//! `pane.rename`: sets or clears a pane's label.

use super::super::responses::pane_not_found;
use crate::app::App;
use maestro_protocol::schema::{ErrorBody, PaneRenameParams, ResponseResult};

impl<C: FnMut() -> u128> App<C> {
    /// Sets the named pane's label, or clears it when none is given or it is blank, and
    /// answers with the pane. A changed label is published as an updated pane.
    ///
    /// # Errors
    /// Returns `pane_not_found` for a pane that is gone.
    pub(super) async fn handle_pane_rename(
        &mut self,
        params: PaneRenameParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let (workspace, pane) = self.target_pane(&params.pane_id)?;
        let terminal = self.state.topology.workspaces[workspace]
            .terminal_id(pane)
            .and_then(|id| self.state.terminals.get_mut(id))
            .ok_or_else(|| pane_not_found(&params.pane_id))?;
        let previous = terminal.manual_label.clone();
        match &params.label {
            Some(label) => terminal.set_manual_label(label),
            None => terminal.clear_manual_label(),
        }
        let changed = terminal.manual_label != previous;
        self.state.session_dirty = true;
        if changed {
            self.emit_pane_updated(workspace, pane).await;
        }
        self.pane_info_result(workspace, pane, &params.pane_id)
            .await
    }
}
