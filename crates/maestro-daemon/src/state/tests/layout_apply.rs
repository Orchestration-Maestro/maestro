//! Layout apply through the daemon's handler with live processes: replacement, launch inputs,
//! directories, limits, rollback, focus and publication.
#![cfg(target_os = "linux")]
use crate::state::App;
use crate::state::test_support::api::{
    add_tab, add_workspaces, answer, event_names, events_after, failure, root_pane,
    workspace_ids_of,
};
use crate::state::tests::support::{
    layouts::{balanced, chain, shell, sleeper, split},
    live::{wait_for_line, with_live_daemon},
};
use crate::{
    settings::model::NewTerminalCwdConfig,
    state::model::{ActionPaneRecord, Mode},
};
use maestro_protocol::{
    events::EventData,
    schema::{
        ErrorBody, LayoutDescription, LayoutNode, LayoutPane, ResponseResult, SplitDirection,
    },
};
use maestro_test_directory::Scratch;
use maestro_workspace::tiling::PaneId;
use serde_json::{Value, json};
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
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
fn follow(app: &mut App) {
    app.config.terminal.new_cwd = NewTerminalCwdConfig::Follow;
}

/// Records `cwd` as the last known directory of `pane` in the first workspace.
fn set_cwd(app: &mut App, pane: PaneId, cwd: &Path) {
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.state.terminals.get_mut(&terminal).unwrap().cwd = cwd.to_path_buf();
}

