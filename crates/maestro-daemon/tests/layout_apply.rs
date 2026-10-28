//! Layout apply through the daemon's handler with live processes: replacement, launch inputs,
//! directories, limits, rollback, focus and publication.
#![cfg(target_os = "linux")]
use crate::support::{
    api::{
        TestApp, add_tab, add_workspaces, answer, event_names, events_after, root_pane,
        wait_for_line, with_live_daemon,
    },
    layouts::{balanced, chain, shell, sleeper, split},
};
use maestro_daemon::{
    app::state::{Mode, PluginPaneRecord},
    settings::model::NewTerminalCwdConfig,
};
use maestro_protocol::{
    events::EventData,
    schema::{
        ErrorBody, LayoutDescription, LayoutNode, LayoutPane, ResponseResult, SplitDirection,
    },
};
use maestro_test_directory::Scratch;
use maestro_workspace::layout::PaneId;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The layout an apply answers with.
fn applied(result: Result<ResponseResult, ErrorBody>) -> LayoutDescription {
    match result {
        Ok(ResponseResult::LayoutApply { layout }) => layout,
        other => panic!("expected an applied layout: {other:?}"),
    }
}

/// The leaves of an exported tree in layout order.
fn leaves(node: &LayoutNode) -> Vec<&LayoutPane> {
    match node {
        LayoutNode::Pane { pane } => vec![pane],
        LayoutNode::Split { first, second, .. } => {
            let mut all = leaves(first);
            all.extend(leaves(second));
            all
        }
    }
}

/// The request applying `root`, with the members of `extra` beside it.
fn apply_wire(root: &Value, extra: &Value) -> Value {
    let mut wire = json!({"type": "layout.apply", "root": root});
    wire.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    wire
}

/// Makes new panes follow the directory of the pane they come from.
fn follow(app: &mut TestApp) {
    app.config.terminal.new_cwd = NewTerminalCwdConfig::Follow;
}

/// Records `cwd` as the last known directory of `pane` in the first workspace.
fn set_cwd(app: &mut TestApp, pane: PaneId, cwd: &Path) {
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.state.terminals.get_mut(&terminal).unwrap().cwd = cwd.to_path_buf();
}

/// The directory recorded for the root pane of tab `tab` in the first workspace.
fn root_cwd(app: &TestApp, tab: usize) -> PathBuf {
    let tab = &app.state.topology.workspaces[0].tabs[tab];
    app.state.terminals[tab.terminal_id(tab.root_pane).unwrap()]
        .cwd
        .clone()
}

#[test]
fn applying_a_layout_replaces_the_named_tab() {
    with_live_daemon(
        "layout_apply::applying_a_layout_replaces_the_named_tab",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let root = json!({
                "type": "split", "direction": "right", "ratio": 0.7,
                "first": {"type": "pane", "label": "editor"},
                "second": {
                    "type": "pane", "label": "tests", "env": {"ROLE": "tests"},
                    "command": ["/bin/sleep", "60"]
                }
            });
            let wire = apply_wire(
                &root,
                &json!({"tab_id": "w1:t1", "tab_label": "dev", "focus": true}),
            );
            let layout = applied(answer(app, wire).await);
            let workspace = &app.state.topology.workspaces[0];
            assert_eq!(workspace.tabs.len(), 1);
            assert_eq!(workspace.tab_display_name(0).as_deref(), Some("dev"));
            assert_eq!(
                (layout.workspace_id.as_str(), layout.tab_id.as_str()),
                ("w1", "w1:t2")
            );
            assert!(!layout.zoomed);
            assert_eq!(layout.focused_pane_id, "w1:p2");
            let LayoutNode::Split {
                direction, ratio, ..
            } = &layout.root
            else {
                panic!("the applied tree is split");
            };
            assert_eq!(*direction, SplitDirection::Right);
            assert!((ratio - 0.7).abs() < f32::EPSILON);
            let panes = leaves(&layout.root);
            assert_eq!(
                panes
                    .iter()
                    .map(|pane| (pane.pane_id.as_deref(), pane.label.as_deref()))
                    .collect::<Vec<_>>(),
                [
                    (Some("w1:p2"), Some("editor")),
                    (Some("w1:p3"), Some("tests"))
                ]
            );
            assert_eq!(
                panes[1].command,
                Some(vec!["/bin/sleep".to_owned(), "60".to_owned()])
            );
            assert_eq!(app.state.terminals.len(), 2);
            assert_eq!(app.terminal_runtimes.len(), 2);
            assert!(app.state.session_dirty);
        },
    );
}

