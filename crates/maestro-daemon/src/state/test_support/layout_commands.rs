//! Helpers that the layout command tests share between the integration program and the unit tests.

use super::api::{add_tab, daemon_with_workspaces, split_pane};
use crate::state::App;
use maestro_protocol::schema::{ErrorBody, LayoutDescription, ResponseResult, SplitDirection};
use maestro_workspace::tiling::PaneId;

/// The layout an export answers with.
///
/// # Errors
/// Returns the answer that was not an exported layout.
pub fn exported(result: Result<ResponseResult, ErrorBody>) -> Result<LayoutDescription, String> {
    match result {
        Ok(ResponseResult::LayoutExport { layout }) => Ok(layout),
        other => Err(format!("expected an exported layout: {other:?}")),
    }
}

/// The layout a ratio change answers with.
///
/// # Errors
/// Returns the answer that was not the adjusted layout.
pub fn adjusted(result: Result<ResponseResult, ErrorBody>) -> Result<LayoutDescription, String> {
    match result {
        Ok(ResponseResult::LayoutSplitRatioSet { layout }) => Ok(layout),
        other => Err(format!("expected the adjusted layout: {other:?}")),
    }
}

/// A workspace whose first tab holds one pane and whose second tab splits two.
///
/// Returns the daemon with the panes of the second tab: its root and the one split off it.
///
/// # Errors
/// Returns why the daemon, the tab or the split could not be built.
pub fn two_tabs() -> Result<(App, PaneId, PaneId), String> {
    let mut app = daemon_with_workspaces(&["/one"])?;
    let root = add_tab(&mut app, 0).ok_or("the second tab was not added")?;
    let right = split_pane(&mut app, 0, root, SplitDirection::Right)
        .ok_or("the second tab was not split")?;
    Ok((app, root, right))
}
