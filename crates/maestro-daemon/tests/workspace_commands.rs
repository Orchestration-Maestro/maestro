//! Workspace commands through the daemon's handler: targets, ordering, close and creation.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, add_workspaces, daemon_with_workspaces, events_after, failure, request,
    with_live_daemon, workspace_ids,
};
use maestro_daemon::app::state::Mode;
use maestro_daemon::settings::model::NewTerminalCwdConfig;
use maestro_protocol::events::EventData;
use maestro_protocol::rpc_types::RpcCommand;
use maestro_protocol::schema::{ResponseResult, WorkspaceInfo};
use maestro_workspace::workspace::WorktreeSpaceMembership;
use serde_json::json;
use std::path::PathBuf;

/// The workspace identities a list result holds, in order.
fn listed(result: &Result<ResponseResult, maestro_protocol::schema::ErrorBody>) -> Vec<String> {
    match result {
        Ok(ResponseResult::WorkspaceList { workspaces }) => ids_of(workspaces),
        other => panic!("expected a workspace list: {other:?}"),
    }
}

/// The identities of `workspaces`, in order.
fn ids_of(workspaces: &[WorkspaceInfo]) -> Vec<String> {
    workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect()
}

/// The identity a single-workspace result reports.
fn reported(result: Result<ResponseResult, maestro_protocol::schema::ErrorBody>) -> String {
    match result {
        Ok(ResponseResult::WorkspaceInfo { workspace }) => workspace.workspace_id,
        other => panic!("expected a workspace: {other:?}"),
    }
}

/// The identities of the active and selected workspaces.
fn navigation(app: &TestApp) -> (Option<String>, String) {
    let workspaces = &app.state.topology.workspaces;
    (
        app.state.active.map(|index| workspaces[index].id.clone()),
        workspaces[app.state.selected].id.clone(),
    )
}

/// Makes the workspace at `index` a member of the repository group `key`.
fn join_group(app: &mut TestApp, index: usize, key: &str, linked: bool) {
    app.state.topology.workspaces[index].worktree_space = Some(WorktreeSpaceMembership {
        key: key.into(),
        label: "repo".into(),
        repo_root: "/repo".into(),
        checkout_path: format!("/repo-{index}").into(),
        is_linked_worktree: linked,
    });
}

#[test]
fn workspace_targets_preserve_ids_aliases_and_missing_errors() {
    let mut app = daemon_with_workspaces(&["/one", "/two", "/three"]);
    assert_eq!(
        listed(&request(&mut app, json!({"type": "workspace.list"}))),
        ["w1", "w2", "w3"]
    );
    for alias in ["w2", "w_2", "2"] {
        let get = json!({"type": "workspace.get", "workspace_id": alias});
        assert_eq!(reported(request(&mut app, get)), "w2", "{alias}");
    }
    let moved = json!({"type": "workspace.move", "workspace_id": "w1", "insert_index": 3});
    assert_eq!(listed(&request(&mut app, moved)), ["w2", "w3", "w1"]);
    // Stable ids follow the workspace; positional aliases follow the display order.
    let get = |id: &str| json!({"type": "workspace.get", "workspace_id": id});
    assert_eq!(reported(request(&mut app, get("w1"))), "w1");
    assert_eq!(reported(request(&mut app, get("w_1"))), "w2");
    assert_eq!(reported(request(&mut app, get("3"))), "w1");
    let focus = json!({"type": "workspace.focus", "workspace_id": "w3"});
    assert_eq!(reported(request(&mut app, focus)), "w3");
    assert_eq!(navigation(&app).0.as_deref(), Some("w3"));
    let close = json!({"type": "workspace.close", "workspace_id": "w2"});
    assert_eq!(request(&mut app, close), Ok(ResponseResult::Ok {}));
    assert_missing_workspaces_change_nothing(&mut app);
}