#[test]
fn a_leaf_command_runs_with_its_directory_and_environment() {
    with_live_daemon(
        "layout_apply::a_leaf_command_runs_with_its_directory_and_environment",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let scratch = Scratch::new().unwrap();
            let script = "printf 'seen:<%s>|<%s>|<%s>\\n' \"$ROLE\" \"$MAESTRO_PANE_ID\" \"$PWD\"; \
                          exec sleep 60";
            let leaf = json!({
                "type": "pane", "cwd": scratch.path(), "env": {"ROLE": "tests"},
                "command": ["/bin/sh", "-c", script]
            });
            let layout = applied(answer(app, apply_wire(&leaf, &json!({}))).await);
            let pane = layout.focused_pane_id.clone();
            let (_, created) = app.state.parse_pane_id(&pane).unwrap();
            let terminal = app.state.topology.workspaces[0]
                .terminal_id(created)
                .unwrap()
                .clone();
            let text = wait_for_line(app, &terminal, "seen:<").await;
            let expected = format!("seen:<tests>|<{pane}>|<{}>", scratch.path().display());
            assert!(text.contains(&expected), "{text:?}");
            assert_eq!(
                app.state.terminals[&terminal].launch_argv,
                Some(vec![
                    "/bin/sh".to_owned(),
                    "-c".to_owned(),
                    script.to_owned()
                ])
            );
        },
    );
}

#[test]
fn a_new_layout_tab_follows_the_directory_of_the_focused_pane() {
    with_live_daemon(
        "layout_apply::a_new_layout_tab_follows_the_directory_of_the_focused_pane",
        async |app| {
            follow(app);
            add_workspaces(app, &["/tabs"]);
            let focused = Scratch::new().unwrap();
            set_cwd(app, root_pane(app, 0), focused.path());
            applied(answer(app, apply_wire(&sleeper(), &json!({}))).await);
            assert_eq!(app.state.topology.workspaces[0].tabs.len(), 2);
            assert_eq!(root_cwd(app, 1), focused.path());
        },
    );
}

#[test]
fn a_replacing_layout_follows_the_directory_of_the_replaced_tab() {
    with_live_daemon(
        "layout_apply::a_replacing_layout_follows_the_directory_of_the_replaced_tab",
        async |app| {
            follow(app);
            add_workspaces(app, &["/tabs"]);
            let (active, background) = (Scratch::new().unwrap(), Scratch::new().unwrap());
            set_cwd(app, root_pane(app, 0), active.path());
            let replaced = add_tab(app, 0);
            set_cwd(app, replaced, background.path());
            let wire = apply_wire(&sleeper(), &json!({"tab_id": "w1:t2"}));
            applied(answer(app, wire).await);
            assert_eq!(app.state.topology.workspaces[0].tabs.len(), 2);
            assert_eq!(root_cwd(app, 1), background.path());
        },
    );
}

#[test]
fn a_leaf_without_a_directory_starts_where_the_pane_it_splits_runs() {
    with_live_daemon(
        "layout_apply::a_leaf_without_a_directory_starts_where_the_pane_it_splits_runs",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let (first, other) = (Scratch::new().unwrap(), Scratch::new().unwrap());
            let root = split(
                json!({"type": "pane", "cwd": first.path(), "command": ["/bin/sleep", "60"]}),
                split(
                    sleeper(),
                    json!({"type": "pane", "cwd": other.path(), "command": ["/bin/sleep", "60"]}),
                ),
            );
            let layout = applied(answer(app, apply_wire(&root, &json!({}))).await);
            let cwds: Vec<_> = leaves(&layout.root)
                .iter()
                .map(|pane| pane.cwd.clone().map(PathBuf::from))
                .collect();
            assert_eq!(
                cwds,
                [
                    Some(first.path().to_path_buf()),
                    Some(first.path().to_path_buf()),
                    Some(other.path().to_path_buf())
                ]
            );
        },
    );
}

