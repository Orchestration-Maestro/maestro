//! Helpers that the pane swap tests share between the integration program and the unit tests.

use super::api::request;
use crate::state::App;
use maestro_protocol::schema::{ErrorBody, PaneSwapResult, ResponseResult};
use maestro_workspace::tiling::PaneId;
use serde_json::json;

/// The swap record a swap result holds.
///
/// # Errors
/// Returns the answer that was not a swap result.
pub fn swap_of(result: Result<ResponseResult, ErrorBody>) -> Result<PaneSwapResult, String> {
    match result {
        Ok(ResponseResult::PaneSwap { swap }) => Ok(swap),
        other => Err(format!("expected a swap result: {other:?}")),
    }
}

/// The leaves of the first tab of the first workspace, in layout order.
#[must_use]
pub fn leaves(app: &App) -> Vec<PaneId> {
    app.state.topology.workspaces[0].tabs[0].layout.pane_ids()
}

/// Asks for a swap of `source` and `target` and returns the answer.
///
/// # Errors
/// Returns the answer that was not a swap result.
pub fn swap_named(app: &mut App, source: &str, target: &str) -> Result<PaneSwapResult, String> {
    let wire = json!({"type": "pane.swap", "source_pane_id": source, "target_pane_id": target});
    swap_of(request(app, wire))
}
