//! `pane.resize`: moves the divider nearest a pane's edge.

use crate::app::{App, layout::navigation_direction};
use maestro_protocol::schema::{
    ErrorBody, PaneResizeParams, PaneResizeReason, PaneResizeResult, ResponseResult,
};

/// Fraction of the split moved when a request names no amount.
const DEFAULT_RESIZE_AMOUNT: f32 = 0.05;

/// Largest fraction of the split one request moves.
const MAX_RESIZE_AMOUNT: f32 = 0.5;

impl<C: FnMut() -> u128> App<C> {
    /// Moves the divider nearest the pane's edge in a direction; focus stays where it was.
    ///
    /// The sign of the amount is ignored and a non-finite amount means the default.
    pub(super) fn handle_pane_resize(
        &mut self,
        params: &PaneResizeParams,
    ) -> Result<ResponseResult, ErrorBody> {
        let at = self.located_pane(params.pane_id.as_deref())?;
        let pane_id = self.public_pane(at.workspace, at.pane)?;
        let amount = params
            .amount
            .filter(|amount| amount.is_finite())
            .unwrap_or(DEFAULT_RESIZE_AMOUNT)
            .abs()
            .min(MAX_RESIZE_AMOUNT);
        let area = self.state.view_area();
        let changed = self.state.topology.workspaces[at.workspace].tabs[at.tab]
            .layout
            .resize_pane(
                at.pane,
                navigation_direction(params.direction),
                amount,
                area,
            );
        if changed {
            self.state.session_dirty = true;
        }
        let layout = self.tab_layout(at)?;
        if changed {
            self.emit_layout_updated_snapshot(layout.clone());
        }
        Ok(ResponseResult::PaneResize {
            resize: PaneResizeResult {
                changed,
                reason: (!changed).then_some(PaneResizeReason::Unchanged),
                pane_id,
                focused_pane_id: layout.focused_pane_id.clone(),
                layout,
            },
        })
    }
}