/// Every command naming a missing, malformed or out-of-range workspace fails with its
/// exact text and changes neither order, focus nor events.
fn assert_missing_workspaces_change_nothing(app: &mut TestApp) {
    let get = |id: &str| json!({"type": "workspace.get", "workspace_id": id});
    let sequence = app.event_hub.current_sequence();
    for missing in ["w2", "", "w_", "wx", "w_9", "9", "0", "w_0"] {
        let expected = failure(
            "workspace_not_found",
            &format!("workspace {missing} not found"),
        );
        assert_eq!(request(app, get(missing)), expected, "{missing:?}");
        let focus = json!({"type": "workspace.focus", "workspace_id": missing});
        assert_eq!(request(app, focus), expected, "{missing:?}");
        let rename = json!({"type": "workspace.rename", "workspace_id": missing, "label": "x"});
        assert_eq!(request(app, rename), expected, "{missing:?}");
        let close = json!({"type": "workspace.close", "workspace_id": missing});
        assert_eq!(request(app, close), expected, "{missing:?}");
    }
    assert_eq!(workspace_ids(app), ["w3", "w1"]);
    assert_eq!(navigation(app).0.as_deref(), Some("w3"));
    assert!(events_after(app, sequence).is_empty());
}

#[test]
fn workspace_move_preserves_stable_identity_and_focus() {
    let mut app = daemon_with_workspaces(&["/one", "/two", "/three"]);
    app.state.selected = 1;
    let moved = json!({"type": "workspace.move", "workspace_id": "w1", "insert_index": 3});
    let result = request(&mut app, moved);
    assert_eq!(listed(&result), ["w2", "w3", "w1"]);
    assert_eq!(workspace_ids(&app), ["w2", "w3", "w1"]);
    assert_eq!(navigation(&app), (Some("w1".into()), "w2".into()));
    let Ok(ResponseResult::WorkspaceList { workspaces }) = result else {
        unreachable!()
    };
    assert_eq!(
        events_after(&app, 0),
        [EventData::WorkspaceMoved {
            workspace_id: "w1".into(),
            insert_index: 3,
            workspaces: workspaces.clone(),
        }]
    );
    assert_eq!(workspaces[2].number, 3);
    assert!(workspaces[2].focused);
    assert!(app.state.session_dirty);
}

#[test]
fn workspace_block_moves_are_atomic_and_keep_request_order() {
    let mut app = daemon_with_workspaces(&["/child", "/normal", "/parent", "/tail"]);
    app.state.active = Some(2);
    app.state.selected = 0;
    let block = json!({
        "type": "workspace.move_block",
        "workspace_ids": ["w3", "1"],
        "before_workspace_id": "w_4",
    });
    let result = request(&mut app, block);
    assert_eq!(listed(&result), ["w2", "w3", "w1", "w4"]);
    assert_eq!(workspace_ids(&app), ["w2", "w3", "w1", "w4"]);
    assert_eq!(navigation(&app), (Some("w3".into()), "w1".into()));
    let Ok(ResponseResult::WorkspaceList { workspaces }) = result else {
        unreachable!()
    };
    assert_eq!(
        events_after(&app, 0),
        [EventData::WorkspaceReordered {
            workspace_ids: vec!["w3".into(), "w1".into()],
            before_workspace_id: Some("w4".into()),
            workspaces,
        }]
    );
}

#[test]
fn unchanged_workspace_order_emits_no_move() {
    let mut app = daemon_with_workspaces(&["/one", "/two", "/three"]);
    for unchanged in [
        json!({"type": "workspace.move", "workspace_id": "w1", "insert_index": 0}),
        json!({"type": "workspace.move", "workspace_id": "w1", "insert_index": 1}),
        json!({"type": "workspace.move_block", "workspace_ids": ["w1", "w2"], "before_workspace_id": "w3"}),
        json!({"type": "workspace.move_block", "workspace_ids": ["w3"]}),
    ] {
        let result = request(&mut app, unchanged.clone());
        assert_eq!(listed(&result), ["w1", "w2", "w3"], "{unchanged}");
    }
    assert_eq!(workspace_ids(&app), ["w1", "w2", "w3"]);
    assert!(events_after(&app, 0).is_empty());
    assert!(!app.state.session_dirty);
}

