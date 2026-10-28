//! Pane move through the daemon's handler: destinations, identity, focus, events and refusals.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, add_tab, daemon_with_workspaces, event_names, events_after, failure, join_group,
    request, root_pane, split_pane, workspace_ids,
};
use maestro_daemon::app::state::{CopyModeState, Mode};
use maestro_daemon::app::terminal_targets::TerminalTargetError;
use maestro_daemon::events::AppEvent;
use maestro_detection::agents::Agent;
use maestro_protocol::events::EventData;
use maestro_protocol::schema::{
    ErrorBody, PaneMoveReason, PaneMoveResult, ResponseResult, SplitDirection,
};
use serde_json::{Value, json};

/// The move record a move result holds.
fn move_of(result: Result<ResponseResult, ErrorBody>) -> PaneMoveResult {
    match result {
        Ok(ResponseResult::PaneMove { move_result }) => move_result,
        other => panic!("expected a move result: {other:?}"),
    }
}

/// The wire form of moving `pane` to `destination`.
fn moving(pane: &str, destination: &Value, focus: bool) -> Value {
    json!({"type": "pane.move", "pane_id": pane, "destination": destination, "focus": focus})
}

/// The wire form of a destination next to `target` in `tab`.
fn into_tab(tab: &str, target: Option<&str>, split: &str) -> Value {
    json!({"type": "tab", "tab_id": tab, "target_pane_id": target, "split": split})
}

/// One workspace with a first tab of a single pane and a second tab of a single pane.
fn two_tabs() -> TestApp {
    let mut app = daemon_with_workspaces(&["/issue"]);
    add_tab(&mut app, 0);
    app
}

/// The first tab's only pane moved beside the second tab's pane at ratio 0.25, with focus,
/// together with the terminal identity the pane had before.
fn into_the_second_tab() -> (TestApp, PaneMoveResult, String) {
    let mut app = two_tabs();
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(root_pane(&app, 0))
        .unwrap()
        .to_string();
    let mut destination = into_tab("w1:t2", Some("w1:p2"), "right");
    destination["ratio"] = json!(0.25);
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)));
    (app, moved, terminal)
}

#[test]
fn moving_a_pane_to_another_tab_keeps_its_terminal_and_closes_the_emptied_tab() {
    let (app, moved, terminal) = into_the_second_tab();
    assert!(moved.changed);
    assert_eq!(moved.reason, None);
    assert_eq!(moved.previous_pane_id, "w1:p1");
    assert_eq!(moved.previous_tab_id, "w1:t1");
    assert_eq!(moved.previous_workspace_id, "w1");
    assert_eq!(moved.pane.pane_id, "w1:p1");
    assert_eq!(moved.pane.tab_id, "w1:t2");
    assert_eq!(moved.pane.terminal_id, terminal);
    assert_eq!(moved.closed_tab_id.as_deref(), Some("w1:t1"));
    assert_eq!(moved.closed_workspace_id, None);
    assert_eq!((moved.created_tab, moved.created_workspace), (None, None));
    assert_eq!(moved.source_layout, None);
    let workspace = &app.state.topology.workspaces[0];
    let source = root_pane(&app, 0);
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(
        workspace.terminal_id(source).map(ToString::to_string),
        Some(terminal)
    );
}

#[test]
fn a_focused_move_splits_at_the_ratio_and_focuses_the_moved_pane() {
    let (app, moved, _) = into_the_second_tab();
    assert_eq!(moved.target_layout.panes.len(), 2);
    assert!((moved.target_layout.splits[0].ratio - 0.25).abs() < f32::EPSILON);
    assert_eq!(moved.focused_pane_id, "w1:p1");
    let source = root_pane(&app, 0);
    assert_eq!(
        app.state.topology.workspaces[0].tabs[0].layout.focused(),
        source
    );
    assert!(app.state.session_dirty);
}

#[test]
fn a_move_to_another_tab_publishes_the_closed_tab_the_move_and_the_target_layout() {
    let (app, moved, _) = into_the_second_tab();
    assert_eq!(
        events_after(&app, 0),
        [
            EventData::TabClosed {
                tab_id: "w1:t1".into(),
                workspace_id: "w1".into(),
            },
            EventData::PaneMoved {
                previous_pane_id: "w1:p1".into(),
                previous_workspace_id: "w1".into(),
                previous_tab_id: "w1:t1".into(),
                pane: moved.pane,
                created_workspace: None,
                created_tab: None,
                closed_workspace_id: None,
                closed_tab_id: Some("w1:t1".into()),
            },
            EventData::LayoutUpdated {
                layout: *moved.target_layout
            },
        ]
    );
}

