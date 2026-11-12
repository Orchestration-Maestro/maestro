//! Helpers that the pane resize tests share between the integration program and the unit tests.

use super::api::{daemon_with_workspaces, pane_layout, root_pane, split_pane_focused};
use crate::state::App;
use maestro_protocol::schema::{ErrorBody, PaneResizeResult, ResponseResult, SplitDirection};
use maestro_workspace::tiling::PaneId;

/// The resize record a resize result holds.
///
/// # Errors
/// Returns the answer that was not a resize result.
pub fn resize_of(result: Result<ResponseResult, ErrorBody>) -> Result<PaneResizeResult, String> {
    match result {
        Ok(ResponseResult::PaneResize { resize }) => Ok(resize),
        other => Err(format!("expected a resize result: {other:?}")),
    }
}

/// A one-workspace daemon whose first tab holds a pane on the left and the focused one on the right.
///
/// # Errors
/// Returns why the daemon or the split could not be built.
pub fn pair() -> Result<(App, PaneId, PaneId), String> {
    let mut app = daemon_with_workspaces(&["/one"])?;
    let left = root_pane(&app, 0);
    let right = split_pane_focused(&mut app, 0, left, SplitDirection::Right, true)
        .ok_or("the first pane was not split")?;
    Ok((app, left, right))
}

/// The ratio of the tab's only divider.
///
/// # Errors
/// Returns the answer that was not a pane layout, or the missing divider.
pub fn ratio(app: &mut App) -> Result<f32, String> {
    pane_layout(app)?
        .splits
        .first()
        .map(|split| split.ratio)
        .ok_or_else(|| "the tab has no divider".to_owned())
}
