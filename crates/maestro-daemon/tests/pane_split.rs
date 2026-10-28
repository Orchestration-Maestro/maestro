//! Pane split through the daemon's handler: targets, focus, ratio, launch inputs and publication.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, add_tab, add_workspaces, answer, daemon_with_workspaces, event_names, events_after,
    failure, request, root_pane, run_in_pane, split_pane, with_live_daemon,
};
use maestro_daemon::app::state::Mode;
use maestro_daemon::settings::model::NewTerminalCwdConfig;
use maestro_protocol::events::EventData;
use maestro_protocol::schema::{ErrorBody, PaneInfo, ResponseResult, SplitDirection};
use maestro_workspace::layout::PaneId;
use serde_json::json;
use std::path::PathBuf;

/// The pane a split result reports.
fn split_of(result: Result<ResponseResult, ErrorBody>) -> PaneInfo {
    match result {
        Ok(ResponseResult::PaneInfo { pane }) => pane,
        other => panic!("expected the new pane: {other:?}"),
    }
}

/// A workspace with a background second tab of two panes, the first tab's root focused.
///
/// Returns the first tab's root, the background tab's root and the background tab's focused pane.
fn background_tab(app: &mut TestApp) -> (PaneId, PaneId, PaneId) {
    add_workspaces(app, &["/tabs"]);
    let active = root_pane(app, 0);
    let target = add_tab(app, 0);
    let focused = split_pane(app, 0, target, SplitDirection::Right);
    app.state.topology.workspaces[0].tabs[1]
        .layout
        .focus_pane(focused);
    app.state.focus_pane_in_workspace(0, focused);
    app.state.focus_pane_in_workspace(0, active);
    (active, target, focused)
}

#[test]
fn a_split_targets_the_named_pane_in_a_background_tab() {
    with_live_daemon(
        "pane_split::a_split_targets_the_named_pane_in_a_background_tab",
        async |app| {
            let scratch = maestro_test_directory::Scratch::new().unwrap();
            app.config.terminal.new_cwd = NewTerminalCwdConfig::Follow;
            let (active, target, focused) = background_tab(app);
            let terminal = app.state.topology.workspaces[0]
                .terminal_id(target)
                .unwrap()
                .clone();
            app.state.terminals.get_mut(&terminal).unwrap().cwd = scratch.path().to_path_buf();
            let wire =
                json!({"type": "pane.split", "target_pane_id": "w1:p2", "direction": "right"});
            let pane = split_of(answer(app, wire).await);
            assert_eq!((pane.tab_id.as_str(), pane.focused), ("w1:t2", false));
            assert_eq!(
                pane.cwd.map(PathBuf::from),
                Some(scratch.path().to_path_buf())
            );
            let workspace = &app.state.topology.workspaces[0];
            assert_eq!((app.state.active, workspace.active_tab), (Some(0), 0));
            assert_eq!(workspace.tabs[0].layout.focused(), active);
            assert_eq!(workspace.tabs[0].layout.pane_count(), 1);
            assert_eq!(workspace.tabs[1].layout.focused(), focused);
            assert_eq!(workspace.tabs[1].layout.pane_count(), 3);
            app.state.last_pane();
            let workspace = &app.state.topology.workspaces[0];
            assert_eq!(workspace.active_tab, 1);
            assert_eq!(workspace.focused_pane_id(), Some(focused));
        },
    );
}

#[test]
fn a_split_focuses_the_new_pane_when_asked() {
    with_live_daemon(
        "pane_split::a_split_focuses_the_new_pane_when_asked",
        async |app| {
            let (active, _, _) = background_tab(app);
            let wire = json!({
                "type": "pane.split", "target_pane_id": "w1:p2", "direction": "down", "focus": true
            });
            let pane = split_of(answer(app, wire).await);
            assert_eq!((pane.tab_id.as_str(), pane.focused), ("w1:t2", true));
            let workspace = &app.state.topology.workspaces[0];
            assert_eq!((app.state.active, workspace.active_tab), (Some(0), 1));
            assert_eq!(app.state.mode, Mode::Terminal);
            assert_eq!(
                app.state
                    .public_pane_id(0, workspace.focused_pane_id().unwrap()),
                Some(pane.pane_id)
            );
            app.state.last_pane();
            assert_eq!(
                app.state.topology.workspaces[0].focused_pane_id(),
                Some(active)
            );
        },
    );
}

