//! Pane swap through the daemon's handler: explicit and directional swaps, no-ops and refusals.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, add_tab, daemon_with_workspaces, event_names, events_after, failure, request,
    root_pane, split_pane,
};
use maestro_protocol::events::EventData;
use maestro_protocol::schema::{
    ErrorBody, PaneSwapReason, PaneSwapResult, ResponseResult, SplitDirection,
};
use maestro_workspace::layout::PaneId;
use serde_json::{Value, json};

/// The swap record a swap result holds.
fn swap_of(result: Result<ResponseResult, ErrorBody>) -> PaneSwapResult {
    match result {
        Ok(ResponseResult::PaneSwap { swap }) => swap,
        other => panic!("expected a swap result: {other:?}"),
    }
}

/// A one-workspace daemon whose first tab holds a pane on the left (focused) and one on the right.
fn pair() -> (TestApp, PaneId, PaneId) {
    let mut app = daemon_with_workspaces(&["/one"]);
    let left = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, left, SplitDirection::Right);
    (app, left, right)
}

/// The leaves of the first tab of the first workspace, in layout order.
fn leaves(app: &TestApp) -> Vec<PaneId> {
    app.state.topology.workspaces[0].tabs[0].layout.pane_ids()
}

/// Asks for a swap of `source` and `target` and returns the answer.
fn swap_named(app: &mut TestApp, source: &str, target: &str) -> PaneSwapResult {
    let wire = json!({"type": "pane.swap", "source_pane_id": source, "target_pane_id": target});
    swap_of(request(app, wire))
}

#[test]
fn swapping_two_named_panes_keeps_the_source_focused_and_returns_the_layout() {
    let (mut app, left, right) = pair();
    let swap = swap_named(&mut app, "w1:p1", "w1:p2");
    assert!(swap.changed);
    assert_eq!(swap.reason, None);
    assert_eq!(swap.source_pane_id, "w1:p1");
    assert_eq!(swap.target_pane_id.as_deref(), Some("w1:p2"));
    assert_eq!(swap.focused_pane_id, swap.source_pane_id);
    assert_eq!(swap.layout.focused_pane_id, swap.source_pane_id);
    assert_eq!(swap.layout.panes.len(), 2);
    assert_eq!(leaves(&app), [right, left]);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
    assert!(app.state.session_dirty);
    assert_eq!(
        events_after(&app, 0),
        [EventData::LayoutUpdated {
            layout: swap.layout
        }]
    );
}

#[test]
fn swapping_an_unfocused_source_updates_the_last_pane_history() {
    let (mut app, source, focused) = pair();
    let target = split_pane(&mut app, 0, focused, SplitDirection::Down);
    app.state.topology.workspaces[0].tabs[0]
        .layout
        .focus_pane(focused);
    assert!(swap_named(&mut app, "w1:p1", "w1:p3").changed);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(source)
    );
    assert_eq!(leaves(&app), [target, focused, source]);
    app.state.last_pane();
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(focused)
    );
}

#[test]
fn a_directional_swap_exchanges_the_pane_with_its_neighbour() {
    let (mut app, left, right) = pair();
    let wire = json!({"type": "pane.swap", "pane_id": "w1:p1", "direction": "right"});
    let swap = swap_of(request(&mut app, wire));
    assert!(swap.changed);
    assert_eq!(swap.source_pane_id, "w1:p1");
    assert_eq!(swap.target_pane_id.as_deref(), Some("w1:p2"));
    assert_eq!(leaves(&app), [right, left]);
    let wire = json!({"type": "pane.swap", "direction": "left"});
    let swap = swap_of(request(&mut app, wire));
    assert!(swap.changed, "the focused pane is the source by default");
    assert_eq!(leaves(&app), [left, right]);
    assert_eq!(event_names(&app, 0), ["layout_updated", "layout_updated"]);
}

