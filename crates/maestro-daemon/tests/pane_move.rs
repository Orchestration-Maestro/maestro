//! Pane move through the daemon's handler: destinations, identity, focus, events and refusals.
#![cfg(target_os = "linux")]
use maestro_daemon::state::terminal_targets::TerminalTargetError;
use maestro_daemon::state::test_support::api::{
    add_tab, daemon_with_workspaces, event_names, events_after, failure, request, root_pane,
    split_pane, workspace_ids,
};
use maestro_daemon::state::test_support::pane_move::{
    across_workspaces, into_tab, into_the_second_tab, move_of, moving, promoted, two_tabs,
};
use maestro_protocol::events::EventData;
use maestro_protocol::schema::{PaneMoveReason, SplitDirection};
use serde_json::json;

#[test]
fn moving_a_pane_to_another_tab_keeps_its_terminal_and_closes_the_emptied_tab() {
    let (app, moved, terminal) = into_the_second_tab().unwrap();
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
    let (_, pane) = app.state.parse_pane_id(&moved.pane.pane_id).unwrap();
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(
        workspace.terminal_id(pane).map(ToString::to_string),
        Some(terminal)
    );
}
#[test]
fn a_move_to_another_tab_publishes_the_closed_tab_the_move_and_the_target_layout() {
    let (app, moved, _) = into_the_second_tab().unwrap();
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
fn a_moved_pane_answers_to_its_old_handle_but_is_only_an_agent_target_by_its_new_one() {
    let (app, moved, _) = across_workspaces().unwrap();
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
fn moving_a_pane_to_a_new_tab_wraps_the_same_terminal() {
    let mut app = daemon_with_workspaces(&["/issue"]).unwrap();
    let source = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, source, SplitDirection::Right).unwrap();
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(source)
        .unwrap()
        .to_string();
    let destination = json!({"type": "new_tab", "label": "moved"});
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true))).unwrap();
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
        event_names(&app, 0).unwrap(),
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
    let mut app = daemon_with_workspaces(&["/issue"]).unwrap();
    let destination = json!({"type": "new_tab", "workspace_id": "w1", "label": "again"});
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true))).unwrap();
    assert!(moved.changed);
    assert_eq!(moved.closed_tab_id.as_deref(), Some("w1:t1"));
    assert_eq!(moved.closed_workspace_id, None);
    assert_eq!(moved.source_layout, None);
    let tab = moved.created_tab.unwrap();
    assert_eq!(
        (tab.tab_id.as_str(), tab.label.as_str()),
        ("w1:t2", "again")
    );
    assert_eq!(workspace_ids(&mut app).unwrap(), ["w1"]);
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(workspace.active_tab, 0);
    let root = root_pane(&app, 0);
    assert_eq!(app.state.public_pane_id(0, root).as_deref(), Some("w1:p1"));
}
#[test]
fn a_promoted_pane_announces_the_closure_the_creations_and_the_move_in_order() {
    let (app, moved, _) = promoted().unwrap();
    assert_eq!(
        event_names(&app, 0).unwrap(),
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
fn a_zoomed_tab_refuses_a_move_and_names_both_layouts() {
    let mut app = two_tabs().unwrap();
    app.state.topology.workspaces[0].tabs[1].zoomed = true;
    let destination = into_tab("w1:t2", Some("w1:p2"), "right");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, true))).unwrap();
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
    let split = split_pane(&mut app, 0, root, SplitDirection::Right).unwrap();
    app.state.topology.workspaces[0].tabs[0].zoomed = true;
    let destination = into_tab("w1:t2", None, "right");
    let moved = move_of(request(&mut app, moving("w1:p3", &destination, true))).unwrap();
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
    let mut app = two_tabs().unwrap();
    add_tab(&mut app, 0).unwrap();
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
    let mut app = two_tabs().unwrap();
    let previously_focused = app.state.topology.workspaces[0].tabs[1].root_pane;
    app.state.topology.workspaces[0].active_tab = 1;
    let explicit = split_pane(&mut app, 0, previously_focused, SplitDirection::Right).unwrap();
    let destination = into_tab("w1:t2", Some("w1:p3"), "right");
    let moved = move_of(request(&mut app, moving("w1:p1", &destination, false))).unwrap();
    assert!(moved.changed);
    assert_eq!(moved.focused_pane_id, "w1:p2");
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.tabs[0].layout.focused(), previously_focused);
    assert!(workspace.tabs[0].layout.pane_ids().contains(&explicit));
}
#[test]
fn a_move_refuses_unknown_sources_and_destinations_without_a_change() {
    let mut app = two_tabs().unwrap();
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