#[test]
fn a_focused_move_returns_copy_mode_to_the_moved_pane() {
    let mut app = two_tabs();
    let source = root_pane(&app, 0);
    app.state.copy_mode = Some(CopyModeState {
        pane_id: source,
        selection: None,
    });
    let destination = into_tab("w1:t2", Some("w1:p2"), "right");
    assert!(move_of(request(&mut app, moving("w1:p1", &destination, true))).changed);
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

/// An agent pane alone in the first of two workspaces, moved beside the second's pane
/// without focus, with the terminal identity it had before.
fn across_workspaces() -> (TestApp, PaneMoveResult, String) {
    let mut app = daemon_with_workspaces(&["/issue", "/other"]);
    let source = root_pane(&app, 0);
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(source)
        .unwrap()
        .to_string();
    app.state.handle_app_event(AppEvent::AgentProcessDetected {
        pane_id: source,
        agent: Agent::Pi,
        observed_at: std::time::Instant::now(),
    });
    let destination = into_tab("w2:t1", Some("w2:p1"), "down");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, false)));
    (app, moved, terminal)
}

#[test]
fn moving_a_pane_across_workspaces_gives_it_the_target_workspaces_handle() {
    let (app, moved, terminal) = across_workspaces();
    assert!(moved.changed);
    assert_eq!(moved.previous_pane_id, "w1:p1");
    assert_eq!(moved.previous_workspace_id, "w1");
    assert_eq!(moved.closed_workspace_id.as_deref(), Some("w1"));
    assert_ne!(moved.pane.pane_id, moved.previous_pane_id);
    assert!(moved.pane.pane_id.starts_with("w2:p"));
    assert_eq!(moved.pane.workspace_id, "w2");
    assert_eq!(moved.pane.tab_id, "w2:t1");
    assert_eq!(moved.pane.terminal_id, terminal);
    assert_eq!(workspace_ids(&app), ["w2"]);
    assert_eq!(app.state.active, Some(0));
}

#[test]
fn a_moved_pane_answers_to_its_old_handle_but_is_only_an_agent_target_by_its_new_one() {
    let (app, moved, _) = across_workspaces();
    let current = app.state.parse_pane_id(&moved.pane.pane_id);
    assert!(current.is_some());
    assert_eq!(app.state.parse_pane_id("w1:p1"), current);
    assert!(matches!(
        app.resolve_agent_target("w1:p1"),
        Err(TerminalTargetError::NotFound { .. })
    ));
    assert!(app.resolve_agent_target(&moved.pane.pane_id).is_ok());
}

#[test]
fn a_legacy_tab_spelling_survives_the_removal_of_the_source_workspace() {
    let mut app = daemon_with_workspaces(&["/issue", "/other"]);
    join_group(&mut app, 0, "repo-key", true);
    let destination = into_tab("t_2_1", Some("w2:p1"), "right");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)));
    assert!(moved.changed);
    assert_eq!(moved.closed_workspace_id.as_deref(), Some("w1"));
    assert_eq!(
        (moved.pane.workspace_id.as_str(), moved.pane.tab_id.as_str()),
        ("w2", "w2:t1")
    );
    assert_eq!(workspace_ids(&app), ["w2"]);
    assert_eq!(app.state.active, Some(0));
    assert_eq!(app.state.selected, 0);
}