#[test]
fn workspace_block_validation_is_atomic() {
    let mut app = daemon_with_workspaces(&["/one", "/two", "/three", "/four"]);
    let block = |ids: serde_json::Value, before: Option<&str>| json!({"type": "workspace.move_block", "workspace_ids": ids, "before_workspace_id": before});
    let failed = |message: &str| failure("workspace_move_block_failed", message);
    for (rejected, expected) in [
        (
            block(json!([]), None),
            failed("workspace_ids must not be empty"),
        ),
        (
            block(json!(["w1", "1"]), None),
            failed("workspace 1 appears more than once"),
        ),
        (
            block(json!(["w_2", "w2"]), None),
            failed("workspace w2 appears more than once"),
        ),
        (
            block(json!(["w1", "w9"]), None),
            failure("workspace_not_found", "workspace w9 not found"),
        ),
        (
            block(json!(["w1"]), Some("w9")),
            failure("workspace_not_found", "workspace w9 not found"),
        ),
        (
            block(json!(["w1", "w3"]), Some("3")),
            failed("before_workspace_id must not be part of workspace_ids"),
        ),
    ] {
        assert_eq!(request(&mut app, rejected.clone()), expected, "{rejected}");
        assert_eq!(workspace_ids(&app), ["w1", "w2", "w3", "w4"]);
    }
    assert!(events_after(&app, 0).is_empty());
    for (accepted, order) in [
        (block(json!(["w4"]), Some("w1")), ["w4", "w1", "w2", "w3"]),
        (block(json!(["w4"]), None), ["w1", "w2", "w3", "w4"]),
        (
            block(json!(["w3", "w1"]), Some("w2")),
            ["w3", "w1", "w2", "w4"],
        ),
    ] {
        assert_eq!(listed(&request(&mut app, accepted)), order);
    }
    assert_eq!(events_after(&app, 0).len(), 3);
}

#[test]
fn closing_a_linked_workspace_keeps_its_neighbours() {
    let mut app = daemon_with_workspaces(&["/repo", "/repo-one", "/repo-two", "/other"]);
    join_group(&mut app, 0, "repo-key", false);
    join_group(&mut app, 1, "repo-key", true);
    join_group(&mut app, 2, "repo-key", true);
    let closed: Vec<_> = app.state.topology.workspaces[1]
        .tabs
        .iter()
        .flat_map(|tab| tab.panes.values())
        .map(|pane| pane.attached_terminal_id.clone())
        .collect();
    let close = json!({"type": "workspace.close", "workspace_id": "w2"});
    assert_eq!(request(&mut app, close), Ok(ResponseResult::Ok {}));
    assert_eq!(workspace_ids(&app), ["w1", "w3", "w4"]);
    assert_eq!(app.state.terminals.len(), 3);
    assert!(
        closed
            .iter()
            .all(|id| !app.state.terminals.contains_key(id))
    );
    assert_eq!(navigation(&app).0.as_deref(), Some("w1"));
}

#[test]
fn workspace_close_publishes_the_final_worktree_snapshot() {
    let mut app = daemon_with_workspaces(&["/repo-issue"]);
    join_group(&mut app, 0, "repo-key", true);
    let get = json!({"type": "workspace.get", "workspace_id": "w1"});
    let Ok(ResponseResult::WorkspaceInfo { workspace: before }) = request(&mut app, get) else {
        panic!("expected the workspace")
    };
    let close = json!({"type": "workspace.close", "workspace_id": "1"});
    assert_eq!(request(&mut app, close), Ok(ResponseResult::Ok {}));
    assert!(app.state.topology.workspaces.is_empty());
    assert!(app.state.terminals.is_empty());
    assert_eq!(app.state.active, None);
    let worktree = before.worktree.as_ref().unwrap();
    assert!(worktree.is_linked_worktree);
    assert_eq!(
        (worktree.repo_key.as_str(), worktree.checkout_path.as_str()),
        ("repo-key", "/repo-0")
    );
    assert_eq!(
        events_after(&app, 0),
        [EventData::WorkspaceClosed {
            workspace_id: "w1".into(),
            workspace: Some(before),
        }]
    );
}

#[test]
fn daemon_api_request_drains_all_pending_internal_events_before_reading_state() {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::{Agent, AgentState};
    let mut app = daemon_with_workspaces(&["/one"]);
    let (events, receiver) = tokio::sync::mpsc::channel(256);
    app.event_rx = receiver;
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    for count in 0..100 {
        events
            .try_send(AppEvent::TerminalBell {
                pane_id: pane,
                count,
            })
            .unwrap();
    }
    events
        .try_send(AppEvent::StateChanged {
            pane_id: pane,
            agent: Some(Agent::Codex),
            state: AgentState::Working,
            visible_blocker: false,
            visible_working: true,
            process_exited: false,
            observed_at: std::time::Instant::now(),
        })
        .unwrap();
    let get = json!({"type": "workspace.get", "workspace_id": "w1"});
    let Ok(ResponseResult::WorkspaceInfo { workspace }) = request(&mut app, get) else {
        panic!("expected the workspace")
    };
    assert_eq!(
        workspace.agent_status,
        maestro_protocol::schema::AgentStatus::Working
    );
    assert!(app.event_rx.try_recv().is_err());
}

