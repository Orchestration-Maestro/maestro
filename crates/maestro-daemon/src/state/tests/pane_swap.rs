//! Pane swap through the daemon's handler: explicit and directional swaps, no-ops and refusals.

use crate::state::test_support::api::{
    daemon_with_workspaces, events_after, failure, pair, request,
};
use crate::state::test_support::pane_swap::{leaves, swap_named, swap_of};
use maestro_protocol::events::EventData;
use maestro_protocol::schema::PaneSwapReason;
use serde_json::{Value, json};

#[test]
fn swapping_two_named_panes_keeps_the_source_focused_and_returns_the_layout() {
    let (mut app, left, right) = pair().unwrap();
    let swap = swap_named(&mut app, "w1:p1", "w1:p2").unwrap();
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
fn a_directional_swap_without_a_neighbour_changes_nothing() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let wire = json!({"type": "pane.swap", "pane_id": "w1:p1", "direction": "left"});
    let swap = swap_of(request(&mut app, wire)).unwrap();
    assert!(!swap.changed);
    assert_eq!(swap.reason, Some(PaneSwapReason::NoNeighbor));
    assert_eq!(swap.source_pane_id, "w1:p1");
    assert_eq!(swap.target_pane_id, None);
    assert_eq!(swap.layout.panes.len(), 1);
    assert!(events_after(&app, 0).is_empty());
    assert!(!app.state.session_dirty);
}
#[test]
fn a_swap_of_unknown_panes_needs_a_tab_to_describe() {
    let (mut app, _, _) = pair().unwrap();
    app.state.active = None;
    let wire: Value = json!({"type": "pane.swap", "source_pane_id": "a", "target_pane_id": "b"});
    assert_eq!(
        request(&mut app, wire),
        failure("pane_layout_unavailable", "pane layout unavailable")
    );
}
