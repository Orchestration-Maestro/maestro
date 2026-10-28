//! Pane topology commands through the daemon's handler: reads, focus and neighbours.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, add_tab, add_workspaces, answer, daemon_with_workspaces, failure, request, root_pane,
    split_pane, with_live_daemon,
};
use maestro_daemon::app::state::Mode;
use maestro_detection::agents::AgentState;
use maestro_protocol::schema::{
    AgentStatus, ErrorBody, PaneDirection, PaneFocusDirectionReason, PaneInfo, PaneLayoutSnapshot,
    ResponseResult, SplitDirection,
};
use maestro_workspace::layout::PaneId;
use serde_json::{Value, json};

/// The pane a single-pane result reports.
fn reported(result: Result<ResponseResult, ErrorBody>) -> PaneInfo {
    match result {
        Ok(ResponseResult::PaneInfo { pane } | ResponseResult::PaneCurrent { pane }) => pane,
        other => panic!("expected a pane: {other:?}"),
    }
}

/// The pane identities a list result holds, in order.
fn listed(result: Result<ResponseResult, ErrorBody>) -> Vec<String> {
    match result {
        Ok(ResponseResult::PaneList { panes }) => {
            panes.into_iter().map(|pane| pane.pane_id).collect()
        }
        other => panic!("expected a pane list: {other:?}"),
    }
}

/// The layout a layout result reports.
fn layout_of(result: Result<ResponseResult, ErrorBody>) -> PaneLayoutSnapshot {
    match result {
        Ok(ResponseResult::PaneLayout { layout }) => layout,
        other => panic!("expected a pane layout: {other:?}"),
    }
}

/// One workspace whose first tab holds a pane on the left and one on the right.
fn pair() -> (TestApp, PaneId, PaneId) {
    let mut app = daemon_with_workspaces(&["/one"]);
    let left = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, left, SplitDirection::Right);
    (app, left, right)
}

/// The public handle of a pane in the workspace at `workspace`.
fn public(app: &TestApp, workspace: usize, pane: PaneId) -> String {
    app.state.public_pane_id(workspace, pane).unwrap()
}

#[test]
fn pane_listing_follows_workspace_tab_and_split_order() {
    let (mut app, _, _) = pair();
    add_tab(&mut app, 0);
    add_workspaces(&mut app, &["/two"]);
    assert_eq!(
        listed(request(&mut app, json!({"type": "pane.list"}))),
        ["w1:p1", "w1:p2", "w1:p3", "w2:p1"]
    );
    for alias in ["w2", "w_2", "2"] {
        let list = json!({"type": "pane.list", "workspace_id": alias});
        assert_eq!(listed(request(&mut app, list)), ["w2:p1"], "{alias}");
    }
    let list = json!({"type": "pane.list", "workspace_id": "w9"});
    assert_eq!(
        request(&mut app, list),
        failure("workspace_not_found", "workspace w9 not found")
    );
}

#[test]
fn the_current_pane_prefers_the_caller_then_the_focus() {
    let (mut app, left, right) = pair();
    let focused = reported(request(&mut app, json!({"type": "pane.current"})));
    assert_eq!(focused.pane_id, public(&app, 0, left));
    assert!(focused.focused);
    let caller = json!({"type": "pane.current", "caller_pane_id": "w1:p2"});
    let named = reported(request(&mut app, caller));
    assert_eq!(named.pane_id, public(&app, 0, right));
    assert!(!named.focused);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
}

#[test]
fn the_current_pane_is_refused_without_a_caller_or_an_active_pane() {
    let (mut app, _, _) = pair();
    let missing = json!({"type": "pane.current", "caller_pane_id": "missing"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane not found")
    );
    app.state.active = None;
    assert_eq!(
        request(&mut app, json!({"type": "pane.current"})),
        failure("pane_not_found", "pane not found")
    );
}

#[test]
fn a_pane_is_read_by_any_accepted_spelling_and_a_missing_one_is_refused() {
    let (mut app, _, right) = pair();
    for alias in ["w1:p2", "w1-2"] {
        let get = json!({"type": "pane.get", "pane_id": alias});
        assert_eq!(
            reported(request(&mut app, get)).pane_id,
            public(&app, 0, right)
        );
    }
    let get = json!({"type": "pane.get", "pane_id": "w1:p9"});
    assert_eq!(
        request(&mut app, get),
        failure("pane_not_found", "pane w1:p9 not found")
    );
}

#[test]
fn focusing_a_pane_activates_its_tab_and_workspace() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]);
    let target = add_tab(&mut app, 1);
    let target_id = public(&app, 1, target);
    assert_eq!(app.state.active, Some(0));
    let focus = json!({"type": "pane.focus", "pane_id": target_id});
    let pane = reported(request(&mut app, focus));
    assert_eq!(pane.pane_id, target_id);
    assert!(pane.focused);
    assert_eq!(app.state.active, Some(1));
    assert_eq!(app.state.topology.workspaces[1].active_tab, 1);
    assert_eq!(
        app.state.topology.workspaces[1].focused_pane_id(),
        Some(target)
    );
    assert_eq!(app.state.mode, Mode::Terminal);
}