#[test]
fn a_split_applies_the_ratio_and_the_right_click_routing() {
    with_live_daemon(
        "pane_split::a_split_applies_the_ratio_and_the_right_click_routing",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let wire = json!({
                "type": "pane.split", "target_pane_id": "w1:p1", "direction": "right",
                "ratio": 0.333, "right_click": "pane"
            });
            let pane = split_of(answer(app, wire).await);
            let workspace = &app.state.topology.workspaces[0];
            let splits = workspace.tabs[0].layout.splits(app.state.view_area());
            assert_eq!(splits.len(), 1);
            assert!((splits[0].ratio - 0.333).abs() < f32::EPSILON);
            let (_, created) = app.state.parse_pane_id(&pane.pane_id).unwrap();
            assert!(
                workspace
                    .pane_state(created)
                    .unwrap()
                    .right_click_passthrough
            );
            let wire = json!({"type": "pane.split", "direction": "down"});
            let pane = split_of(answer(app, wire).await);
            let (_, created) = app.state.parse_pane_id(&pane.pane_id).unwrap();
            let workspace = &app.state.topology.workspaces[0];
            assert!(
                !workspace
                    .pane_state(created)
                    .unwrap()
                    .right_click_passthrough
            );
        },
    );
}

#[test]
fn a_split_without_a_target_uses_the_focused_pane_of_the_workspace_or_the_active_one() {
    with_live_daemon(
        "pane_split::a_split_without_a_target_uses_the_focused_pane_of_the_workspace_or_the_active_one",
        async |app| {
            add_workspaces(app, &["/one", "/two"]);
            let wire = json!({"type": "pane.split", "direction": "right"});
            assert_eq!(split_of(answer(app, wire).await).workspace_id, "w1");
            let wire = json!({"type": "pane.split", "direction": "right", "workspace_id": "w2"});
            assert_eq!(split_of(answer(app, wire).await).workspace_id, "w2");
            let workspaces = &app.state.topology.workspaces;
            assert_eq!(workspaces[0].tabs[0].layout.pane_count(), 2);
            assert_eq!(workspaces[1].tabs[0].layout.pane_count(), 2);
            assert_eq!(app.state.active, Some(0));
        },
    );
}

#[test]
fn a_split_publishes_the_new_pane_and_its_layout() {
    with_live_daemon(
        "pane_split::a_split_publishes_the_new_pane_and_its_layout",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            app.state.session_dirty = false;
            let wire =
                json!({"type": "pane.split", "target_pane_id": "w1:p1", "direction": "right"});
            let pane = split_of(answer(app, wire).await);
            assert_eq!(pane.pane_id, "w1:p2");
            assert!(pane.terminal_id.starts_with("term_"));
            assert!(app.state.session_dirty);
            let (workspace, created) = app.state.parse_pane_id(&pane.pane_id).unwrap();
            let terminal = app.state.topology.workspaces[workspace]
                .terminal_id(created)
                .unwrap();
            assert!(app.terminal_runtimes.get(terminal).is_some());
            assert_eq!(
                events_after(app, 0),
                [
                    EventData::PaneCreated { pane },
                    EventData::LayoutUpdated {
                        layout: app.pane_layout_snapshot(0, 0).unwrap()
                    },
                ]
            );
        },
    );
}

