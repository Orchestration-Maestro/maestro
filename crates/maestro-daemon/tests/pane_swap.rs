//! Pane swap through the daemon's handler: explicit and directional swaps, no-ops and refusals.
#![cfg(target_os = "linux")]

use maestro_daemon::state::test_support::api::{
    add_tab, daemon_with_workspaces, event_names, events_after, failure, pair, request, split_pane,
};
use maestro_daemon::state::test_support::pane_swap::{leaves, swap_named, swap_of};
use maestro_protocol::schema::{PaneSwapReason, SplitDirection};
use serde_json::json;

#[test]
fn swapping_an_unfocused_source_updates_the_last_pane_history() {
    let (mut app, source, focused) = pair().unwrap();
    let target = split_pane(&mut app, 0, focused, SplitDirection::Down).unwrap();
    app.state.topology.workspaces[0].tabs[0]
        .layout
        .focus_pane(focused);
    assert!(swap_named(&mut app, "w1:p1", "w1:p3").unwrap().changed);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(source)
    );
    assert_eq!(leaves(&app).unwrap(), [target, focused, source]);
    app.state.last_pane();
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(focused)
    );
}
#[test]
fn a_directional_swap_exchanges_the_pane_with_its_neighbour() {
    let (mut app, left, right) = pair().unwrap();
    let wire = json!({"type": "pane.swap", "pane_id": "w1:p1", "direction": "right"});
    let swap = swap_of(request(&mut app, wire)).unwrap();
    assert!(swap.changed);
    assert_eq!(swap.source_pane_id, "w1:p1");
    assert_eq!(swap.target_pane_id.as_deref(), Some("w1:p2"));
    assert_eq!(leaves(&app).unwrap(), [right, left]);
    let wire = json!({"type": "pane.swap", "direction": "left"});
    let swap = swap_of(request(&mut app, wire)).unwrap();
    assert!(swap.changed, "the focused pane is the source by default");
    assert_eq!(leaves(&app).unwrap(), [left, right]);
    assert_eq!(
        event_names(&app, 0).unwrap(),
        ["layout_updated", "layout_updated"]
    );
}
#[test]
fn a_swap_with_a_missing_pane_is_a_not_found_no_op() {
    let (mut app, _, _) = pair().unwrap();
    let swap = swap_named(&mut app, "w1:p1", "missing-pane").unwrap();
    assert!(!swap.changed);
    assert_eq!(swap.reason, Some(PaneSwapReason::NotFound));
    assert_eq!(swap.source_pane_id, "w1:p1");
    assert_eq!(swap.target_pane_id.as_deref(), Some("missing-pane"));
    assert_eq!(swap.layout.panes.len(), 2);
    let swap = swap_named(&mut app, "missing-pane", "w1:p2").unwrap();
    assert_eq!(swap.reason, Some(PaneSwapReason::NotFound));
    assert_eq!(swap.source_pane_id, "missing-pane");
    assert_eq!(swap.target_pane_id.as_deref(), Some("w1:p2"));
    assert_eq!(swap.layout.workspace_id, "w1");
    assert!(events_after(&app, 0).is_empty());
}
#[test]
fn a_swap_outside_one_tab_is_a_cross_tab_no_op() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]).unwrap();
    add_tab(&mut app, 0).unwrap();
    for (target, tab) in [("w2:p1", "w1:t1"), ("w1:p2", "w1:t1")] {
        let swap = swap_named(&mut app, "w1:p1", target).unwrap();
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
    let (mut app, left, right) = pair().unwrap();
    let swap = swap_named(&mut app, "w1:p1", "w1-1").unwrap();
    assert!(!swap.changed);
    assert_eq!(swap.reason, Some(PaneSwapReason::SamePane));
    assert_eq!(leaves(&app).unwrap(), [left, right]);
}
#[test]
fn a_swap_names_either_a_direction_or_both_ends() {
    let (mut app, _, _) = pair().unwrap();
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
