//! Pane topology commands through the daemon's handler: reads, focus and neighbours.
use crate::state::model::Mode;
use crate::state::test_support::api::{
    add_tab, add_workspaces, answer, daemon_with_workspaces, failure, pair, request, root_pane,
};
use crate::state::test_support::pane_commands::{focus_toward, public, reported};
use crate::state::tests::support::live::with_live_daemon;
use maestro_detection::agents::AgentState;
use maestro_protocol::schema::{AgentStatus, PaneDirection, ResponseResult};
use serde_json::json;

#[test]
fn the_current_pane_is_refused_without_a_caller_or_an_active_pane() {
    let (mut app, _, _) = pair().unwrap();
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
fn focusing_a_pane_activates_its_tab_and_workspace() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]).unwrap();
    let target = add_tab(&mut app, 1).unwrap();
    let target_id = public(&app, 1, target).unwrap();
    assert_eq!(app.state.active, Some(0));
    let focus = json!({"type": "pane.focus", "pane_id": target_id});
    let pane = reported(request(&mut app, focus)).unwrap();
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
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
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
        reported(request(&mut app, get)).unwrap().agent_status,
        AgentStatus::Done
    );
    let focus = json!({"type": "pane.focus", "pane_id": "w1:p1"});
    assert_eq!(
        reported(request(&mut app, focus)).unwrap().agent_status,
        AgentStatus::Idle
    );
}
#[test]
fn focus_moves_to_the_neighbour_in_a_direction() {
    let (mut app, _, right) = pair().unwrap();
    let focus = focus_toward(&mut app, "w1:p1", PaneDirection::Right).unwrap();
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
            add_workspaces(app, &["/tabs"]).unwrap();
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
