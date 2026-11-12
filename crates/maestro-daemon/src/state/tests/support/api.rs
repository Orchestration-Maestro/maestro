//! A daemon answering workspace commands, with workspaces that need no live process.

use super::{manifest_cache, spawning::launch_inputs};
use crate::tests::block_on;
use crate::{
    events::AppEvent, locations::DaemonPaths, service::event_hub::EventHub,
    settings::model::Config, state::App,
};
use maestro_detection::agents::{Agent, AgentState};
use maestro_protocol::{
    events::EventData,
    rpc_types::RpcCommand,
    schema::{
        ErrorBody, LayoutDescription, PaneInfo, PaneLayoutSnapshot, ResponseResult, SplitDirection,
    },
};
use maestro_settings::{InMemorySettingsStorage, SettingsManager};
use maestro_workspace::{
    spaces::WorktreeSpaceMembership,
    terminal_id::TerminalId,
    tiling::{PaneId, SplitRequest},
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};

/// A daemon without workspaces, built from `config`.
pub(crate) fn daemon_with(config: &Config) -> App {
    let (launch, receiver) = launch_inputs(&[], manifest_cache());
    App::new(config, || 0, launch, receiver, EventHub::default())
}

/// A daemon without workspaces.
pub(crate) fn daemon() -> App {
    daemon_with(&Config::default())
}

/// A daemon with one workspace per directory, without processes; the first is active.
pub(crate) fn daemon_with_workspaces(cwds: &[&str]) -> App {
    let mut app = daemon();
    add_workspaces(&mut app, cwds);
    app
}

/// Adds one workspace per directory, without processes, activating the first if none is.
pub(crate) fn add_workspaces(app: &mut App, cwds: &[&str]) {
    for cwd in cwds {
        app.seed_workspace(cwd).unwrap();
    }
    app.state.session_dirty = false;
}

/// Splits `target` in the workspace at `workspace` without a process or a focus change.
pub(crate) fn split_pane(
    app: &mut App,
    workspace: usize,
    target: PaneId,
    direction: SplitDirection,
) -> PaneId {
    split_pane_focused(app, workspace, target, direction, false)
}

/// Splits `target` in the workspace at `workspace` without a process, optionally focusing
/// the new pane.
pub(crate) fn split_pane_focused(
    app: &mut App,
    workspace: usize,
    target: PaneId,
    direction: SplitDirection,
    focus_new_pane: bool,
) -> PaneId {
    let request = SplitRequest {
        target,
        direction,
        ratio: None,
        focus_new_pane,
    };
    let pane = app.seed_split(workspace, request).unwrap();
    app.state.session_dirty = false;
    pane
}

/// Adds a tab without a process to the workspace at `workspace` and returns its root pane.
pub(crate) fn add_tab(app: &mut App, workspace: usize) -> PaneId {
    let pane = app.seed_tab(workspace).unwrap();
    app.state.session_dirty = false;
    pane
}

/// Joins the workspace at `index` to the repository group `key`.
pub(crate) fn join_group(app: &mut App, index: usize, key: &str, linked: bool) {
    app.seed_worktree_membership(
        index,
        WorktreeSpaceMembership {
            key: key.into(),
            label: "repo".into(),
            repo_root: "/repo".into(),
            checkout_path: format!("/repo-{index}").into(),
            is_linked_worktree: linked,
        },
    );
    app.state.session_dirty = false;
}

/// Reports that fallback detection saw `agent` in `state` on the first pane of the first
/// workspace.
pub(crate) fn detect(app: &mut App, agent: Agent, state: AgentState) {
    let pane_id = root_pane(app, 0);
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(agent),
        state,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: Instant::now(),
    });
}

/// Gives the first pane of the first workspace a recognized agent process with fallback `state`.
pub(crate) fn pilot(app: &mut App, state: AgentState) {
    let pane_id = root_pane(app, 0);
    app.handle_internal_event(AppEvent::AgentProcessDetected {
        pane_id,
        agent: Agent::Pi,
        observed_at: Instant::now(),
    });
    detect(app, Agent::Pi, state);
}

/// The root pane of the first tab of the workspace at `workspace`.
pub(crate) fn root_pane(app: &App, workspace: usize) -> PaneId {
    app.state.topology.workspaces[workspace].tabs[0].root_pane
}

/// The layout `layout.export` answers with for the active tab, or for `tab_id`.
pub(crate) fn exported_layout(app: &mut App, tab_id: Option<&str>) -> LayoutDescription {
    let wire = tab_id.map_or(
        json!({"type": "layout.export"}),
        |tab| json!({"type": "layout.export", "tab_id": tab}),
    );
    match request(app, wire) {
        Ok(ResponseResult::LayoutExport { layout }) => layout,
        other => panic!("expected an exported layout: {other:?}"),
    }
}

/// The geometry `pane.layout` answers with for the active tab.
pub(crate) fn pane_layout(app: &mut App) -> PaneLayoutSnapshot {
    match request(app, json!({"type": "pane.layout"})) {
        Ok(ResponseResult::PaneLayout { layout }) => layout,
        other => panic!("expected a pane layout: {other:?}"),
    }
}

/// The handle of the pane that holds the focus in the active tab of the workspace at
/// `workspace`.
pub(crate) fn focused_pane_id(app: &mut App, workspace: usize) -> String {
    let id = workspace_ids(app).swap_remove(workspace);
    let Ok(ResponseResult::WorkspaceInfo { workspace }) =
        request(app, json!({"type": "workspace.get", "workspace_id": id}))
    else {
        panic!("expected the workspace");
    };
    exported_layout(app, Some(&workspace.active_tab_id)).focused_pane_id
}