#[test]
fn daemon_read_only_api_requests_do_not_force_rerender() {
    let mut app = daemon_with_workspaces(&["/one", "/two"]);
    for read in [
        json!({"type": "workspace.list"}),
        json!({"type": "workspace.get", "workspace_id": "w1"}),
        json!({"type": "tab.list"}),
        json!({"type": "tab.get", "tab_id": "w1:t1"}),
    ] {
        assert!(request(&mut app, read.clone()).is_ok(), "{read}");
        assert!(!app.launch.render_dirty.is_pending(), "{read}");
    }
    let focus = json!({"type": "workspace.focus", "workspace_id": "w2"});
    assert!(request(&mut app, focus).is_ok());
    assert!(app.launch.render_dirty.is_pending());
    let _ = app.launch.render_dirty.take();
    // A queued internal change still renders when a read drains it.
    let (events, receiver) = tokio::sync::mpsc::channel(8);
    app.event_rx = receiver;
    events
        .try_send(maestro_daemon::events::AppEvent::ClipboardWrite {
            content: b"copied".to_vec(),
        })
        .unwrap();
    assert!(request(&mut app, json!({"type": "workspace.list"})).is_ok());
    assert!(app.launch.render_dirty.is_pending());
}

/// A shell configuration whose new panes start in `policy`'s directory.
fn with_policy(app: &mut TestApp, policy: NewTerminalCwdConfig) {
    app.config.terminal.new_cwd = policy;
}

/// The created records of a `workspace.create` result.
fn created(
    result: Result<ResponseResult, maestro_protocol::schema::ErrorBody>,
) -> (
    WorkspaceInfo,
    maestro_protocol::schema::TabInfo,
    maestro_protocol::schema::PaneInfo,
) {
    match result {
        Ok(ResponseResult::WorkspaceCreated {
            workspace,
            tab,
            root_pane,
        }) => (workspace, tab, root_pane),
        other => panic!("expected a created workspace: {other:?}"),
    }
}

/// Answers `wire` through the handler from inside a running test runtime.
async fn answer(
    app: &mut TestApp,
    wire: serde_json::Value,
) -> Result<ResponseResult, maestro_protocol::schema::ErrorBody> {
    let command: RpcCommand = crate::support::api::command(wire);
    let mut settings = crate::support::api::settings();
    let paths = crate::support::api::untouched_paths();
    app.handle_api_request(command, &mut settings, &paths).await
}

#[test]
fn daemon_workspace_create_response_includes_initial_tab_and_root_pane() {
    with_live_daemon(
        "workspace_commands::daemon_workspace_create_response_includes_initial_tab_and_root_pane",
        async |app| {
            // No active workspace: even an unfocused creation becomes active.
            let first = answer(
                app,
                json!({"type": "workspace.create", "label": "api-root"}),
            )
            .await;
            let (workspace, tab, root_pane) = created(first);
            assert_eq!(workspace.workspace_id, "w1");
            assert_eq!(workspace.label, "api-root");
            assert_eq!(
                (workspace.number, workspace.tab_count, workspace.pane_count),
                (1, 1, 1)
            );
            assert_eq!(workspace.active_tab_id, tab.tab_id);
            assert_eq!(
                (tab.tab_id.as_str(), tab.workspace_id.as_str()),
                ("w1:t1", "w1")
            );
            assert_eq!((tab.number, tab.pane_count), (1, 1));
            assert_eq!(root_pane.pane_id, "w1:p1");
            assert_eq!(root_pane.workspace_id, workspace.workspace_id);
            assert_eq!(root_pane.tab_id, tab.tab_id);
            assert!(root_pane.terminal_id.starts_with("term_"));
            assert_ne!(root_pane.terminal_id, root_pane.pane_id);
            assert!(workspace.focused && tab.focused && root_pane.focused);
            assert_eq!(
                events_after(app, 0),
                [
                    EventData::WorkspaceCreated { workspace },
                    EventData::TabCreated { tab },
                    EventData::PaneCreated { pane: root_pane },
                    EventData::LayoutUpdated {
                        layout: app.pane_layout_snapshot(0, 0).unwrap()
                    },
                ]
            );
            let unfocused = answer(app, json!({"type": "workspace.create", "focus": false})).await;
            let (workspace, tab, root_pane) = created(unfocused);
            assert_eq!(workspace.workspace_id, "w2");
            assert!(!workspace.focused && !tab.focused && !root_pane.focused);
            assert_eq!(app.state.active, Some(0));
            let focused = answer(app, json!({"type": "workspace.create", "focus": true})).await;
            let (workspace, tab, root_pane) = created(focused);
            assert_eq!(workspace.workspace_id, "w3");
            assert!(workspace.focused && tab.focused && root_pane.focused);
            assert_eq!(app.state.active, Some(2));
        },
    );
}

