//! Pane zoom through the daemon's handler: modes, focus changes and no-ops.
use crate::state::model::{CopyModeState, Mode};
use crate::state::test_support::api::{
    daemon_with_workspaces, event_names, failure, pair, request, root_pane, split_pane,
};
use crate::state::test_support::pane_zoom::{last_layout, zoom_of};
use maestro_protocol::schema::SplitDirection;
use serde_json::json;

#[test]
fn zooming_toggles_the_focused_pane_and_publishes_the_layout() {
    let (mut app, left, _) = pair().unwrap();
    let zoom = zoom_of(request(&mut app, json!({"type": "pane.zoom"}))).unwrap();
    assert!(zoom.changed && zoom.zoom_changed && !zoom.focus_changed);
    assert_eq!(zoom.reason, None);
    assert_eq!(zoom.pane_id, "w1:p1");
    assert_eq!(zoom.focused_pane_id, zoom.pane_id);
    assert!(zoom.zoomed.0 && zoom.layout.zoomed);
    assert!(app.state.topology.workspaces[0].tabs[0].zoomed);
    assert_eq!(event_names(&app, 0).unwrap(), ["layout_updated"]);
    assert_eq!(last_layout(&app).unwrap(), zoom.layout);
    assert!(app.state.session_dirty);
    let zoom = zoom_of(request(&mut app, json!({"type": "pane.zoom"}))).unwrap();
    assert!(zoom.changed && zoom.zoom_changed && !zoom.focus_changed);
    assert!(!zoom.zoomed.0 && !zoom.layout.zoomed);
    assert!(!last_layout(&app).unwrap().zoomed);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
}
#[test]
fn zooming_a_background_pane_focuses_it_and_keeps_the_last_pane_history() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]).unwrap();
    let first = root_pane(&app, 0).unwrap();
    let target = root_pane(&app, 1).unwrap();
    split_pane(&mut app, 1, target, SplitDirection::Right).unwrap();
    assert_eq!(app.state.active, Some(0));
    let wire = json!({"type": "pane.zoom", "pane_id": "w2:p1", "mode": "on"});
    let zoom = zoom_of(request(&mut app, wire)).unwrap();
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
    let mut app = daemon_with_workspaces(&["/one", "/two"]).unwrap();
    let source = root_pane(&app, 0).unwrap();
    let other = split_pane(&mut app, 0, source, SplitDirection::Right).unwrap();
    app.state.topology.workspaces[0].tabs[0]
        .layout
        .focus_pane(other);
    let target = root_pane(&app, 1).unwrap();
    split_pane(&mut app, 1, target, SplitDirection::Right).unwrap();
    app.state.active = Some(1);
    app.state.selected = 1;
    app.state.mode = Mode::Terminal;
    app.state.copy_mode = Some(CopyModeState {
        pane_id: source,
        selection: None,
    });
    let wire = json!({"type": "pane.zoom", "pane_id": "w1:p1", "mode": "on"});
    assert!(zoom_of(request(&mut app, wire)).unwrap().focus_changed);
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
fn zooming_a_missing_pane_is_refused() {
    let (mut app, _, _) = pair().unwrap();
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
