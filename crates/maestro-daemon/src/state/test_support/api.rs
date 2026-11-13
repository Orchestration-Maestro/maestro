//! A daemon answering workspace commands, with workspaces that need no live process.

use super::{manifest_cache, run, spawning::launch_inputs};
use crate::{
    events::AppEvent, locations::DaemonPaths, service::event_hub::EventHub,
    settings::model::Config, state::App,
};
use maestro_detection::agents::{Agent, AgentState};
use maestro_protocol::{
    envelopes::RpcCommand,
    events::EventData,
    schema::{
        ErrorBody, LayoutDescription, PaneInfo, PaneLayoutSnapshot, ResponseResult, SplitDirection,
    },
};
use maestro_settings::{InMemorySettingsStorage, SettingsManager};
use maestro_workspace::{
    spaces::WorktreeSpaceMembership,
    tiling::{PaneId, SplitRequest},
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};

/// A daemon without workspaces, built from `config`.
///
/// # Errors
/// Returns why the launch inputs could not be built.
pub fn daemon_with(config: &Config) -> Result<App, String> {
    daemon_with_args(config, &[])
}

/// A daemon without workspaces, built from `config`, whose panes start the setup entry with
/// the harness arguments `args`.
///
/// # Errors
/// Returns why the launch inputs could not be built.
pub fn daemon_with_args(config: &Config, args: &[String]) -> Result<App, String> {
    let (launch, receiver) = launch_inputs(args, manifest_cache()?)?;
    Ok(App::new(
        config,
        || 0,
        launch,
        receiver,
        EventHub::default(),
    ))
}

/// A daemon without workspaces.
///
/// # Errors
/// Returns why the launch inputs could not be built.
pub fn daemon() -> Result<App, String> {
    daemon_with(&Config::default())
}

/// A daemon with one workspace per directory, without processes; the first is active.
///
/// # Errors
/// Returns why the daemon or a workspace could not be built.
pub fn daemon_with_workspaces(cwds: &[&str]) -> Result<App, String> {
    let mut app = daemon()?;
    add_workspaces(&mut app, cwds)?;
    Ok(app)
}

/// Adds one workspace per directory, without processes, activating the first if none is.
///
/// # Errors
/// Returns the directory that could not be added.
pub fn add_workspaces(app: &mut App, cwds: &[&str]) -> Result<(), String> {
    for cwd in cwds {
        app.seed_workspace(cwd)
            .ok_or_else(|| format!("workspace in {cwd} was not added"))?;
    }
    app.state.session_dirty = false;
    Ok(())
}

/// Splits `target` in the workspace at `workspace` without a process or a focus change.
pub fn split_pane(
    app: &mut App,
    workspace: usize,
    target: PaneId,
    direction: SplitDirection,
) -> Option<PaneId> {
    split_pane_focused(app, workspace, target, direction, false)
}

/// Splits `target` in the workspace at `workspace` without a process, optionally focusing
/// the new pane.
pub fn split_pane_focused(
    app: &mut App,
    workspace: usize,
    target: PaneId,
    direction: SplitDirection,
    focus_new_pane: bool,
) -> Option<PaneId> {
    let request = SplitRequest {
        target,
        direction,
        ratio: None,
        focus_new_pane,
    };
    let pane = app.seed_split(workspace, request)?;
    app.state.session_dirty = false;
    Some(pane)
}

/// Adds a tab without a process to the workspace at `workspace` and returns its root pane.
pub fn add_tab(app: &mut App, workspace: usize) -> Option<PaneId> {
    let pane = app.seed_tab(workspace)?;
    app.state.session_dirty = false;
    Some(pane)
}

/// Joins the workspace at `index` to the repository group `key`.
pub fn join_group(app: &mut App, index: usize, key: &str, linked: bool) {
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
/// workspace. Returns `None` when there is no such pane.
pub fn detect(app: &mut App, agent: Agent, state: AgentState) -> Option<()> {
    let pane_id = root_pane(app, 0)?;
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(agent),
        state,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: Instant::now(),
    });
    Some(())
}

