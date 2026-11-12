//! Pane move through the daemon's handler: destinations, identity, focus, events and refusals.
use crate::state::model::{CopyModeState, Mode};
use crate::state::test_support::api::{
    daemon_with_workspaces, events_after, join_group, request, root_pane, split_pane, workspace_ids,
};
use crate::state::test_support::pane_move::{
    across_workspaces, into_tab, into_the_second_tab, move_of, moving, promoted, two_tabs,
};
use maestro_protocol::schema::{PaneMoveReason, SplitDirection};
use serde_json::json;

#[test]
fn a_focused_move_splits_at_the_ratio_and_focuses_the_moved_pane() {
    let (app, moved, _) = into_the_second_tab().unwrap();
    assert_eq!(moved.target_layout.panes.len(), 2);
    assert!((moved.target_layout.splits[0].ratio - 0.25).abs() < f32::EPSILON);
    assert_eq!(moved.focused_pane_id, "w1:p1");
    let (_, pane) = app.state.parse_pane_id(&moved.pane.pane_id).unwrap();
    assert_eq!(
        app.state.topology.workspaces[0].tabs[0].layout.focused(),
        pane
    );
    assert!(app.state.session_dirty);
}
#[test]
fn a_focused_move_returns_copy_mode_to_the_moved_pane() {
    let mut app = two_tabs().unwrap();
    let source = root_pane(&app, 0);
    app.state.copy_mode = Some(CopyModeState {
        pane_id: source,
        selection: None,
    });
    let destination = into_tab("w1:t2", Some("w1:p2"), "right");
    assert!(
        move_of(request(&mut app, moving("w1:p1", &destination, true)))
            .unwrap()
            .changed
    );
    assert_eq!(app.state.mode, Mode::Copy);
    assert_eq!(
        app.state.copy_mode.as_ref().map(|copy| copy.pane_id),
        Some(source)
    );
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(source)
    );
}
#[test]
fn moving_a_pane_across_workspaces_gives_it_the_target_workspaces_handle() {
    let (mut app, moved, terminal) = across_workspaces().unwrap();
    assert!(moved.changed);
    assert_eq!(moved.previous_pane_id, "w1:p1");
    assert_eq!(moved.previous_workspace_id, "w1");
    assert_eq!(moved.closed_workspace_id.as_deref(), Some("w1"));
    assert_ne!(moved.pane.pane_id, moved.previous_pane_id);
    assert!(moved.pane.pane_id.starts_with("w2:p"));
    assert_eq!(moved.pane.workspace_id, "w2");
    assert_eq!(moved.pane.tab_id, "w2:t1");
    assert_eq!(moved.pane.terminal_id, terminal);
    assert_eq!(workspace_ids(&mut app).unwrap(), ["w2"]);
    assert_eq!(app.state.active, Some(0));
}
#[test]
fn a_legacy_tab_spelling_survives_the_removal_of_the_source_workspace() {
    let mut app = daemon_with_workspaces(&["/issue", "/other"]).unwrap();
    join_group(&mut app, 0, "repo-key", true);
    let destination = into_tab("t_2_1", Some("w2:p1"), "right");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true))).unwrap();
    assert!(moved.changed);
    assert_eq!(moved.closed_workspace_id.as_deref(), Some("w1"));
    assert_eq!(
        (moved.pane.workspace_id.as_str(), moved.pane.tab_id.as_str()),
        ("w2", "w2:t1")
    );
    assert_eq!(workspace_ids(&mut app).unwrap(), ["w2"]);
    assert_eq!(app.state.active, Some(0));
    assert_eq!(app.state.selected, 0);
}
#[test]
fn moving_a_pane_to_a_new_tab_of_another_workspace_renumbers_it_there() {
    let mut app = daemon_with_workspaces(&["/issue", "/other"]).unwrap();
    let source = root_pane(&app, 0);
    split_pane(&mut app, 0, source, SplitDirection::Right).unwrap();
    let destination = json!({"type": "new_tab", "workspace_id": "w2"});
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, false))).unwrap();
    assert!(moved.changed);
    assert_eq!(moved.closed_workspace_id, None);
    assert_eq!(moved.closed_tab_id, None);
    assert_eq!(moved.pane.workspace_id, "w2");
    assert_ne!(moved.pane.pane_id, "w1:p1");
    assert_eq!(moved.created_tab.unwrap().workspace_id, "w2");
    assert_eq!(app.state.parse_pane_id("w1:p1"), Some((1, source)));
    assert_eq!(app.state.active, Some(0));
}
#[test]
fn moving_the_only_pane_to_a_new_workspace_closes_the_emptied_one() {
    let (app, moved, terminal) = promoted().unwrap();
    assert!(moved.changed);
    assert_eq!(moved.closed_workspace_id.as_deref(), Some("w1"));
    assert_eq!(moved.closed_tab_id.as_deref(), Some("w1:t1"));
    let workspace = moved.created_workspace.clone().unwrap();
    assert_eq!(
        (workspace.label.as_str(), workspace.focused),
        ("promoted", true)
    );
    let tab = moved.created_tab.clone().unwrap();
    assert_eq!((tab.label.as_str(), tab.focused), ("main", true));
    assert_ne!(moved.pane.pane_id, "w1:p1");
    assert_eq!(moved.pane.terminal_id, terminal);
    assert_eq!(app.state.topology.workspaces.len(), 1);
    assert_eq!(app.state.topology.workspaces[0].id, workspace.workspace_id);
    assert_eq!(app.state.active, Some(0));
}
#[test]
fn moving_into_the_same_tab_is_a_no_op() {
    let mut app = daemon_with_workspaces(&["/issue"]).unwrap();
    let destination = into_tab("w1:t1", None, "right");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true))).unwrap();
    assert!(!moved.changed);
    assert_eq!(moved.reason, Some(PaneMoveReason::SameTab));
    assert_eq!(moved.closed_tab_id, None);
    assert_eq!(moved.pane.pane_id, "w1:p1");
    assert_eq!(moved.source_layout.as_deref(), Some(&*moved.target_layout));
    assert_eq!(app.state.topology.workspaces[0].tabs.len(), 1);
    assert!(events_after(&app, 0).is_empty());
    assert!(!app.state.session_dirty);
}