#[test]
fn focusing_the_focused_pane_marks_a_finished_agent_seen() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let pane = root_pane(&app, 0);
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.state.terminals.get_mut(&terminal).unwrap().state = AgentState::Idle;
    app.state.topology.workspaces[0]
        .pane_state_mut(pane)
        .unwrap()
        .seen = false;
    let get = json!({"type": "pane.get", "pane_id": "w1:p1"});
    assert_eq!(
        reported(request(&mut app, get)).agent_status,
        AgentStatus::Done
    );
    let focus = json!({"type": "pane.focus", "pane_id": "w1:p1"});
    assert_eq!(
        reported(request(&mut app, focus)).agent_status,
        AgentStatus::Idle
    );
}

#[test]
fn focusing_a_missing_pane_is_refused_without_a_change() {
    let (mut app, left, _) = pair();
    let focus = json!({"type": "pane.focus", "pane_id": "pane_missing"});
    assert_eq!(
        request(&mut app, focus),
        failure("pane_not_found", "pane pane_missing not found")
    );
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
}

#[test]
fn a_layout_describes_the_tab_of_the_named_or_focused_pane() {
    let (mut app, _, _) = pair();
    add_tab(&mut app, 0);
    let expected = app.pane_layout_snapshot(0, 0).unwrap();
    let named = json!({"type": "pane.layout", "pane_id": "w1:p2"});
    assert_eq!(layout_of(request(&mut app, named)), expected);
    assert_eq!(
        layout_of(request(&mut app, json!({"type": "pane.layout"}))),
        expected
    );
    let other = json!({"type": "pane.layout", "pane_id": "w1:p3"});
    assert_eq!(
        layout_of(request(&mut app, other)),
        app.pane_layout_snapshot(0, 1).unwrap()
    );
    let missing = json!({"type": "pane.layout", "pane_id": "w1:p9"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane not found")
    );
}

/// The neighbour record a neighbour result holds.
fn neighbour_of(app: &mut TestApp, pane: &str, direction: &str) -> Option<String> {
    let command = json!({"type": "pane.neighbor", "pane_id": pane, "direction": direction});
    let Ok(ResponseResult::PaneNeighbor { neighbor }) = request(app, command) else {
        panic!("expected a neighbour");
    };
    assert_eq!(neighbor.pane_id, pane);
    assert_eq!(neighbor.layout, app.pane_layout_snapshot(0, 0).unwrap());
    neighbor.neighbor_pane_id
}

#[test]
fn a_neighbour_is_the_pane_next_to_it_in_a_direction() {
    let (mut app, _, _) = pair();
    assert_eq!(
        neighbour_of(&mut app, "w1:p1", "right").as_deref(),
        Some("w1:p2")
    );
    assert_eq!(
        neighbour_of(&mut app, "w1:p2", "left").as_deref(),
        Some("w1:p1")
    );
    assert_eq!(neighbour_of(&mut app, "w1:p1", "left"), None);
    assert_eq!(neighbour_of(&mut app, "w1:p1", "up"), None);
}

#[test]
fn edges_tell_which_sides_of_the_view_a_pane_touches() {
    let (mut app, _, _) = pair();
    for (pane, left, right) in [("w1:p1", true, false), ("w1:p2", false, true)] {
        let command = json!({"type": "pane.edges", "pane_id": pane});
        let Ok(ResponseResult::PaneEdges { edges }) = request(&mut app, command) else {
            panic!("expected edges");
        };
        assert_eq!(edges.pane_id, pane);
        assert_eq!((edges.left, edges.right), (left, right), "{pane}");
        assert!(edges.up && edges.down.0, "{pane}");
        assert_eq!(edges.layout, app.pane_layout_snapshot(0, 0).unwrap());
    }
    let missing = json!({"type": "pane.edges", "pane_id": "w1:p9"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane not found")
    );
}

/// The result of moving focus from `pane` toward `direction`.
fn focus_toward(app: &mut TestApp, pane: &str, direction: PaneDirection) -> Value {
    let command = json!({"type": "pane.focus_direction", "pane_id": pane, "direction": direction});
    let Ok(ResponseResult::PaneFocusDirection { focus }) = request(app, command) else {
        panic!("expected a focus result");
    };
    serde_json::to_value(focus).unwrap()
}

#[test]
fn focus_moves_to_the_neighbour_in_a_direction() {
    let (mut app, _, right) = pair();
    let focus = focus_toward(&mut app, "w1:p1", PaneDirection::Right);
    assert_eq!(focus["changed"], true);
    assert_eq!(focus.get("reason"), None);
    assert_eq!(focus["source_pane_id"], "w1:p1");
    assert_eq!(focus["focused_pane_id"], "w1:p2");
    assert_eq!(focus["layout"]["focused_pane_id"], "w1:p2");
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(right)
    );
    assert_eq!(app.state.mode, Mode::Terminal);
}

#[test]
fn focus_stays_put_when_no_neighbour_lies_that_way() {
    let (mut app, left, _) = pair();
    let focus = focus_toward(&mut app, "w1:p1", PaneDirection::Left);
    assert_eq!(focus["changed"], false);
    assert_eq!(
        focus["reason"],
        serde_json::to_value(PaneFocusDirectionReason::NoNeighbor).unwrap()
    );
    assert_eq!(focus["source_pane_id"], "w1:p1");
    assert_eq!(focus["focused_pane_id"], "w1:p1");
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
}

#[test]
fn process_info_needs_a_pane_with_a_process() {
    let (mut app, _, _) = pair();
    let info = json!({"type": "pane.process_info", "pane_id": "w1:p1"});
    assert_eq!(
        request(&mut app, info),
        failure("pane_not_found", "pane not found")
    );
    let missing = json!({"type": "pane.process_info", "pane_id": "w1:p9"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane not found")
    );
}

#[test]
fn daemon_pane_process_info_returns_response_for_existing_pane() {
    with_live_daemon(
        "pane_commands::daemon_pane_process_info_returns_response_for_existing_pane",
        async |app| {
            let created = answer(app, json!({"type": "workspace.create"})).await;
            let Ok(ResponseResult::WorkspaceCreated { root_pane, .. }) = created else {
                panic!("expected a created workspace: {created:?}");
            };
            let wire = json!({"type": "pane.process_info", "pane_id": root_pane.pane_id});
            let Ok(ResponseResult::PaneProcessInfo { process_info }) = answer(app, wire).await
            else {
                panic!("expected process info");
            };
            assert_eq!(process_info.pane_id, root_pane.pane_id);
            assert!(process_info.shell_pid.is_some());
            assert_eq!(process_info.tty, None);
            let focused = json!({"type": "pane.process_info"});
            let Ok(ResponseResult::PaneProcessInfo { process_info }) = answer(app, focused).await
            else {
                panic!("expected process info");
            };
            assert_eq!(process_info.pane_id, root_pane.pane_id);
        },
    );
}

#[test]
fn a_new_tab_root_is_listed_with_its_terminal_and_tab() {
    with_live_daemon(
        "pane_commands::a_new_tab_root_is_listed_with_its_terminal_and_tab",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let created = answer(app, json!({"type": "tab.create"})).await;
            let Ok(ResponseResult::TabCreated { tab, root_pane }) = created else {
                panic!("expected a created tab: {created:?}");
            };
            let list = json!({"type": "pane.list", "workspace_id": tab.workspace_id});
            let Ok(ResponseResult::PaneList { panes }) = answer(app, list).await else {
                panic!("expected a pane list");
            };
            let listed = panes
                .iter()
                .find(|pane| pane.pane_id == root_pane.pane_id)
                .expect("the new root pane is listed");
            assert_eq!(listed.terminal_id, root_pane.terminal_id);
            assert_eq!(listed.tab_id, tab.tab_id);
        },
    );
}