#[test]
fn moving_a_pane_to_a_new_tab_wraps_the_same_terminal() {
    let mut app = daemon_with_workspaces(&["/issue"]);
    let source = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, source, SplitDirection::Right);
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(source)
        .unwrap()
        .to_string();
    let destination = json!({"type": "new_tab", "label": "moved"});
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)));
    assert!(moved.changed);
    let tab = moved.created_tab.clone().unwrap();
    assert_eq!((tab.label.as_str(), tab.focused), ("moved", true));
    assert_eq!(moved.closed_tab_id, None);
    assert_eq!(moved.pane.pane_id, "w1:p1");
    assert_eq!(moved.pane.terminal_id, terminal);
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.tabs.len(), 2);
    assert!(workspace.tabs[0].terminal_id(right).is_some());
    assert_eq!(
        workspace.tabs[1]
            .terminal_id(source)
            .map(ToString::to_string),
        Some(terminal)
    );
    assert_eq!(
        event_names(&app, 0),
        [
            "tab_created",
            "pane_moved",
            "layout_updated",
            "layout_updated"
        ]
    );
    let events = events_after(&app, 0);
    assert!(matches!(&events[0], EventData::TabCreated { tab } if tab.focused));
    assert_eq!(
        events[2],
        EventData::LayoutUpdated {
            layout: *moved.source_layout.unwrap()
        }
    );
    assert_eq!(
        events[3],
        EventData::LayoutUpdated {
            layout: *moved.target_layout
        }
    );
}

#[test]
fn moving_the_only_pane_to_a_new_tab_keeps_its_workspace() {
    let mut app = daemon_with_workspaces(&["/issue"]);
    let destination = json!({"type": "new_tab", "workspace_id": "w1", "label": "again"});
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)));
    assert!(moved.changed);
    assert_eq!(moved.closed_tab_id.as_deref(), Some("w1:t1"));
    assert_eq!(moved.closed_workspace_id, None);
    assert_eq!(moved.source_layout, None);
    let tab = moved.created_tab.unwrap();
    assert_eq!(
        (tab.tab_id.as_str(), tab.label.as_str()),
        ("w1:t2", "again")
    );
    assert_eq!(workspace_ids(&app), ["w1"]);
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(workspace.active_tab, 0);
    assert_eq!(
        app.state.public_pane_id(0, root_pane(&app, 0)).as_deref(),
        Some("w1:p1")
    );
}

#[test]
fn moving_a_pane_to_a_new_tab_of_another_workspace_renumbers_it_there() {
    let mut app = daemon_with_workspaces(&["/issue", "/other"]);
    let source = root_pane(&app, 0);
    split_pane(&mut app, 0, source, SplitDirection::Right);
    let destination = json!({"type": "new_tab", "workspace_id": "w2"});
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, false)));
    assert!(moved.changed);
    assert_eq!(moved.closed_workspace_id, None);
    assert_eq!(moved.closed_tab_id, None);
    assert_eq!(moved.pane.workspace_id, "w2");
    assert_ne!(moved.pane.pane_id, "w1:p1");
    assert_eq!(moved.created_tab.unwrap().workspace_id, "w2");
    assert_eq!(app.state.parse_pane_id("w1:p1"), Some((1, source)));
    assert_eq!(app.state.active, Some(0));
}

/// The only pane of the only workspace promoted to a focused new workspace, with the
/// terminal identity the pane had before.
fn promoted() -> (TestApp, PaneMoveResult, String) {
    let mut app = daemon_with_workspaces(&["/issue"]);
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(root_pane(&app, 0))
        .unwrap()
        .to_string();
    let destination = json!({"type": "new_workspace", "label": "promoted", "tab_label": "main"});
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)));
    (app, moved, terminal)
}

#[test]
fn moving_the_only_pane_to_a_new_workspace_closes_the_emptied_one() {
    let (app, moved, terminal) = promoted();
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
fn a_promoted_pane_announces_the_closure_the_creations_and_the_move_in_order() {
    let (app, moved, _) = promoted();
    assert_eq!(
        event_names(&app, 0),
        [
            "tab_closed",
            "workspace_closed",
            "workspace_created",
            "tab_created",
            "pane_moved",
            "layout_updated",
        ]
    );
    let events = events_after(&app, 0);
    assert_eq!(
        events[1],
        EventData::WorkspaceClosed {
            workspace_id: "w1".into(),
            workspace: None,
        }
    );
    assert!(matches!(&events[2], EventData::WorkspaceCreated { workspace } if workspace.focused));
    assert!(matches!(&events[3], EventData::TabCreated { tab } if tab.focused));
    assert_eq!(
        events[5],
        EventData::LayoutUpdated {
            layout: *moved.target_layout
        }
    );
}

#[test]
fn moving_into_the_same_tab_is_a_no_op() {
    let mut app = daemon_with_workspaces(&["/issue"]);
    let destination = into_tab("w1:t1", None, "right");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)));
    assert!(!moved.changed);
    assert_eq!(moved.reason, Some(PaneMoveReason::SameTab));
    assert_eq!(moved.closed_tab_id, None);
    assert_eq!(moved.pane.pane_id, "w1:p1");
    assert_eq!(moved.source_layout.as_deref(), Some(&*moved.target_layout));
    assert_eq!(app.state.topology.workspaces[0].tabs.len(), 1);
    assert!(events_after(&app, 0).is_empty());
    assert!(!app.state.session_dirty);
}