/// The directory recorded for the root pane of tab `tab` in the first workspace.
fn root_cwd(app: &App, tab: usize) -> PathBuf {
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
            add_workspaces(app, &["/tabs"]).unwrap();
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
            add_workspaces(app, &["/tabs"]).unwrap();
            let scratch = Scratch::new().unwrap();
            let reporter = install_leaf(scratch.path(), "reporter", "report-environment");
            let leaf = json!({
                "type": "pane", "cwd": scratch.path(), "env": {"ROLE": "tests"},
                "command": [reporter]
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
                Some(vec![reporter.display().to_string()])
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
            add_workspaces(app, &["/tabs"]).unwrap();
            let focused = Scratch::new().unwrap();
            let root = root_pane(app, 0);
            set_cwd(app, root, focused.path());
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
            add_workspaces(app, &["/tabs"]).unwrap();
            let (active, background) = (Scratch::new().unwrap(), Scratch::new().unwrap());
            let root = root_pane(app, 0);
            set_cwd(app, root, active.path());
            let replaced = add_tab(app, 0).unwrap();
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
            add_workspaces(app, &["/tabs"]).unwrap();
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
fn replacing_a_tab_drops_the_action_records_of_its_panes() {
    with_live_daemon(
        "layout_apply::replacing_a_tab_drops_the_action_records_of_its_panes",
        async |app| {
            add_workspaces(app, &["/tabs"]).unwrap();
            let replaced = root_pane(app, 0);
            app.state.action_panes.insert(
                replaced,
                ActionPaneRecord {
                    action_id: "example.layout".into(),
                    entrypoint: "board".into(),
                },
            );
            let wire = apply_wire(&sleeper(), &json!({"tab_id": "w1:t1"}));
            applied(answer(app, wire).await);
            assert!(app.state.action_panes.is_empty());
        },
    );
}

#[test]
fn a_layout_may_hold_sixteen_levels_and_twenty_four_panes() {
    with_live_daemon(
        "layout_apply::a_layout_may_hold_sixteen_levels_and_twenty_four_panes",
        async |app| {
            add_workspaces(app, &["/tabs"]).unwrap();
            for (root, panes) in [(chain(16, &sleeper()), 16), (balanced(24, &sleeper()), 24)] {
                let layout = applied(answer(app, apply_wire(&root, &json!({}))).await);
                assert_eq!(leaves(&layout.root).len(), panes);
            }
        },
    );
}

/// Applies `wire`, which cannot start a process, and checks that the workspace is as it was:
/// one tab holding only `original`, no runtime, no event and no pending save.
async fn assert_rolled_back(app: &mut App, original: PaneId, wire: Value) {
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
            add_workspaces(app, &["/tabs"]).unwrap();
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
            add_workspaces(app, &["/tabs"]).unwrap();
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
            add_workspaces(app, &["/tabs"]).unwrap();
            app.state.topology.workspaces[0].tabs[0].set_custom_name("build".into());
            applied(answer(app, apply_wire(&sleeper(), &json!({"tab_id": "w1:t1"}))).await);
            let name = |app: &App| app.state.topology.workspaces[0].tab_display_name(0);
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
            add_workspaces(app, &["/tabs"]).unwrap();
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
            add_workspaces(app, &["/one", "/two"]).unwrap();
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
            add_workspaces(app, &["/tabs"]).unwrap();
            let root = split(sleeper(), sleeper());
            applied(answer(app, apply_wire(&root, &json!({"tab_id": "w1:t1"}))).await);
            assert_eq!(
                event_names(app, 0).unwrap(),
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
            add_workspaces(app, &["/tabs"]).unwrap();
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

/// Installs the prepared native fixture as `name` in `directory`, selected by a `command` scenario.
fn install_leaf(directory: &Path, name: &str, scenario: &str) -> PathBuf {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .expect("nextest native-fixture setup supplies the artifact");
    let path = directory.join(name);
    maestro_test_process::install_executable(Path::new(&artifact), &path).unwrap();
    let record = json!({"scenario": scenario});
    std::fs::write(
        directory.join(format!("{name}.fixture")),
        format!("command\n{record}"),
    )
    .unwrap();
    path
}

/// Run the layout operation with a checked relay, returning as soon as either fails.
async fn relay_hangup(
    socket: &Path,
    body: impl std::future::Future<Output = ()>,
) -> io::Result<()> {
    let listener = UnixListener::bind(socket)?;
    let (report, result) = tokio::sync::oneshot::channel();
    let relay = std::thread::spawn(move || {
        // A failed body may drop its receiver before the relay ends.
        let _ = report.send(release_after_hangup(&listener));
    });
    tokio::try_join!(
        async {
            result.await.map_err(io::Error::other)??;
            relay
                .join()
                .map_err(|_| io::Error::other("hangup relay panicked"))
        },
        async {
            body.await;
            Ok(())
        }
    )?;
    Ok(())
}

/// Release the one waiting leaf after the one holder reports its hangup, in either arrival order.
fn release_after_hangup(listener: &UnixListener) -> io::Result<()> {
    let mut waiting = None;
    let mut hung_up = false;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut kind = [0];
        stream.read_exact(&mut kind)?;
        match kind {
            [b'h'] => hung_up = true,
            [b'w'] => waiting = Some(stream),
            _ => return Err(io::Error::other("unknown relay announcement")),
        }
        if hung_up && let Some(leaf) = waiting.take() {
            release(leaf);
            return Ok(());
        }
    }
    Err(io::Error::other("hangup relay closed before release"))
}

/// A peer that closes before announcing returns its I/O error to the layout operation.
#[test]
fn hangup_relay_reports_closed_stream() {
    let scratch = Scratch::new().unwrap();
    let socket = scratch.path().join("hangup");
    let error = crate::tests::block_on(relay_hangup(&socket, async {
        drop(UnixStream::connect(&socket).unwrap());
    }))
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

/// Sends the release byte; a leaf that already ended needs none.
fn release(mut leaf: UnixStream) {
    let _ = leaf.write_all(b"x");
}

/// A leaf that holds the hangup, reports `ready` on a line of its own (the launch helper leaves
/// one open) and tells `socket` once the hangup arrives, so the pane takes a while to stop.
fn holds_until_terminated(directory: &Path, socket: &Path) -> Value {
    let holder = install_leaf(directory, "holder", "hold-hangup");
    json!({"type": "pane", "command": [holder, socket]})
}

/// Waits until the focused pane of `layout` has blocked hangup, so shutdown cannot reach it first.
async fn wait_until_ready(app: &App, layout: &LayoutDescription) {
    let (workspace, pane) = app.state.parse_pane_id(&layout.focused_pane_id).unwrap();
    let terminal = app.state.topology.workspaces[workspace]
        .terminal_id(pane)
        .unwrap()
        .clone();
    wait_for_line(app, &terminal, "ready").await;
}

/// A leaf that exits once `socket` releases it.
fn exits_once_released(directory: &Path, socket: &Path) -> Value {
    let waiter = install_leaf(directory, "waiter", "await-release");
    json!({"type": "pane", "command": [waiter, socket]})
}

#[test]
fn a_replacement_that_exits_while_the_replaced_tab_stops_leaves_no_workspace() {
    with_live_daemon(
        "layout_apply::a_replacement_that_exits_while_the_replaced_tab_stops_leaves_no_workspace",
        async |app| {
            add_workspaces(app, &["/tabs"]).unwrap();
            let scratch = Scratch::new().unwrap();
            let socket = scratch.path().join("hangup");
            Box::pin(relay_hangup(&socket, async {
                let holder = apply_wire(
                    &holds_until_terminated(scratch.path(), &socket),
                    &json!({"tab_id": "w1:t1"}),
                );
                let held = applied(answer(app, holder).await);
                wait_until_ready(app, &held).await;
                let replacement = apply_wire(
                    &exits_once_released(scratch.path(), &socket),
                    &json!({"tab_id": "w1:t2"}),
                );
                assert_eq!(
                    answer(app, replacement).await,
                    failure("layout_apply_failed", "new layout tab disappeared")
                );
                assert!(workspace_ids_of(app).await.unwrap().is_empty());
            }))
            .await
            .unwrap();
        },
    );
}

#[test]
fn a_workspace_that_closes_while_the_replaced_tab_stops_does_not_move_the_layout() {
    with_live_daemon(
        "layout_apply::a_workspace_that_closes_while_the_replaced_tab_stops_does_not_move_the_layout",
        async |app| {
            add_workspaces(app, &["/first", "/second", "/third"]).unwrap();
            let scratch = Scratch::new().unwrap();
            let socket = scratch.path().join("hangup");
            Box::pin(relay_hangup(&socket, async {
                let leaving = apply_wire(
                    &exits_once_released(scratch.path(), &socket),
                    &json!({"tab_id": "w1:t1"}),
                );
                applied(answer(app, leaving).await);
                let holder = apply_wire(
                    &holds_until_terminated(scratch.path(), &socket),
                    &json!({"tab_id": "w2:t1"}),
                );
                let held = applied(answer(app, holder).await);
                wait_until_ready(app, &held).await;
                let replacement = apply_wire(&sleeper(), &json!({"tab_id": "w2:t2"}));
                let layout = applied(answer(app, replacement).await);
                assert_eq!(
                    (layout.workspace_id.as_str(), layout.tab_id.as_str()),
                    ("w2", "w2:t3")
                );
                assert_eq!(workspace_ids_of(app).await.unwrap(), ["w2", "w3"]);
                let closed: Vec<_> = events_after(app, 0)
                    .into_iter()
                    .filter_map(|event| match event {
                        EventData::TabClosed {
                            tab_id,
                            workspace_id,
                        } if tab_id == "w2:t2" => Some(workspace_id),
                        _ => None,
                    })
                    .collect();
                assert_eq!(closed, ["w2"]);
            }))
            .await
            .unwrap();
        },
    );
}
