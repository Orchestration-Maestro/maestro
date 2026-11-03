//! Pane close through the daemon's handler: the pane, its tab or its workspace goes, with its terminal.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, add_tab, daemon_with_workspaces, events_after, failure, join_group, request,
    root_pane, split_pane, workspace_ids,
};
use maestro_daemon::app::state::Mode;
use maestro_protocol::events::EventData;
use maestro_protocol::schema::{ResponseResult, SplitDirection};
use serde_json::json;

/// Closes the pane with the public handle `pane`.
fn close(
    app: &mut TestApp,
    pane: &str,
) -> Result<ResponseResult, maestro_protocol::schema::ErrorBody> {
    request(app, json!({"type": "pane.close", "pane_id": pane}))
}

#[test]
fn closing_a_split_pane_keeps_its_tab_and_publishes_the_layout() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let left = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, left, SplitDirection::Right);
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(right)
        .unwrap()
        .clone();
    assert_eq!(close(&mut app, "w1:p2"), Ok(ResponseResult::Ok {}));
    assert_eq!(
        app.state.topology.workspaces[0].tabs[0].layout.pane_ids(),
        [left]
    );
    assert!(!app.state.terminals.contains_key(&terminal));
    assert!(app.state.session_dirty);
    assert_eq!(
        events_after(&app, 0),
        [
            EventData::PaneClosed {
                pane_id: "w1:p2".into(),
                workspace_id: "w1".into(),
            },
            EventData::LayoutUpdated {
                layout: app.pane_layout_snapshot(0, 0).unwrap()
            },
        ]
    );
    assert_eq!(
        close(&mut app, "w1:p2"),
        failure("pane_not_found", "pane w1:p2 not found")
    );
}

#[test]
fn closing_the_only_pane_of_one_of_several_tabs_closes_only_that_tab() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let logs = add_tab(&mut app, 0);
    app.state.topology.workspaces[0].switch_tab(1);
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(logs)
        .unwrap()
        .clone();
    assert_eq!(close(&mut app, "w1:p2"), Ok(ResponseResult::Ok {}));
    assert_eq!(workspace_ids(&app), ["w1"]);
    assert_eq!(app.state.topology.workspaces[0].tabs.len(), 1);
    assert!(!app.state.terminals.contains_key(&terminal));
    assert_eq!(
        events_after(&app, 0),
        [EventData::PaneClosed {
            pane_id: "w1:p2".into(),
            workspace_id: "w1".into(),
        }]
    );
}

#[test]
fn closing_the_last_pane_closes_the_workspace() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]);
    let record = {
        let Ok(ResponseResult::WorkspaceInfo { workspace }) = request(
            &mut app,
            json!({"type": "workspace.get", "workspace_id": "w1"}),
        ) else {
            panic!("expected the workspace");
        };
        workspace
    };
    assert_eq!(close(&mut app, "w1:p1"), Ok(ResponseResult::Ok {}));
    assert_eq!(workspace_ids(&app), ["w2"]);
    assert_eq!(app.state.terminals.len(), 1);
    assert_eq!(app.state.active, Some(0));
    assert_eq!(
        events_after(&app, 0),
        [
            EventData::PaneClosed {
                pane_id: "w1:p1".into(),
                workspace_id: "w1".into(),
            },
            EventData::WorkspaceClosed {
                workspace_id: "w1".into(),
                workspace: Some(record),
            },
        ]
    );
    assert_eq!(close(&mut app, "w2:p1"), Ok(ResponseResult::Ok {}));
    assert!(app.state.topology.workspaces.is_empty());
    assert_eq!(app.state.active, None);
}

#[test]
fn closing_the_last_pane_of_a_parent_group_waits_for_confirmation() {
    let mut app = daemon_with_workspaces(&["/repo", "/repo-child"]);
    join_group(&mut app, 0, "repo-key", false);
    join_group(&mut app, 1, "repo-key", true);
    app.state.confirm_close.0 = true;
    app.state.selected = 1;
    assert_eq!(
        close(&mut app, "w1:p1"),
        failure(
            "confirmation_required",
            "closing this pane would close a worktree group"
        )
    );
    assert_eq!(app.state.mode, Mode::ConfirmClose);
    assert_eq!(app.state.selected, 0);
    assert_eq!(workspace_ids(&app), ["w1", "w2"]);
    assert_eq!(app.state.terminals.len(), 2);
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn closing_the_only_pane_of_a_linked_worktree_closes_only_its_workspace() {
    let mut app = daemon_with_workspaces(&["/repo", "/repo-issue"]);
    join_group(&mut app, 0, "repo-key", false);
    join_group(&mut app, 1, "repo-key", true);
    app.state.confirm_close.0 = true;
    assert_eq!(close(&mut app, "w2:p1"), Ok(ResponseResult::Ok {}));
    assert_eq!(workspace_ids(&app), ["w1"]);
}

#[test]
fn closing_a_missing_pane_is_refused_without_a_change() {
    let mut app = daemon_with_workspaces(&["/one"]);
    assert_eq!(
        close(&mut app, "w1:p9"),
        failure("pane_not_found", "pane w1:p9 not found")
    );
    assert_eq!(workspace_ids(&app), ["w1"]);
    assert!(events_after(&app, 0).is_empty());
}