#[test]
fn a_directional_swap_without_a_neighbour_changes_nothing() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let wire = json!({"type": "pane.swap", "pane_id": "w1:p1", "direction": "left"});
    let swap = swap_of(request(&mut app, wire));
    assert!(!swap.changed);
    assert_eq!(swap.reason, Some(PaneSwapReason::NoNeighbor));
    assert_eq!(swap.source_pane_id, "w1:p1");
    assert_eq!(swap.target_pane_id, None);
    assert_eq!(swap.layout.panes.len(), 1);
    assert!(events_after(&app, 0).is_empty());
    assert!(!app.state.session_dirty);
}

#[test]
fn a_swap_with_a_missing_pane_is_a_not_found_no_op() {
    let (mut app, _, _) = pair();
    let swap = swap_named(&mut app, "w1:p1", "missing-pane");
    assert!(!swap.changed);
    assert_eq!(swap.reason, Some(PaneSwapReason::NotFound));
    assert_eq!(swap.source_pane_id, "w1:p1");
    assert_eq!(swap.target_pane_id.as_deref(), Some("missing-pane"));
    assert_eq!(swap.layout.panes.len(), 2);
    let swap = swap_named(&mut app, "missing-pane", "w1:p2");
    assert_eq!(swap.reason, Some(PaneSwapReason::NotFound));
    assert_eq!(swap.source_pane_id, "missing-pane");
    assert_eq!(swap.target_pane_id.as_deref(), Some("w1:p2"));
    assert_eq!(swap.layout.workspace_id, "w1");
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn a_swap_outside_one_tab_is_a_cross_tab_no_op() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]);
    add_tab(&mut app, 0);
    for (target, tab) in [("w2:p1", "w1:t1"), ("w1:p2", "w1:t1")] {
        let swap = swap_named(&mut app, "w1:p1", target);
        assert!(!swap.changed, "{target}");
        assert_eq!(swap.reason, Some(PaneSwapReason::CrossTab), "{target}");
        assert_eq!(swap.source_pane_id, "w1:p1");
        assert_eq!(swap.target_pane_id.as_deref(), Some(target));
        assert_eq!(
            (
                swap.layout.workspace_id.as_str(),
                swap.layout.tab_id.as_str()
            ),
            ("w1", tab)
        );
    }
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn swapping_a_pane_with_itself_is_a_no_op() {
    let (mut app, left, right) = pair();
    let swap = swap_named(&mut app, "w1:p1", "w1-1");
    assert!(!swap.changed);
    assert_eq!(swap.reason, Some(PaneSwapReason::SamePane));
    assert_eq!(leaves(&app), [left, right]);
}

#[test]
fn a_swap_names_either_a_direction_or_both_ends() {
    let (mut app, _, _) = pair();
    let invalid = |message: &str| failure("invalid_pane_swap", message);
    let both =
        "provide either direction with optional pane_id, or source_pane_id and target_pane_id";
    for (wire, expected) in [
        (json!({"type": "pane.swap"}), both),
        (
            json!({"type": "pane.swap", "direction": "left", "source_pane_id": "w1:p1"}),
            both,
        ),
        (
            json!({"type": "pane.swap", "source_pane_id": "w1:p1"}),
            "missing target_pane_id",
        ),
        (
            json!({"type": "pane.swap", "target_pane_id": "w1:p1"}),
            "missing source_pane_id",
        ),
    ] {
        assert_eq!(request(&mut app, wire.clone()), invalid(expected), "{wire}");
    }
    let wire = json!({"type": "pane.swap", "pane_id": "w1:p9", "direction": "left"});
    assert_eq!(
        request(&mut app, wire),
        failure("pane_not_found", "source pane not found")
    );
}

#[test]
fn a_swap_of_unknown_panes_needs_a_tab_to_describe() {
    let (mut app, _, _) = pair();
    app.state.active = None;
    let wire: Value = json!({"type": "pane.swap", "source_pane_id": "a", "target_pane_id": "b"});
    assert_eq!(
        request(&mut app, wire),
        failure("pane_layout_unavailable", "pane layout unavailable")
    );
}
