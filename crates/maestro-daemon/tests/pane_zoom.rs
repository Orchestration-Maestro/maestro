//! Pane zoom through the daemon's handler: modes, focus changes and no-ops.
#![cfg(target_os = "linux")]
use maestro_daemon::state::test_support::api::{
    daemon_with_workspaces, event_names, events_after, pair, request, root_pane,
};
use maestro_daemon::state::test_support::pane_zoom::{last_layout, zoom_of};
use maestro_protocol::schema::PaneZoomReason;
use serde_json::json;

#[test]
fn zooming_a_single_pane_changes_nothing() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let wire = json!({"type": "pane.zoom", "pane_id": "w1:p1", "mode": "toggle"});
    let zoom = zoom_of(request(&mut app, wire)).unwrap();
    assert!(!zoom.changed && !zoom.zoom_changed && !zoom.focus_changed);
    assert_eq!(zoom.reason, Some(PaneZoomReason::SinglePane));
    assert_eq!(zoom.pane_id, "w1:p1");
    assert!(!zoom.zoomed.0);
    assert!(!app.state.topology.workspaces[0].tabs[0].zoomed);
    assert!(events_after(&app, 0).is_empty());
}
#[test]
fn zoom_on_and_off_are_idempotent() {
    let (mut app, _, _) = pair().unwrap();
    for (mode, changed, reason, zoomed) in [
        ("on", true, None, true),
        ("on", false, Some(PaneZoomReason::AlreadyZoomed), true),
        ("off", true, None, false),
        ("off", false, Some(PaneZoomReason::AlreadyUnzoomed), false),
    ] {
        let wire = json!({"type": "pane.zoom", "pane_id": "w1:p1", "mode": mode});
        let zoom = zoom_of(request(&mut app, wire)).unwrap();
        assert_eq!(zoom.changed, changed, "{mode}");
        assert_eq!(zoom.zoom_changed, changed, "{mode}");
        assert!(!zoom.focus_changed, "{mode}");
        assert_eq!(zoom.reason, reason, "{mode}");
        assert_eq!(zoom.zoomed.0, zoomed, "{mode}");
    }
    assert_eq!(
        event_names(&app, 0).unwrap(),
        ["layout_updated", "layout_updated"]
    );
}
#[test]
fn an_idempotent_zoom_still_reports_the_focus_change() {
    let (mut app, _, right) = pair().unwrap();
    app.state.topology.workspaces[0].tabs[0].zoomed = true;
    let wire = json!({"type": "pane.zoom", "pane_id": "w1:p2", "mode": "on"});
    let zoom = zoom_of(request(&mut app, wire)).unwrap();
    assert!(zoom.changed && !zoom.zoom_changed && zoom.focus_changed);
    assert_eq!(zoom.reason, Some(PaneZoomReason::AlreadyZoomed));
    assert!(zoom.zoomed.0);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(right)
    );
    assert_eq!(last_layout(&app).unwrap().focused_pane_id, "w1:p2");
}
#[test]
fn each_seeded_workspace_names_its_first_tab_root() {
    let mut app = daemon_with_workspaces(&[]).unwrap();
    let seeded: Vec<_> = (0..20)
        .map(|index| app.seed_workspace(&format!("/w{index}")).unwrap())
        .collect();
    let found: Vec<_> = (0..20)
        .map(|index| root_pane(&app, index).unwrap())
        .collect();
    assert_eq!(found, seeded);
}