#[test]
fn a_zoomed_tab_refuses_a_move_and_names_both_layouts() {
    let mut app = two_tabs();
    app.state.topology.workspaces[0].tabs[1].zoomed = true;
    let destination = into_tab("w1:t2", Some("w1:p2"), "right");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true)));
    assert!(!moved.changed);
    assert_eq!(moved.reason, Some(PaneMoveReason::ZoomedTab));
    assert_eq!(moved.target_layout.tab_id, "w1:t2");
    assert_eq!(
        moved.source_layout.map(|layout| layout.tab_id),
        Some("w1:t1".into())
    );
    assert_eq!(app.state.topology.workspaces[0].tabs.len(), 2);
    app.state.topology.workspaces[0].tabs[1].zoomed = false;
    let root = root_pane(&app, 0);
    let split = split_pane(&mut app, 0, root, SplitDirection::Right);
    app.state.topology.workspaces[0].tabs[0].zoomed = true;
    let destination = into_tab("w1:t2", None, "right");
    let moved = move_of(request(&mut app, moving("w1:p3", &destination, true)));
    assert_eq!(moved.reason, Some(PaneMoveReason::ZoomedTab));
    assert_eq!(moved.target_layout.tab_id, "w1:t1");
    assert_eq!(moved.source_layout.as_deref(), Some(&*moved.target_layout));
    assert_eq!(
        app.state.topology.workspaces[0].tabs[0].layout.pane_count(),
        2
    );
    assert!(
        app.state.topology.workspaces[0].tabs[0]
            .layout
            .pane_ids()
            .contains(&split)
    );
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn moving_next_to_a_pane_outside_the_target_tab_is_refused() {
    let mut app = two_tabs();
    add_tab(&mut app, 0);
    let wrong = into_tab("w1:t2", Some("w1:p3"), "right");
    assert_eq!(
        request(&mut app, moving("w1:p1", &wrong, true)),
        failure(
            "target_pane_not_found",
            "target pane w1:p3 is not in tab w1:t2"
        )
    );
    let missing = into_tab("w1:t2", Some("w1:p9"), "right");
    assert_eq!(
        request(&mut app, moving("w1:p1", &missing, true)),
        failure("target_pane_not_found", "target pane w1:p9 not found")
    );
    assert_eq!(app.state.topology.workspaces[0].tabs.len(), 3);
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn a_move_without_focus_keeps_the_focus_of_the_target_tab() {
    let mut app = two_tabs();
    let previously_focused = app.state.topology.workspaces[0].tabs[1].root_pane;
    app.state.topology.workspaces[0].active_tab = 1;
    let explicit = split_pane(&mut app, 0, previously_focused, SplitDirection::Right);
    let destination = into_tab("w1:t2", Some("w1:p3"), "right");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, false)));
    assert!(moved.changed);
    assert_eq!(moved.focused_pane_id, "w1:p2");
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.tabs[0].layout.focused(), previously_focused);
    assert!(workspace.tabs[0].layout.pane_ids().contains(&explicit));
}

#[test]
fn a_move_refuses_unknown_sources_and_destinations_without_a_change() {
    let mut app = two_tabs();
    for (wire, expected) in [
        (
            moving("w1:p9", &into_tab("w1:t2", None, "right"), true),
            failure("pane_not_found", "source pane not found"),
        ),
        (
            moving("w1:p1", &into_tab("w1:t9", None, "right"), true),
            failure("tab_not_found", "tab w1:t9 not found"),
        ),
        (
            moving(
                "w1:p1",
                &json!({"type": "new_tab", "workspace_id": "w9"}),
                true,
            ),
            failure("workspace_not_found", "workspace w9 not found"),
        ),
    ] {
        assert_eq!(request(&mut app, wire.clone()), expected, "{wire}");
    }
    assert_eq!(app.state.topology.workspaces[0].tabs.len(), 2);
    assert!(events_after(&app, 0).is_empty());
}