/// Gives the first pane of the first workspace a recognized agent process with fallback `state`.
/// Returns `None` when there is no such pane.
pub fn pilot(app: &mut App, state: AgentState) -> Option<()> {
    let pane_id = root_pane(app, 0)?;
    app.handle_internal_event(AppEvent::AgentProcessDetected {
        pane_id,
        agent: Agent::Pi,
        observed_at: Instant::now(),
    });
    detect(app, Agent::Pi, state)
}

/// The root pane of the first tab of the workspace at `workspace`.
#[must_use]
pub fn root_pane(app: &App, workspace: usize) -> Option<PaneId> {
    let tab = app.state.topology.workspaces.get(workspace)?.tabs.first()?;
    Some(tab.root_pane)
}

/// The layout `layout.export` answers with for the active tab, or for `tab_id`.
///
/// # Errors
/// Returns the answer that was not an exported layout.
pub fn exported_layout(app: &mut App, tab_id: Option<&str>) -> Result<LayoutDescription, String> {
    let wire = tab_id.map_or(
        json!({"type": "layout.export"}),
        |tab| json!({"type": "layout.export", "tab_id": tab}),
    );
    match request(app, wire) {
        Ok(ResponseResult::LayoutExport { layout }) => Ok(layout),
        other => Err(format!("expected an exported layout: {other:?}")),
    }
}

/// The geometry `pane.layout` answers with for the active tab.
///
/// # Errors
/// Returns the answer that was not a pane layout.
pub fn pane_layout(app: &mut App) -> Result<PaneLayoutSnapshot, String> {
    match request(app, json!({"type": "pane.layout"})) {
        Ok(ResponseResult::PaneLayout { layout }) => Ok(layout),
        other => Err(format!("expected a pane layout: {other:?}")),
    }
}

/// The handle of the pane that holds the focus in the active tab of the workspace at
/// `workspace`.
///
/// # Errors
/// Returns the answer that did not name the workspace or its layout.
pub fn focused_pane_id(app: &mut App, workspace: usize) -> Result<String, String> {
    let mut ids = workspace_ids(app)?;
    if workspace >= ids.len() {
        return Err(format!("no workspace at {workspace}"));
    }
    let id = ids.swap_remove(workspace);
    let Ok(ResponseResult::WorkspaceInfo { workspace }) =
        request(app, json!({"type": "workspace.get", "workspace_id": id}))
    else {
        return Err("expected the workspace".into());
    };
    Ok(exported_layout(app, Some(&workspace.active_tab_id))?.focused_pane_id)
}

/// A daemon with one workspace whose first tab holds a pane on the left and one on the right.
///
/// # Errors
/// Returns why the daemon or the split could not be built.
pub fn pair() -> Result<(App, PaneId, PaneId), String> {
    let mut app = daemon_with_workspaces(&["/one"])?;
    let left = root_pane(&app, 0).ok_or("the daemon has no first pane")?;
    let right = split_pane(&mut app, 0, left, SplitDirection::Right)
        .ok_or("the first pane was not split")?;
    Ok((app, left, right))
}

/// Paths below an agent directory nothing reads or writes.
///
/// # Errors
/// Returns the diagnostic for an invalid daemon target.
pub fn untouched_paths() -> Result<DaemonPaths, String> {
    DaemonPaths::new("/nonexistent-maestro-agent-dir", None)
}

/// A settings owner over an empty in-memory document.
#[must_use]
pub fn settings() -> SettingsManager {
    SettingsManager::from_storage(Arc::new(InMemorySettingsStorage::new()))
}

/// Decodes `command` from its wire form.
///
/// # Errors
/// Returns the wire text and the decoding error of a malformed fixture.
pub fn command(wire: Value) -> Result<RpcCommand, String> {
    let text = wire.to_string();
    serde_json::from_value(wire).map_err(|error| format!("{text}: {error}"))
}

