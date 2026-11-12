//! Helpers that the pane move tests share between the integration program and the unit tests.

use super::api::{add_tab, daemon_with_workspaces, request, root_pane};
use crate::{events::AppEvent, state::App};
use maestro_detection::agents::Agent;
use maestro_protocol::schema::{ErrorBody, PaneMoveResult, ResponseResult};
use serde_json::{Value, json};

/// The move record a move result holds.
///
/// # Errors
/// Returns the answer that was not a move result.
pub fn move_of(result: Result<ResponseResult, ErrorBody>) -> Result<PaneMoveResult, String> {
    match result {
        Ok(ResponseResult::PaneMove { move_result }) => Ok(move_result),
        other => Err(format!("expected a move result: {other:?}")),
    }
}

/// The wire form of moving `pane` to `destination`.
#[must_use]
pub fn moving(pane: &str, destination: &Value, focus: bool) -> Value {
    json!({"type": "pane.move", "pane_id": pane, "destination": destination, "focus": focus})
}

/// The wire form of a destination next to `target` in `tab`.
#[must_use]
pub fn into_tab(tab: &str, target: Option<&str>, split: &str) -> Value {
    json!({"type": "tab", "tab_id": tab, "target_pane_id": target, "split": split})
}

/// One workspace with a first tab of a single pane and a second tab of a single pane.
///
/// # Errors
/// Returns why the daemon or the tab could not be built.
pub fn two_tabs() -> Result<App, String> {
    let mut app = daemon_with_workspaces(&["/issue"])?;
    add_tab(&mut app, 0).ok_or("the second tab was not added")?;
    Ok(app)
}

/// The terminal identity of the root pane of the workspace at `workspace`.
fn root_terminal(app: &App, workspace: usize) -> Result<String, String> {
    let root = root_pane(app, workspace);
    app.state.topology.workspaces[workspace]
        .terminal_id(root)
        .map(ToString::to_string)
        .ok_or_else(|| "the root pane has no terminal".to_owned())
}

/// The first tab's only pane moved beside the second tab's pane at ratio 0.25, with focus,
/// together with the terminal identity the pane had before.
///
/// # Errors
/// Returns why the daemon could not be built or the move was not answered.
pub fn into_the_second_tab() -> Result<(App, PaneMoveResult, String), String> {
    let mut app = two_tabs()?;
    let terminal = root_terminal(&app, 0)?;
    let mut destination = into_tab("w1:t2", Some("w1:p2"), "right");
    destination["ratio"] = json!(0.25);
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)))?;
    Ok((app, moved, terminal))
}

/// An agent pane alone in the first of two workspaces, moved beside the second's pane
/// without focus, with the terminal identity it had before.
///
/// # Errors
/// Returns why the daemon could not be built or the move was not answered.
pub fn across_workspaces() -> Result<(App, PaneMoveResult, String), String> {
    let mut app = daemon_with_workspaces(&["/issue", "/other"])?;
    let source = root_pane(&app, 0);
    let terminal = root_terminal(&app, 0)?;
    app.state.handle_app_event(AppEvent::AgentProcessDetected {
        pane_id: source,
        agent: Agent::Pi,
        observed_at: std::time::Instant::now(),
    });
    let destination = into_tab("w2:t1", Some("w2:p1"), "down");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, false)))?;
    Ok((app, moved, terminal))
}

/// The only pane of the only workspace promoted to a focused new workspace, with the
/// terminal identity the pane had before.
///
/// # Errors
/// Returns why the daemon could not be built or the move was not answered.
pub fn promoted() -> Result<(App, PaneMoveResult, String), String> {
    let mut app = daemon_with_workspaces(&["/issue"])?;
    let terminal = root_terminal(&app, 0)?;
    let destination = json!({"type": "new_workspace", "label": "promoted", "tab_label": "main"});
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)))?;
    Ok((app, moved, terminal))
}
