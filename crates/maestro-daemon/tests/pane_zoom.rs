//! Pane zoom through the daemon's handler: modes, focus changes and no-ops.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, command, daemon_with_workspaces, event_names, events_after, failure, request,
    root_pane, split_pane,
};
use maestro_daemon::app::state::{CopyModeState, Mode};
use maestro_protocol::events::EventData;
use maestro_protocol::rpc_types::RpcCommand;
use maestro_protocol::schema::{
    ErrorBody, PaneZoomMode, PaneZoomReason, PaneZoomResult, ResponseResult, SplitDirection,
};
use maestro_workspace::layout::PaneId;
use serde_json::json;

/// The zoom record a zoom result holds.
fn zoom_of(result: Result<ResponseResult, ErrorBody>) -> PaneZoomResult {
    match result {
        Ok(ResponseResult::PaneZoom { zoom }) => zoom,
        other => panic!("expected a zoom result: {other:?}"),
    }
}

/// A one-workspace daemon whose first tab holds a pane on the left and one on the right.
fn pair() -> (TestApp, PaneId, PaneId) {
    let mut app = daemon_with_workspaces(&["/one"]);
    let left = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, left, SplitDirection::Right);
    (app, left, right)
}

/// The layout carried by the last published event.
fn last_layout(app: &TestApp) -> maestro_protocol::schema::PaneLayoutSnapshot {
    match events_after(app, 0).pop() {
        Some(EventData::LayoutUpdated { layout }) => layout,
        other => panic!("expected a layout event last: {other:?}"),
    }
}

#[test]
fn zooming_toggles_the_focused_pane_and_publishes_the_layout() {
    let (mut app, left, _) = pair();
    let zoom = zoom_of(request(&mut app, json!({"type": "pane.zoom"})));
    assert!(zoom.changed && zoom.zoom_changed && !zoom.focus_changed);
    assert_eq!(zoom.reason, None);
    assert_eq!(zoom.pane_id, "w1:p1");
    assert_eq!(zoom.focused_pane_id, zoom.pane_id);
    assert!(zoom.zoomed.0 && zoom.layout.zoomed);
    assert!(app.state.topology.workspaces[0].tabs[0].zoomed);
    assert_eq!(event_names(&app, 0), ["layout_updated"]);
    assert_eq!(last_layout(&app), zoom.layout);
    assert!(app.state.session_dirty);
    let zoom = zoom_of(request(&mut app, json!({"type": "pane.zoom"})));
    assert!(zoom.changed && zoom.zoom_changed && !zoom.focus_changed);
    assert!(!zoom.zoomed.0 && !zoom.layout.zoomed);
    assert!(!last_layout(&app).zoomed);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
}

#[test]
fn zooming_a_background_pane_focuses_it_and_keeps_the_last_pane_history() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]);
    let first = root_pane(&app, 0);
    let target = root_pane(&app, 1);
    split_pane(&mut app, 1, target, SplitDirection::Right);
    assert_eq!(app.state.active, Some(0));
    let wire = json!({"type": "pane.zoom", "pane_id": "w2:p1", "mode": "on"});
    let zoom = zoom_of(request(&mut app, wire));
    assert!(zoom.changed && zoom.zoom_changed && zoom.focus_changed);
    assert_eq!(zoom.pane_id, "w2:p1");
    assert_eq!(app.state.active, Some(1));
    assert_eq!(
        app.state.topology.workspaces[1].focused_pane_id(),
        Some(target)
    );
    assert!(app.state.topology.workspaces[1].tabs[0].zoomed);
    app.state.last_pane();
    assert_eq!(app.state.active, Some(0));
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(first)
    );
}

