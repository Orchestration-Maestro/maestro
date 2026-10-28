//! Tab commands through the daemon's handler: targets, ordering, close and creation.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, add_workspaces, daemon_with_workspaces, events_after, failure, request,
    with_live_daemon, workspace_ids,
};
use maestro_daemon::app::state::Mode;
use maestro_daemon::settings::model::NewTerminalCwdConfig;
use maestro_protocol::events::EventData;
use maestro_protocol::rpc_types::RpcCommand;
use maestro_protocol::schema::{ErrorBody, PaneInfo, ResponseResult, TabInfo};
use maestro_workspace::workspace::WorktreeSpaceMembership;
use serde_json::{Value, json};
use std::path::PathBuf;

/// Adds `count` tabs without processes to the workspace at `index`.
fn add_tabs(app: &mut TestApp, index: usize, count: usize) {
    for _ in 0..count {
        app.state
            .topology
            .create_tab_with_runtime(index, |_, _| Ok(()))
            .unwrap()
            .unwrap();
    }
    crate::support::populate_terminals(&mut app.state);
}

/// The tab identities a list result holds, in order.
fn listed(result: &Result<ResponseResult, ErrorBody>) -> Vec<String> {
    match result {
        Ok(ResponseResult::TabList { tabs }) => tabs.iter().map(|tab| tab.tab_id.clone()).collect(),
        other => panic!("expected a tab list: {other:?}"),
    }
}

/// The tab a single-tab result reports.
fn reported(result: Result<ResponseResult, ErrorBody>) -> TabInfo {
    match result {
        Ok(ResponseResult::TabInfo { tab }) => tab,
        other => panic!("expected a tab: {other:?}"),
    }
}

/// The public tab identities of the workspace at `index`, in display order.
fn tab_ids(app: &TestApp, index: usize) -> Vec<String> {
    let workspace = &app.state.topology.workspaces[index];
    workspace
        .tabs
        .iter()
        .map(|tab| format!("{}:t{}", workspace.id, tab.number))
        .collect()
}

/// A workspace with three tabs and a second workspace with one.
fn three_tabs() -> TestApp {
    let mut app = daemon_with_workspaces(&["/tabs", "/other"]);
    add_tabs(&mut app, 0, 2);
    app
}

#[test]
fn tab_targets_and_creation_preserve_source_policy() {
    let mut app = three_tabs();
    assert_eq!(
        listed(&request(&mut app, json!({"type": "tab.list"}))),
        ["w1:t1", "w1:t2", "w1:t3", "w2:t1"]
    );
    let list = json!({"type": "tab.list", "workspace_id": "w_2"});
    assert_eq!(listed(&request(&mut app, list)), ["w2:t1"]);
    let list = json!({"type": "tab.list", "workspace_id": "w9"});
    assert_eq!(
        request(&mut app, list),
        failure("workspace_not_found", "workspace w9 not found")
    );
    for alias in ["w1:t2", "t_w1_2", "w1:2", "1:t2"] {
        let get = json!({"type": "tab.get", "tab_id": alias});
        assert_eq!(reported(request(&mut app, get)).tab_id, "w1:t2", "{alias}");
    }
    let focus = json!({"type": "tab.focus", "tab_id": "t_w2_1"});
    let tab = reported(request(&mut app, focus));
    assert!(tab.focused);
    assert_eq!(
        (
            app.state.active,
            app.state.topology.workspaces[1].active_tab
        ),
        (Some(1), 0)
    );
    let rename = json!({"type": "tab.rename", "tab_id": "w1:3", "label": "logs"});
    assert_eq!(reported(request(&mut app, rename)).label, "logs");
    assert_missing_tabs_change_nothing(&mut app);
    assert_creation_targets_resolve_before_spawning(&mut app);
}

/// Every command naming a missing tab fails with its exact text and publishes nothing.
fn assert_missing_tabs_change_nothing(app: &mut TestApp) {
    let sequence = app.event_hub.current_sequence();
    for missing in ["w1:t9", "w1:9", "t_w1_9", "w9:t1", "", "w1"] {
        let expected = failure("tab_not_found", &format!("tab {missing} not found"));
        for command in [
            json!({"type": "tab.get", "tab_id": missing}),
            json!({"type": "tab.focus", "tab_id": missing}),
            json!({"type": "tab.rename", "tab_id": missing, "label": "x"}),
            json!({"type": "tab.move", "tab_id": missing, "insert_index": 0}),
            json!({"type": "tab.close", "tab_id": missing}),
        ] {
            assert_eq!(request(app, command.clone()), expected, "{command}");
        }
    }
    assert!(events_after(app, sequence).is_empty());
}