/// Answers `wire`, which must succeed with the plain acknowledgement.
///
/// # Errors
/// Returns the wire text and the answer that was not the plain acknowledgement.
pub fn accepted(app: &mut App, wire: Value) -> Result<(), String> {
    let text = wire.to_string();
    match request(app, wire) {
        Ok(ResponseResult::Ok {}) => Ok(()),
        other => Err(format!("{text}: {other:?}")),
    }
}

/// The pane record `pane.get` returns for the pane the workspace owns.
///
/// # Errors
/// Returns the answer that was not a pane record.
pub fn pane_record(app: &mut App, pane: &str) -> Result<PaneInfo, String> {
    match request(app, json!({"type": "pane.get", "pane_id": pane})) {
        Ok(ResponseResult::PaneInfo { pane }) => Ok(pane),
        other => Err(format!("expected a pane: {other:?}")),
    }
}

/// Answers the wire-form `command` through the daemon's handler.
///
/// # Errors
/// Returns the handler's refusal, or a fixture failure when the wire form or the runtime is unusable.
pub fn request(app: &mut App, wire: Value) -> Result<ResponseResult, ErrorBody> {
    run(answer(app, wire)).unwrap_or_else(|error| Err(fixture_failure(&error.to_string())))
}

/// The failure that reports a fixture the test could not set up.
fn fixture_failure(message: &str) -> ErrorBody {
    ErrorBody {
        code: "fixture_failure".into(),
        message: message.into(),
    }
}

/// Answers `wire` through the handler from inside a running test runtime.
///
/// # Errors
/// Returns the handler's refusal, or a fixture failure when the wire form is unusable.
pub async fn answer(app: &mut App, wire: Value) -> Result<ResponseResult, ErrorBody> {
    match command(wire) {
        Ok(command) => answer_command(app, command).await,
        Err(message) => Err(fixture_failure(&message)),
    }
}

/// Answers a decoded `command` from inside a running test runtime.
///
/// # Errors
/// Returns the handler's refusal, or a fixture failure when the paths are unusable.
pub async fn answer_command(
    app: &mut App,
    command: RpcCommand,
) -> Result<ResponseResult, ErrorBody> {
    let mut settings = settings();
    let paths = untouched_paths().map_err(|message| fixture_failure(&message))?;
    app.handle_api_request(command, &mut settings, &paths).await
}

/// The failure with `code` and `message`.
///
/// # Errors
/// Always: the result is the failure itself, for comparison with an answer.
pub fn failure(code: &str, message: &str) -> Result<ResponseResult, ErrorBody> {
    Err(ErrorBody {
        code: code.into(),
        message: message.into(),
    })
}

/// Every event published after `sequence`.
#[must_use]
pub fn events_after(app: &App, sequence: u64) -> Vec<EventData> {
    app.event_hub()
        .events_after(sequence)
        .into_iter()
        .map(|(_, event)| event)
        .collect()
}

/// The wire names of every event published after `sequence`, in order.
///
/// # Errors
/// Returns the event that has no wire name.
pub fn event_names(app: &App, sequence: u64) -> Result<Vec<String>, String> {
    events_after(app, sequence)
        .iter()
        .map(|event| {
            serde_json::to_value(event)
                .ok()
                .and_then(|wire| wire["type"].as_str().map(str::to_owned))
                .ok_or_else(|| format!("event without a wire name: {event:?}"))
        })
        .collect()
}

/// The stable identities of the workspaces in display order.
///
/// # Errors
/// Returns the answer that was not a workspace list.
pub fn workspace_ids(app: &mut App) -> Result<Vec<String>, String> {
    run(workspace_ids_of(app)).map_err(|error| error.to_string())?
}

/// The stable identities of the workspaces in display order, from inside a running test
/// runtime.
///
/// # Errors
/// Returns the answer that was not a workspace list.
pub async fn workspace_ids_of(app: &mut App) -> Result<Vec<String>, String> {
    let Ok(ResponseResult::WorkspaceList { workspaces }) =
        answer(app, json!({"type": "workspace.list"})).await
    else {
        return Err("expected the workspaces".into());
    };
    Ok(workspaces
        .into_iter()
        .map(|workspace| workspace.workspace_id)
        .collect())
}