/// A daemon with one workspace whose first tab holds a pane on the left and one on the right.
pub(crate) fn pair() -> (App, PaneId, PaneId) {
    let mut app = daemon_with_workspaces(&["/one"]);
    let left = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, left, SplitDirection::Right);
    (app, left, right)
}

/// Paths below an agent directory nothing reads or writes.
pub(crate) fn untouched_paths() -> DaemonPaths {
    DaemonPaths::new("/nonexistent-maestro-agent-dir", None).unwrap()
}

/// A settings owner over an empty in-memory document.
pub(crate) fn settings() -> SettingsManager {
    SettingsManager::from_storage(Arc::new(InMemorySettingsStorage::new()))
}

/// Decodes `command` from its wire form, panicking on a malformed fixture.
pub(crate) fn command(wire: Value) -> RpcCommand {
    let text = wire.to_string();
    serde_json::from_value(wire).unwrap_or_else(|error| panic!("{text}: {error}"))
}

/// Answers `wire`, which must succeed with the plain acknowledgement.
pub(crate) fn accepted(app: &mut App, wire: Value) {
    let text = wire.to_string();
    assert_eq!(request(app, wire), Ok(ResponseResult::Ok {}), "{text}");
}

/// The pane record `pane.get` returns for the pane the workspace owns.
pub(crate) fn pane_record(app: &mut App, pane: &str) -> PaneInfo {
    match request(app, json!({"type": "pane.get", "pane_id": pane})) {
        Ok(ResponseResult::PaneInfo { pane }) => pane,
        other => panic!("expected a pane: {other:?}"),
    }
}

/// Answers the wire-form `command` through the daemon's handler.
pub(crate) fn request(app: &mut App, wire: Value) -> Result<ResponseResult, ErrorBody> {
    block_on(answer(app, wire))
}

/// Answers `wire` through the handler from inside a running test runtime.
pub(crate) async fn answer(app: &mut App, wire: Value) -> Result<ResponseResult, ErrorBody> {
    answer_command(app, command(wire)).await
}

/// Answers a decoded `command` from inside a running test runtime.
pub(crate) async fn answer_command(
    app: &mut App,
    command: RpcCommand,
) -> Result<ResponseResult, ErrorBody> {
    let mut settings = settings();
    let paths = untouched_paths();
    app.handle_api_request(command, &mut settings, &paths).await
}

/// The failure with `code` and `message`.
pub(crate) fn failure(code: &str, message: &str) -> Result<ResponseResult, ErrorBody> {
    Err(ErrorBody {
        code: code.into(),
        message: message.into(),
    })
}

/// Every event published after `sequence`.
pub(crate) fn events_after(app: &App, sequence: u64) -> Vec<EventData> {
    app.event_hub()
        .events_after(sequence)
        .into_iter()
        .map(|(_, event)| event)
        .collect()
}

/// The wire names of every event published after `sequence`, in order.
pub(crate) fn event_names(app: &App, sequence: u64) -> Vec<String> {
    events_after(app, sequence)
        .iter()
        .map(|event| {
            serde_json::to_value(event).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// The stable identities of the workspaces in display order.
pub(crate) fn workspace_ids(app: &mut App) -> Vec<String> {
    block_on(workspace_ids_of(app))
}

/// The stable identities of the workspaces in display order, from inside a running test
/// runtime.
pub(crate) async fn workspace_ids_of(app: &mut App) -> Vec<String> {
    let Ok(ResponseResult::WorkspaceList { workspaces }) =
        answer(app, json!({"type": "workspace.list"})).await
    else {
        panic!("expected the workspaces");
    };
    workspaces
        .into_iter()
        .map(|workspace| workspace.workspace_id)
        .collect()
}

/// Types `script` into the shell of `terminal` and returns the visible text once a line
/// starts with `marker`.
pub(crate) async fn run_in_pane(
    app: &App,
    terminal: &TerminalId,
    script: &str,
    marker: &str,
) -> String {
    let runtime = app.terminal_runtimes.get(terminal).unwrap();
    runtime
        .send_bytes(script.as_bytes().to_vec())
        .await
        .unwrap();
    wait_for_line(app, terminal, marker).await
}

/// The visible text of `terminal` once a line starts with `marker`.
pub(crate) async fn wait_for_line(app: &App, terminal: &TerminalId, marker: &str) -> String {
    let runtime = app.terminal_runtimes.get(terminal).unwrap();
    loop {
        let _ = app.launch.render_dirty.take();
        let text = runtime.visible_text();
        if text.lines().any(|line| line.starts_with(marker)) {
            return text;
        }
        app.launch.render_notify.notified().await;
    }
}

/// Runs `body` alone in a process of its own, against a daemon whose panes start real
/// `/bin/sh` shells in the temporary root, then stops every pane it started.
pub(crate) fn with_live_daemon(test: &str, body: impl AsyncFnOnce(&mut App)) {
    use crate::settings::model::NewTerminalCwdConfig;
    use maestro_test_process::{TestProcess, run_alone};
    if run_alone(test).unwrap() {
        return;
    }
    TestProcess::named("state::tests::daemon_events::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            let mut config = Config::default();
            config.terminal.default_shell = "/bin/sh".into();
            config.terminal.new_cwd = NewTerminalCwdConfig::Path(
                maestro_test_directory::temporary_root()
                    .display()
                    .to_string(),
            );
            let (launch, receiver) = launch_inputs(args, manifest_cache());
            let mut app = App::new(&config, || 0, launch, receiver, EventHub::default());
            block_on(async {
                body(&mut app).await;
                for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
                    runtime.shutdown().await;
                }
            });
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}