#[test]
fn replacing_a_tab_drops_the_plugin_records_of_its_panes() {
    with_live_daemon(
        "layout_apply::replacing_a_tab_drops_the_plugin_records_of_its_panes",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let replaced = root_pane(app, 0);
            app.state.plugin_panes.insert(
                replaced,
                PluginPaneRecord {
                    plugin_id: "example.layout".into(),
                    entrypoint: "board".into(),
                },
            );
            let wire = apply_wire(&sleeper(), &json!({"tab_id": "w1:t1"}));
            applied(answer(app, wire).await);
            assert!(app.state.plugin_panes.is_empty());
        },
    );
}

#[test]
fn a_layout_may_hold_sixteen_levels_and_twenty_four_panes() {
    with_live_daemon(
        "layout_apply::a_layout_may_hold_sixteen_levels_and_twenty_four_panes",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            for (root, panes) in [(chain(16, &sleeper()), 16), (balanced(24, &sleeper()), 24)] {
                let layout = applied(answer(app, apply_wire(&root, &json!({}))).await);
                assert_eq!(leaves(&layout.root).len(), panes);
            }
        },
    );
}

/// Applies `wire`, which cannot start a process, and checks that the workspace is as it was:
/// one tab holding only `original`, no runtime, no event and no pending save.
async fn assert_rolled_back(app: &mut TestApp, original: PaneId, wire: Value) {
    let Err(error) = answer(app, wire.clone()).await else {
        panic!("a missing directory must refuse the layout: {wire}");
    };
    assert_eq!(error.code, "layout_apply_failed", "{wire}");
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.tabs.len(), 1);
    assert_eq!(workspace.tabs[0].root_pane, original);
    assert_eq!(workspace.tabs[0].layout.pane_count(), 1);
    assert_eq!(app.state.terminals.len(), 1);
    assert!(app.terminal_runtimes.is_empty());
    assert!(events_after(app, 0).is_empty());
    assert!(!app.state.session_dirty);
}

#[test]
fn a_leaf_that_cannot_start_rolls_the_whole_layout_back() {
    with_live_daemon(
        "layout_apply::a_leaf_that_cannot_start_rolls_the_whole_layout_back",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let original = root_pane(app, 0);
            let missing = Scratch::new().unwrap().path().join("missing");
            let broken = json!({"type": "pane", "cwd": missing, "command": ["/bin/sleep", "60"]});
            let trees = [
                split(sleeper(), broken.clone()),
                split(sleeper(), split(sleeper(), broken.clone())),
                broken,
            ];
            for root in trees {
                for extra in [json!({}), json!({"tab_id": "w1:t1"})] {
                    assert_rolled_back(app, original, apply_wire(&root, &extra)).await;
                }
            }
        },
    );
}

#[test]
fn a_layout_activates_its_tab_when_asked_or_when_it_replaces_the_active_tab() {
    with_live_daemon(
        "layout_apply::a_layout_activates_its_tab_when_asked_or_when_it_replaces_the_active_tab",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            app.state.mode = Mode::Navigate;
            applied(answer(app, apply_wire(&sleeper(), &json!({}))).await);
            assert_eq!(app.state.topology.workspaces[0].active_tab, 0);
            assert_eq!(app.state.mode, Mode::Navigate);
            let wire = apply_wire(&sleeper(), &json!({"focus": true}));
            applied(answer(app, wire).await);
            assert_eq!(app.state.topology.workspaces[0].active_tab, 2);
            assert_eq!(app.state.mode, Mode::Terminal);
            app.state.mode = Mode::Navigate;
            let wire = apply_wire(&sleeper(), &json!({"tab_id": "w1:t3"}));
            applied(answer(app, wire).await);
            let workspace = &app.state.topology.workspaces[0];
            assert_eq!((workspace.tabs.len(), workspace.active_tab), (3, 2));
            assert_eq!(app.state.mode, Mode::Terminal);
        },
    );
}