#[test]
fn launch_inputs_reach_the_new_pane() {
    with_live_daemon(
        "pane_split::launch_inputs_reach_the_new_pane",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let scratch = maestro_test_directory::Scratch::new().unwrap();
            let wire = json!({
                "type": "pane.split", "target_pane_id": "w1:p1", "direction": "right",
                "cwd": scratch.path(), "env": {"MAESTRO_TEST_ECHO": "é", "MAESTRO_PANE_ID": "spoofed"},
            });
            let pane = split_of(answer(app, wire).await);
            let (_, created) = app.state.parse_pane_id(&pane.pane_id).unwrap();
            let terminal = app.state.topology.workspaces[0]
                .terminal_id(created)
                .unwrap()
                .clone();
            let script = "printf '\\nseen:<%s>|<%s>|<%s>\\n' \"$MAESTRO_TEST_ECHO\" \"$MAESTRO_PANE_ID\" \"$PWD\"\n";
            let text = run_in_pane(app, &terminal, script, "seen:<").await;
            let expected = format!("seen:<é>|<{}>|<{}>", pane.pane_id, scratch.path().display());
            assert!(text.contains(&expected), "{text:?}");
        },
    );
}

#[test]
fn a_split_that_cannot_start_its_process_leaves_the_tab_alone() {
    with_live_daemon(
        "pane_split::a_split_that_cannot_start_its_process_leaves_the_tab_alone",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let missing = maestro_test_directory::Scratch::new()
                .unwrap()
                .path()
                .join("missing");
            let wire = json!({
                "type": "pane.split", "target_pane_id": "w1:p1", "direction": "right", "cwd": missing
            });
            let Err(error) = answer(app, wire).await else {
                panic!("a missing directory must refuse the split");
            };
            assert_eq!(error.code, "pane_split_failed");
            let workspace = &app.state.topology.workspaces[0];
            assert_eq!(workspace.tabs[0].layout.pane_count(), 1);
            assert_eq!(workspace.tabs[0].panes.len(), 1);
            assert_eq!(workspace.next_public_pane_number, 2);
            assert!(app.terminal_runtimes.is_empty());
            assert!(events_after(app, 0).is_empty());
        },
    );
}

#[test]
fn a_split_of_a_missing_target_is_refused_before_a_process_starts() {
    let mut app = daemon_with_workspaces(&["/one"]);
    for wire in [
        json!({"type": "pane.split", "direction": "right", "target_pane_id": "w1:p9"}),
        json!({"type": "pane.split", "direction": "right", "workspace_id": "w9"}),
        json!({"type": "pane.split", "direction": "right", "workspace_id": "w_9"}),
    ] {
        assert_eq!(
            request(&mut app, wire.clone()),
            failure("pane_not_found", "pane not found"),
            "{wire}"
        );
    }
    app.state.active = None;
    let wire = json!({"type": "pane.split", "direction": "right"});
    assert_eq!(
        request(&mut app, wire),
        failure("pane_not_found", "pane not found")
    );
    assert_eq!(
        app.state.topology.workspaces[0].tabs[0].layout.pane_count(),
        1
    );
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn a_split_rejects_a_bad_launch_environment_before_a_process_starts() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let wire = json!({
        "type": "pane.split", "direction": "right", "env": {"BAD=KEY": "value"}
    });
    let Err(error) = request(&mut app, wire) else {
        panic!("a key with an equals sign must be refused");
    };
    assert_eq!(error.code, "invalid_env");
    assert_eq!(
        app.state.topology.workspaces[0].tabs[0].layout.pane_count(),
        1
    );
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn a_pane_split_off_can_be_closed_and_releases_its_process() {
    with_live_daemon(
        "pane_split::a_pane_split_off_can_be_closed_and_releases_its_process",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let wire =
                json!({"type": "pane.split", "target_pane_id": "w1:p1", "direction": "right"});
            let pane = split_of(answer(app, wire).await);
            assert_eq!(app.terminal_runtimes.len(), 1);
            let close = json!({"type": "pane.close", "pane_id": pane.pane_id});
            assert_eq!(answer(app, close).await, Ok(ResponseResult::Ok {}));
            assert!(app.terminal_runtimes.is_empty());
            assert_eq!(
                app.state.topology.workspaces[0].tabs[0].layout.pane_count(),
                1
            );
            assert_eq!(
                event_names(app, 0),
                [
                    "pane_created",
                    "layout_updated",
                    "pane_closed",
                    "layout_updated"
                ]
            );
        },
    );
}
