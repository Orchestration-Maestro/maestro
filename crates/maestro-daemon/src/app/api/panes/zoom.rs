//! `pane.zoom`: shows one pane of a tab alone, or shows the whole tab again.

use super::super::responses::pane_not_found;
use crate::app::App;
use maestro_protocol::{
    schema::{ErrorBody, PaneZoomParams, PaneZoomResult, ResponseResult},
    shared::Flag,
};

impl<C: FnMut() -> u128> App<C> {
    /// Applies a zoom mode to the tab of the named or focused pane, focusing that pane.
    ///
    /// A request that changes neither zoom nor focus publishes nothing.
    pub(super) fn handle_pane_zoom(
        &mut self,
        params: &PaneZoomParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let at = self.located_pane(params.pane_id.as_deref())?;
        let pane_id = self.public_pane(at.workspace, at.pane)?;
        let outcome = self
            .state
            .apply_pane_zoom(at.workspace, at.pane, params.mode)
            .ok_or_else(|| pane_not_found(&pane_id))?;
        let changed = outcome.changed || outcome.focus_changed;
        self.state.settle_terminal_mode_after_focus();
        let layout = self.tab_layout(at)?;
        if changed {
            self.emit_layout_updated_snapshot(layout.clone());
        }
        Ok(ResponseResult::PaneZoom {
            zoom: PaneZoomResult {
                changed,
                zoom_changed: outcome.changed,
                focus_changed: outcome.focus_changed,
                reason: outcome.reason,
                pane_id,
                focused_pane_id: layout.focused_pane_id.clone(),
                zoomed: Flag(outcome.zoomed),
                layout,
            },
        })
    }
}