#[test]
fn a_replaced_tab_keeps_its_name_unless_the_layout_names_one() {
    with_live_daemon(
        "layout_apply::a_replaced_tab_keeps_its_name_unless_the_layout_names_one",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            app.state.topology.workspaces[0].tabs[0].set_custom_name("build".into());
            applied(answer(app, apply_wire(&sleeper(), &json!({"tab_id": "w1:t1"}))).await);
            let name = |app: &TestApp| app.state.topology.workspaces[0].tab_display_name(0);
            assert_eq!(name(app).as_deref(), Some("build"));
            let wire = apply_wire(&sleeper(), &json!({"tab_id": "w1:t2", "tab_label": "dev"}));
            applied(answer(app, wire).await);
            assert_eq!(name(app).as_deref(), Some("dev"));
            applied(answer(app, apply_wire(&sleeper(), &json!({}))).await);
            let workspace = &app.state.topology.workspaces[0];
            assert_eq!(workspace.tab_display_name(1).as_deref(), Some("2"));
        },
    );
}

#[test]
fn a_leaf_label_is_trimmed_and_a_blank_one_is_ignored() {
    with_live_daemon(
        "layout_apply::a_leaf_label_is_trimmed_and_a_blank_one_is_ignored",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let root = split(
                json!({"type": "pane", "label": "  editor  ", "command": ["/bin/sleep", "60"]}),
                json!({"type": "pane", "label": "   ", "command": ["/bin/sleep", "60"]}),
            );
            let layout = applied(answer(app, apply_wire(&root, &json!({}))).await);
            let labels: Vec<_> = leaves(&layout.root)
                .iter()
                .map(|pane| pane.label.clone())
                .collect();
            assert_eq!(labels, [Some("editor".to_owned()), None]);
        },
    );
}

#[test]
fn a_layout_is_applied_to_the_named_workspace() {
    with_live_daemon(
        "layout_apply::a_layout_is_applied_to_the_named_workspace",
        async |app| {
            add_workspaces(app, &["/one", "/two"]);
            let wire = apply_wire(&sleeper(), &json!({"workspace_id": "w2"}));
            let layout = applied(answer(app, wire).await);
            assert_eq!(layout.workspace_id, "w2");
            let lengths: Vec<_> = app
                .state
                .topology
                .workspaces
                .iter()
                .map(|workspace| workspace.tabs.len())
                .collect();
            assert_eq!(lengths, [1, 2]);
            assert_eq!(app.state.active, Some(0));
        },
    );
}

#[test]
fn applying_a_layout_publishes_the_closed_tab_the_new_tab_and_every_pane() {
    with_live_daemon(
        "layout_apply::applying_a_layout_publishes_the_closed_tab_the_new_tab_and_every_pane",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let root = split(sleeper(), sleeper());
            applied(answer(app, apply_wire(&root, &json!({"tab_id": "w1:t1"}))).await);
            assert_eq!(
                event_names(app, 0),
                [
                    "tab_closed",
                    "tab_created",
                    "pane_created",
                    "pane_created",
                    "layout_updated"
                ]
            );
            let events = events_after(app, 0);
            assert!(matches!(
                &events[0],
                EventData::TabClosed { tab_id, workspace_id }
                    if (tab_id.as_str(), workspace_id.as_str()) == ("w1:t1", "w1")
            ));
            assert!(matches!(
                &events[1],
                EventData::TabCreated { tab } if tab.tab_id == "w1:t2" && tab.pane_count == 2
            ));
            let created: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    EventData::PaneCreated { pane } => Some(pane.pane_id.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(created, ["w1:p2", "w1:p3"]);
            assert!(matches!(
                &events[4],
                EventData::LayoutUpdated { layout }
                    if layout.tab_id == "w1:t2" && layout.panes.len() == 2
            ));
        },
    );
}

#[test]
fn shell_leaves_start_the_configured_shell_without_a_launch_command() {
    with_live_daemon(
        "layout_apply::shell_leaves_start_the_configured_shell_without_a_launch_command",
        async |app| {
            add_workspaces(app, &["/tabs"]);
            let layout =
                applied(answer(app, apply_wire(&split(shell(), shell()), &json!({}))).await);
            assert!(
                leaves(&layout.root)
                    .iter()
                    .all(|pane| pane.command.is_none())
            );
            assert_eq!(app.terminal_runtimes.len(), 2);
        },
    );
}