/// Creation targets are resolved before any child starts.
fn assert_creation_targets_resolve_before_spawning(app: &mut TestApp) {
    let create = json!({"type": "tab.create", "workspace_id": "w9"});
    assert_eq!(
        request(app, create),
        failure("workspace_not_found", "workspace w9 not found")
    );
    let create = json!({"type": "tab.create", "workspace_id": "w_9"});
    assert_eq!(
        request(app, create),
        failure("tab_create_failed", "workspace disappeared")
    );
    app.state.active = None;
    assert_eq!(
        request(app, json!({"type": "tab.create"})),
        failure("workspace_not_found", "no active workspace")
    );
    assert_eq!(app.terminal_runtimes.len(), 0);
    assert_eq!(tab_ids(app, 0), ["w1:t1", "w1:t2", "w1:t3"]);
}

#[test]
fn tab_move_keeps_its_workspace_and_active_identity() {
    let mut app = three_tabs();
    app.state.topology.workspaces[0].active_tab = 1;
    let active_root = app.state.topology.workspaces[0].tabs[1].root_pane;
    let moved = json!({"type": "tab.move", "tab_id": "w1:t1", "insert_index": 3});
    let result = request(&mut app, moved);
    assert_eq!(listed(&result), ["w1:t2", "w1:t3", "w1:t1"]);
    assert_eq!(tab_ids(&app, 0), ["w1:t2", "w1:t3", "w1:t1"]);
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.tabs[workspace.active_tab].root_pane, active_root);
    assert_eq!(tab_ids(&app, 1), ["w2:t1"]);
    let Ok(ResponseResult::TabList { tabs }) = result else {
        unreachable!()
    };
    assert_eq!(
        tabs.iter().map(|tab| tab.number).collect::<Vec<_>>(),
        [2, 3, 1]
    );
    assert_eq!(
        events_after(&app, 0),
        [EventData::TabMoved {
            tab_id: "w1:t1".into(),
            workspace_id: "w1".into(),
            insert_index: 3,
            tabs,
        }]
    );
    assert!(app.state.session_dirty);
}

#[test]
fn renaming_a_tab_updates_its_label_and_publishes_it() {
    let mut app = three_tabs();
    app.state.session_dirty = false;
    let rename = json!({"type": "tab.rename", "tab_id": "w1:t2", "label": "a much longer label"});
    let tab = reported(request(&mut app, rename));
    assert_eq!(
        (tab.tab_id.as_str(), tab.label.as_str()),
        ("w1:t2", "a much longer label")
    );
    assert_eq!(
        app.state.topology.workspaces[0]
            .tab_display_name(1)
            .as_deref(),
        Some("a much longer label")
    );
    assert!(app.state.session_dirty);
    assert_eq!(
        events_after(&app, 0),
        [EventData::TabRenamed {
            tab_id: "w1:t2".into(),
            workspace_id: "w1".into(),
            label: "a much longer label".into(),
        }]
    );
}

#[test]
fn single_moves_validate_insertion_boundaries() {
    let mut app = three_tabs();
    add_workspaces(&mut app, &["/third"]);
    for (index, (insert, order)) in [
        (0, ["w1:t1", "w1:t2", "w1:t3"]),
        (1, ["w1:t1", "w1:t2", "w1:t3"]),
        (3, ["w1:t2", "w1:t3", "w1:t1"]),
    ]
    .into_iter()
    .enumerate()
    {
        let mut app = three_tabs();
        let moved = json!({"type": "tab.move", "tab_id": "w1:t1", "insert_index": insert});
        assert_eq!(listed(&request(&mut app, moved)), order, "{insert}");
        assert_eq!(
            events_after(&app, 0).len(),
            usize::from(index == 2),
            "{insert}"
        );
    }
    let moved = json!({"type": "tab.move", "tab_id": "w1:t1", "insert_index": 4});
    assert_eq!(
        request(&mut app, moved),
        failure("tab_move_failed", "insert_index 4 is out of bounds")
    );
    let moved = json!({"type": "workspace.move", "workspace_id": "w1", "insert_index": 4});
    assert_eq!(
        request(&mut app, moved),
        failure("workspace_move_failed", "insert_index 4 is out of bounds")
    );
    let moved = json!({"type": "workspace.move", "workspace_id": "w2", "insert_index": 0});
    assert!(request(&mut app, moved).is_ok());
    let moved = json!({"type": "workspace.move", "workspace_id": "w2", "insert_index": 3});
    assert!(request(&mut app, moved).is_ok());
    assert_eq!(workspace_ids(&app), ["w1", "w3", "w2"]);
    assert_eq!(app.state.active, Some(0));
    assert_eq!(events_after(&app, 0).len(), 2);
}

