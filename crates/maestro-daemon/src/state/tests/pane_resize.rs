//! Pane resize through the daemon's handler: which divider moves, by how much, and what is published.
use crate::state::test_support::api::{
    daemon_with_workspaces, events_after, focused_pane_id, request,
};
use crate::state::test_support::pane_resize::{pair, resize_of};
use maestro_protocol::events::EventData;
use maestro_protocol::schema::PaneResizeReason;
use serde_json::json;

#[test]
fn resizing_moves_the_nearest_divider_without_changing_focus() {
    let (mut app, _, _) = pair().unwrap();
    let wire =
        json!({"type": "pane.resize", "pane_id": "w1:p1", "direction": "right", "amount": 0.1});
    let resize = resize_of(request(&mut app, wire)).unwrap();
    assert!(resize.changed);
    assert_eq!(resize.reason, None);
    assert_eq!(resize.pane_id, "w1:p1");
    assert_eq!(resize.focused_pane_id, "w1:p2");
    assert_eq!(resize.layout.focused_pane_id, "w1:p2");
    assert!((resize.layout.splits[0].ratio - 0.6).abs() < f32::EPSILON);
    assert_eq!(focused_pane_id(&mut app, 0).unwrap(), "w1:p2");
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
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let wire = json!({"type": "pane.resize", "direction": "left"});
    let resize = resize_of(request(&mut app, wire)).unwrap();
    assert!(!resize.changed);
    assert_eq!(resize.reason, Some(PaneResizeReason::Unchanged));
    assert_eq!(resize.pane_id, "w1:p1");
    assert_eq!(resize.focused_pane_id, "w1:p1");
    assert!(!app.state.session_dirty);
    assert!(events_after(&app, 0).is_empty());
}
