//! Helpers that the pane command tests share between the integration program and the unit tests.

use super::api::request;
use crate::state::App;
use maestro_protocol::schema::{ErrorBody, PaneDirection, PaneInfo, ResponseResult};
use maestro_workspace::tiling::PaneId;
use serde_json::{Value, json};

/// The pane a single-pane result reports.
///
/// # Errors
/// Returns the answer that was not a single pane.
pub fn reported(result: Result<ResponseResult, ErrorBody>) -> Result<PaneInfo, String> {
    match result {
        Ok(ResponseResult::PaneInfo { pane } | ResponseResult::PaneCurrent { pane }) => Ok(pane),
        other => Err(format!("expected a pane: {other:?}")),
    }
}

/// The pane identities a list result holds, in order.
///
/// # Errors
/// Returns the answer that was not a pane list.
pub fn listed(result: Result<ResponseResult, ErrorBody>) -> Result<Vec<String>, String> {
    match result {
        Ok(ResponseResult::PaneList { panes }) => {
            Ok(panes.into_iter().map(|pane| pane.pane_id).collect())
        }
        other => Err(format!("expected a pane list: {other:?}")),
    }
}

/// The public handle of a pane in the workspace at `workspace`.
#[must_use]
pub fn public(app: &App, workspace: usize, pane: PaneId) -> Option<String> {
    app.state.public_pane_id(workspace, pane)
}

/// The result of moving focus from `pane` toward `direction`.
///
/// # Errors
/// Returns the answer that was not a focus result, or the result that has no wire form.
pub fn focus_toward(app: &mut App, pane: &str, direction: PaneDirection) -> Result<Value, String> {
    let command = json!({"type": "pane.focus_direction", "pane_id": pane, "direction": direction});
    let Ok(ResponseResult::PaneFocusDirection { focus }) = request(app, command) else {
        return Err("expected a focus result".into());
    };
    serde_json::to_value(focus).map_err(|error| error.to_string())
}