/// Joins the workspace at `index` to the repository group `key`.
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
fn final_tab_close_publishes_tab_then_workspace() {
    let mut app = daemon_with_workspaces(&["/repo-issue", "/other"]);
    join_group(&mut app, 0, "repo-key", true);
    let get = json!({"type": "workspace.get", "workspace_id": "w1"});
    let Ok(ResponseResult::WorkspaceInfo { workspace }) = request(&mut app, get) else {
        panic!("expected the workspace")
    };
    app.state.session_dirty = false;
    let close = json!({"type": "tab.close", "tab_id": "w1:t1"});
    assert_eq!(request(&mut app, close), Ok(ResponseResult::Ok {}));
    assert_eq!(workspace_ids(&app), ["w2"]);
    assert_eq!(app.state.terminals.len(), 1);
    assert!(app.state.session_dirty);
    assert!(workspace.worktree.as_ref().unwrap().is_linked_worktree);
    assert_eq!(
        events_after(&app, 0),
        [
            EventData::TabClosed {
                tab_id: "w1:t1".into(),
                workspace_id: "w1".into(),
            },
            EventData::WorkspaceClosed {
                workspace_id: "w1".into(),
                workspace: Some(workspace),
            },
        ]
    );
}

#[test]
fn tab_close_preserves_confirmation_and_terminal_ownership() {
    let mut app = three_tabs();
    let closing = app.state.topology.workspaces[0].tabs[1].clone();
    let closed = closing.terminal_id(closing.root_pane).unwrap().clone();
    let close = json!({"type": "tab.close", "tab_id": "w1:t2"});
    assert_eq!(request(&mut app, close), Ok(ResponseResult::Ok {}));
    assert_eq!(tab_ids(&app, 0), ["w1:t1", "w1:t3"]);
    assert!(!app.state.terminals.contains_key(&closed));
    assert_eq!(app.state.terminals.len(), 3);
    assert_eq!(
        events_after(&app, 0),
        [EventData::TabClosed {
            tab_id: "w1:t2".into(),
            workspace_id: "w1".into(),
        }]
    );
    // The last tab of a repository with linked worktrees asks before closing them all.
    let mut app = daemon_with_workspaces(&["/repo", "/repo-one"]);
    join_group(&mut app, 0, "repo-key", false);
    join_group(&mut app, 1, "repo-key", true);
    app.state.confirm_close.0 = true;
    let close = json!({"type": "tab.close", "tab_id": "w1:t1"});
    assert_eq!(
        request(&mut app, close),
        failure(
            "confirmation_required",
            "closing this tab would close a worktree group"
        )
    );
    assert_eq!(workspace_ids(&app), ["w1", "w2"]);
    assert_eq!(app.state.terminals.len(), 2);
    assert_eq!(app.state.mode, Mode::ConfirmClose);
    assert!(events_after(&app, 0).is_empty());
}

/// The created records of a `tab.create` result.
fn created(result: Result<ResponseResult, ErrorBody>) -> (TabInfo, PaneInfo) {
    match result {
        Ok(ResponseResult::TabCreated { tab, root_pane }) => (tab, root_pane),
        other => panic!("expected a created tab: {other:?}"),
    }
}

/// Answers `wire` through the handler from inside a running test runtime.
async fn answer(app: &mut TestApp, wire: Value) -> Result<ResponseResult, ErrorBody> {
    let command: RpcCommand = crate::support::api::command(wire);
    let mut settings = crate::support::api::settings();
    let paths = crate::support::api::untouched_paths();
    app.handle_api_request(command, &mut settings, &paths).await
}