#[test]
fn workspace_creation_follows_the_focused_pane_directory() {
    with_live_daemon(
        "workspace_commands::workspace_creation_follows_the_focused_pane_directory",
        async |app| {
            let root = maestro_test_directory::Scratch::new().unwrap();
            let (first, focused) = (root.path().join("root"), root.path().join("focused"));
            std::fs::create_dir(&first).unwrap();
            std::fs::create_dir(&focused).unwrap();
            with_policy(app, NewTerminalCwdConfig::Follow);
            add_workspaces(app, &[first.to_str().unwrap()]);
            app.state
                .topology
                .create_tab_with_runtime(0, |_, _| Ok(()))
                .unwrap()
                .unwrap();
            crate::support::populate_terminals(&mut app.state);
            app.state.topology.workspaces[0].active_tab = 1;
            let tab = &app.state.topology.workspaces[0].tabs[1];
            let terminal = tab.terminal_id(tab.root_pane).unwrap().clone();
            app.state.terminals.get_mut(&terminal).unwrap().cwd = focused.clone();
            let result = answer(app, json!({"type": "workspace.create"})).await;
            let (workspace, _, root_pane) = created(result);
            assert_eq!(root_pane.cwd.map(PathBuf::from), Some(focused.clone()));
            assert_eq!(app.state.topology.workspaces[1].identity_cwd, focused);
            assert_eq!(workspace.workspace_id, "w2");
        },
    );
}

#[test]
fn workspace_creation_preserves_explicit_cwd_and_focus_policy() {
    with_live_daemon(
        "workspace_commands::workspace_creation_preserves_explicit_cwd_and_focus_policy",
        async |app| {
            let root = maestro_test_directory::Scratch::new().unwrap();
            let explicit = root.path().join("explicit");
            std::fs::create_dir(&explicit).unwrap();
            let policy = maestro_test_directory::temporary_root();
            let create = json!({"type": "workspace.create", "cwd": explicit, "focus": true});
            let (workspace, ..) = created(answer(app, create).await);
            assert_eq!(app.state.topology.workspaces[0].identity_cwd, explicit);
            assert_eq!(workspace.label, "explicit");
            let (workspace, ..) = created(answer(app, json!({"type": "workspace.create"})).await);
            assert_eq!(app.state.topology.workspaces[1].identity_cwd, policy);
            assert!(!workspace.focused);
            assert_eq!(app.state.active, Some(0));
            assert_eq!(app.state.mode, Mode::Terminal);
            // A child that cannot start leaves topology, terminals and runtimes as they were.
            app.config.terminal.default_shell = "/missing-maestro-shell-fixture/bin/sh".into();
            let sequence = app.event_hub.current_sequence();
            let failed = answer(app, json!({"type": "workspace.create", "label": "lost"})).await;
            let Err(error) = failed else {
                panic!("a missing shell must fail: {failed:?}")
            };
            assert_eq!(error.code, "workspace_create_failed");
            assert!(!error.message.is_empty());
            assert_eq!(workspace_ids(app), ["w1", "w2"]);
            assert_eq!(
                (app.state.terminals.len(), app.terminal_runtimes.len()),
                (2, 2)
            );
            assert!(events_after(app, sequence).is_empty());
        },
    );
}