#[test]
fn zooming_a_pane_returns_copy_mode_to_that_pane() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]);
    let source = root_pane(&app, 0);
    let other = split_pane(&mut app, 0, source, SplitDirection::Right);
    app.state.topology.workspaces[0].tabs[0]
        .layout
        .focus_pane(other);
    let target = root_pane(&app, 1);
    split_pane(&mut app, 1, target, SplitDirection::Right);
    app.state.active = Some(1);
    app.state.selected = 1;
    app.state.mode = Mode::Terminal;
    app.state.copy_mode = Some(CopyModeState {
        pane_id: source,
        selection: None,
    });
    let wire = json!({"type": "pane.zoom", "pane_id": "w1:p1", "mode": "on"});
    assert!(zoom_of(request(&mut app, wire)).focus_changed);
    assert_eq!(app.state.active, Some(0));
    assert_eq!(app.state.mode, Mode::Copy);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(source)
    );
    assert_eq!(
        app.state.topology.workspaces[1].focused_pane_id(),
        Some(target)
    );
}

#[test]
fn zooming_a_single_pane_changes_nothing() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let wire = json!({"type": "pane.zoom", "pane_id": "w1:p1", "mode": "toggle"});
    let zoom = zoom_of(request(&mut app, wire));
    assert!(!zoom.changed && !zoom.zoom_changed && !zoom.focus_changed);
    assert_eq!(zoom.reason, Some(PaneZoomReason::SinglePane));
    assert_eq!(zoom.pane_id, "w1:p1");
    assert!(!zoom.zoomed.0);
    assert!(!app.state.topology.workspaces[0].tabs[0].zoomed);
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn zoom_on_and_off_are_idempotent() {
    let (mut app, _, _) = pair();
    for (mode, changed, reason, zoomed) in [
        ("on", true, None, true),
        ("on", false, Some(PaneZoomReason::AlreadyZoomed), true),
        ("off", true, None, false),
        ("off", false, Some(PaneZoomReason::AlreadyUnzoomed), false),
    ] {
        let wire = json!({"type": "pane.zoom", "pane_id": "w1:p1", "mode": mode});
        let zoom = zoom_of(request(&mut app, wire));
        assert_eq!(zoom.changed, changed, "{mode}");
        assert_eq!(zoom.zoom_changed, changed, "{mode}");
        assert!(!zoom.focus_changed, "{mode}");
        assert_eq!(zoom.reason, reason, "{mode}");
        assert_eq!(zoom.zoomed.0, zoomed, "{mode}");
    }
    assert_eq!(event_names(&app, 0), ["layout_updated", "layout_updated"]);
}

#[test]
fn an_idempotent_zoom_still_reports_the_focus_change() {
    let (mut app, _, right) = pair();
    app.state.topology.workspaces[0].tabs[0].zoomed = true;
    let wire = json!({"type": "pane.zoom", "pane_id": "w1:p2", "mode": "on"});
    let zoom = zoom_of(request(&mut app, wire));
    assert!(zoom.changed && !zoom.zoom_changed && zoom.focus_changed);
    assert_eq!(zoom.reason, Some(PaneZoomReason::AlreadyZoomed));
    assert!(zoom.zoomed.0);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(right)
    );
    assert_eq!(last_layout(&app).focused_pane_id, "w1:p2");
}

#[test]
fn zooming_a_missing_pane_is_refused() {
    let (mut app, _, _) = pair();
    for wire in [
        json!({"type": "pane.zoom", "pane_id": "w1:p9"}),
        json!({"type": "pane.zoom", "pane_id": "missing", "mode": "on"}),
    ] {
        assert_eq!(
            request(&mut app, wire),
            failure("pane_not_found", "pane not found")
        );
    }
    app.state.active = None;
    assert_eq!(
        request(&mut app, json!({"type": "pane.zoom"})),
        failure("pane_not_found", "pane not found")
    );
}

#[test]
fn zoom_modes_decode_from_the_wire_form() {
    let wire = json!({"type": "pane.zoom", "pane_id": "issue-1", "mode": "on"});
    let RpcCommand::PaneZoom(params) = command(wire.clone()) else {
        panic!("expected a zoom command");
    };
    assert_eq!(params.pane_id.as_deref(), Some("issue-1"));
    assert_eq!(params.mode, PaneZoomMode::On);
    assert_eq!(
        serde_json::to_value(RpcCommand::PaneZoom(params)).unwrap(),
        wire
    );
    let RpcCommand::PaneZoom(params) = command(json!({"type": "pane.zoom"})) else {
        panic!("expected a zoom command");
    };
    assert_eq!(params.mode, PaneZoomMode::Toggle);
}
