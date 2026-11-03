//! Pane resize through the daemon's handler: which divider moves, by how much, and what is published.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, answer_command, daemon_with_workspaces, event_names, events_after, failure, request,
    root_pane, split_pane,
};
use crate::support::runtime::block_on;
use maestro_protocol::events::EventData;
use maestro_protocol::rpc_types::RpcCommand;
use maestro_protocol::schema::{
    ErrorBody, PaneDirection, PaneResizeParams, PaneResizeReason, PaneResizeResult, ResponseResult,
    SplitDirection,
};
use maestro_workspace::layout::PaneId;
use serde_json::json;

/// The resize record a resize result holds.
fn resize_of(result: Result<ResponseResult, ErrorBody>) -> PaneResizeResult {
    match result {
        Ok(ResponseResult::PaneResize { resize }) => resize,
        other => panic!("expected a resize result: {other:?}"),
    }
}

/// A one-workspace daemon whose first tab holds a pane on the left and the focused one on the right.
fn pair() -> (TestApp, PaneId, PaneId) {
    let mut app = daemon_with_workspaces(&["/one"]);
    let left = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, left, SplitDirection::Right);
    app.state.topology.workspaces[0].tabs[0]
        .layout
        .focus_pane(right);
    (app, left, right)
}

/// The ratio of the tab's only divider.
fn ratio(app: &TestApp) -> f32 {
    app.state.topology.workspaces[0].tabs[0]
        .layout
        .splits(app.state.view_area())[0]
        .ratio
}

/// Resizes `pane` rightward by a decoded amount, which the wire form cannot carry.
fn resize_by(app: &mut TestApp, amount: Option<f32>) -> PaneResizeResult {
    let command = RpcCommand::PaneResize(PaneResizeParams {
        pane_id: Some("w1:p1".into()),
        direction: PaneDirection::Right,
        amount,
    });
    resize_of(block_on(answer_command(app, command)))
}

#[test]
fn resizing_moves_the_nearest_divider_without_changing_focus() {
    let (mut app, _, right) = pair();
    let wire =
        json!({"type": "pane.resize", "pane_id": "w1:p1", "direction": "right", "amount": 0.1});
    let resize = resize_of(request(&mut app, wire));
    assert!(resize.changed);
    assert_eq!(resize.reason, None);
    assert_eq!(resize.pane_id, "w1:p1");
    assert_eq!(resize.focused_pane_id, "w1:p2");
    assert_eq!(resize.layout.focused_pane_id, "w1:p2");
    assert!((resize.layout.splits[0].ratio - 0.6).abs() < f32::EPSILON);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(right)
    );
    assert!(app.state.session_dirty);
    assert_eq!(
        events_after(&app, 0),
        [EventData::LayoutUpdated {
            layout: resize.layout
        }]
    );
}

#[test]
fn resizing_without_a_divider_to_move_reports_unchanged() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let wire = json!({"type": "pane.resize", "direction": "left"});
    let resize = resize_of(request(&mut app, wire));
    assert!(!resize.changed);
    assert_eq!(resize.reason, Some(PaneResizeReason::Unchanged));
    assert_eq!(resize.pane_id, "w1:p1");
    assert_eq!(resize.focused_pane_id, "w1:p1");
    assert!(!app.state.session_dirty);
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn a_resize_amount_defaults_ignores_sign_and_is_capped() {
    let (mut app, _, _) = pair();
    for (amount, expected) in [
        (None, 0.55),
        (Some(f32::NAN), 0.6),
        (Some(f32::INFINITY), 0.65),
        (Some(-0.1), 0.75),
        (Some(9.0), 0.9),
    ] {
        assert!(resize_by(&mut app, amount).changed, "{amount:?}");
        assert!((ratio(&app) - expected).abs() < 1e-5, "{amount:?}");
    }
    assert!(!resize_by(&mut app, Some(0.2)).changed);
    assert_eq!(event_names(&app, 0), ["layout_updated"; 5]);
}

#[test]
fn resizing_a_missing_pane_is_refused_without_a_change() {
    let (mut app, _, _) = pair();
    let wire = json!({"type": "pane.resize", "pane_id": "w1:p9", "direction": "right"});
    assert_eq!(
        request(&mut app, wire),
        failure("pane_not_found", "pane not found")
    );
    assert!((ratio(&app) - 0.5).abs() < f32::EPSILON);
    assert!(events_after(&app, 0).is_empty());
}