#[test]
fn daemon_tab_create_response_includes_root_pane() {
    with_live_daemon(
        "tab_commands::daemon_tab_create_response_includes_root_pane",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let focused = json!({"type": "tab.create", "focus": true, "label": "build"});
            let (tab, root_pane) = created(answer(app, focused).await);
            assert_eq!(
                (tab.tab_id.as_str(), tab.workspace_id.as_str()),
                ("w1:t2", "w1")
            );
            assert_eq!(
                (tab.number, tab.pane_count, tab.label.as_str()),
                (2, 1, "build")
            );
            assert_eq!(root_pane.pane_id, "w1:p2");
            assert_eq!(root_pane.tab_id, tab.tab_id);
            assert_eq!(root_pane.workspace_id, tab.workspace_id);
            assert!(root_pane.terminal_id.starts_with("term_"));
            assert!(tab.focused && root_pane.focused);
            assert_eq!(app.state.topology.workspaces[0].active_tab, 1);
            assert_eq!(
                events_after(app, 0),
                [
                    EventData::TabCreated { tab },
                    EventData::PaneCreated { pane: root_pane },
                    EventData::LayoutUpdated {
                        layout: app.pane_layout_snapshot(0, 1).unwrap()
                    },
                ]
            );
            let unfocused = json!({"type": "tab.create", "workspace_id": "w1"});
            let (tab, root_pane) = created(answer(app, unfocused).await);
            assert_eq!(tab.tab_id, "w1:t3");
            assert!(!tab.focused && !root_pane.focused);
            assert_eq!(app.state.topology.workspaces[0].active_tab, 1);
        },
    );
}

#[test]
fn tab_creation_uses_the_focused_cached_directory() {
    with_live_daemon(
        "tab_commands::tab_creation_uses_the_focused_cached_directory",
        async |app| {
            let root = maestro_test_directory::Scratch::new().unwrap();
            let (first, cached) = (root.path().join("root"), root.path().join("cached"));
            std::fs::create_dir(&first).unwrap();
            std::fs::create_dir(&cached).unwrap();
            app.config.terminal.new_cwd = NewTerminalCwdConfig::Follow;
            add_workspaces(app, &[first.to_str().unwrap()]);
            add_tabs(app, 0, 1);
            app.state.topology.workspaces[0].active_tab = 1;
            let tab = &app.state.topology.workspaces[0].tabs[1];
            let terminal = tab.terminal_id(tab.root_pane).unwrap().clone();
            app.state.terminals.get_mut(&terminal).unwrap().cwd = cached.clone();
            let (_, root_pane) = created(answer(app, json!({"type": "tab.create"})).await);
            assert_eq!(root_pane.cwd.map(PathBuf::from), Some(cached));
        },
    );
}

#[test]
fn launch_environment_rejects_equals_in_keys_before_spawning() {
    let mut app = daemon_with_workspaces(&["/tabs"]);
    for create in [
        json!({"type": "tab.create", "env": {"BAD=KEY": "value"}}),
        json!({"type": "workspace.create", "env": {"BAD=KEY": "value"}}),
    ] {
        assert_eq!(
            request(&mut app, create),
            failure("invalid_env", "env key BAD=KEY must not contain '='")
        );
    }
    assert_eq!(workspace_ids(&app), ["w1"]);
    assert_eq!(app.state.topology.workspaces[0].tabs.len(), 1);
    assert_eq!(app.state.topology.workspaces[0].next_public_tab_number, 2);
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn launch_environment_reaches_the_root_pane() {
    with_live_daemon(
        "tab_commands::launch_environment_reaches_the_root_pane",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let create = json!({
                "type": "tab.create",
                "focus": true,
                "env": {"MAESTRO_TEST_ECHO": "é", "HOME": "/overridden", "EMPTY": "", "MAESTRO_PANE_ID": "spoofed"},
            });
            let (_, root_pane) = created(answer(app, create).await);
            let terminal = app.state.topology.workspaces[0].tabs[1]
                .terminal_id(app.state.topology.workspaces[0].tabs[1].root_pane)
                .unwrap()
                .clone();
            let runtime = app.terminal_runtimes.get(&terminal).unwrap();
            let script = "printf '\\nseen:<%s>|<%s>|<%s>|<%s>\\n' \"$MAESTRO_TEST_ECHO\" \"$HOME\" \"${EMPTY-unset}\" \"$MAESTRO_PANE_ID\"\n";
            runtime
                .send_bytes(script.as_bytes().to_vec())
                .await
                .unwrap();
            let expected = format!("seen:<é>|</overridden>|<>|<{}>", root_pane.pane_id);
            let text = loop {
                let _ = app.launch.render_dirty.take();
                let text = runtime.visible_text();
                if text.lines().any(|line| line.starts_with("seen:<")) {
                    break text;
                }
                app.launch.render_notify.notified().await;
            };
            assert!(text.contains(&expected), "{text:?}");
        },
    );
}
