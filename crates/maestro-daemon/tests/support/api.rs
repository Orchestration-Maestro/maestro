//! A daemon answering workspace commands, with workspaces that need no live process.

use super::{launch::launch_inputs, manifest_cache, populate_terminals, runtime::block_on};
use maestro_daemon::{
    api::event_hub::EventHub, app::App, paths::DaemonPaths, settings::model::Config,
};
use maestro_protocol::{
    events::EventData,
    rpc_types::RpcCommand,
    schema::{ErrorBody, ResponseResult, SplitDirection},
};
use maestro_settings::{InMemorySettingsStorage, SettingsManager};
use maestro_workspace::{
    layout::{PaneId, SplitRequest},
    terminal_id::TerminalId,
    workspace::WorktreeSpaceMembership,
};
use serde_json::Value;
use std::sync::Arc;

/// Daemon owner with a deterministic identity clock.
pub type TestApp = App<fn() -> u128>;

/// A daemon without workspaces, built from `config`.
pub fn daemon_with(config: &Config) -> TestApp {
    let (launch, receiver) = launch_inputs(&[], manifest_cache());
    App::new(
        config,
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    )
}

/// A daemon without workspaces.
pub fn daemon() -> TestApp {
    daemon_with(&Config::default())
}

/// A daemon with one workspace per directory, without processes; the first is active.
pub fn daemon_with_workspaces(cwds: &[&str]) -> TestApp {
    let mut app = daemon();
    add_workspaces(&mut app, cwds);
    app
}

/// Adds one workspace per directory, without processes, activating the first if none is.
pub fn add_workspaces(app: &mut TestApp, cwds: &[&str]) {
    for cwd in cwds {
        app.state
            .topology
            .new_with_tab((*cwd).into(), None, |_, _| Ok(()))
            .unwrap();
    }
    populate_terminals(&mut app.state);
    if app.state.active.is_none() && !app.state.topology.workspaces.is_empty() {
        app.state.active = Some(0);
    }
}

/// Splits `target` in the workspace at `workspace` without a process or a focus change.
pub fn split_pane(
    app: &mut TestApp,
    workspace: usize,
    target: PaneId,
    direction: SplitDirection,
) -> PaneId {
    let (pane, ()) = app
        .state
        .topology
        .split_pane_with_runtime(
            workspace,
            SplitRequest {
                target,
                direction,
                ratio: None,
                focus_new_pane: false,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap();
    populate_terminals(&mut app.state);
    pane
}

/// Adds a tab without a process to the workspace at `workspace` and returns its root pane.
pub fn add_tab(app: &mut TestApp, workspace: usize) -> PaneId {
    let (pane, ()) = app
        .state
        .topology
        .create_tab_with_runtime(workspace, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    populate_terminals(&mut app.state);
    pane
}

/// Joins the workspace at `index` to the repository group `key`.
pub fn join_group(app: &mut TestApp, index: usize, key: &str, linked: bool) {
    app.state.topology.workspaces[index].worktree_space = Some(WorktreeSpaceMembership {
        key: key.into(),
        label: "repo".into(),
        repo_root: "/repo".into(),
        checkout_path: format!("/repo-{index}").into(),
        is_linked_worktree: linked,
    });
}

/// The root pane of the first tab of the workspace at `workspace`.
pub fn root_pane(app: &TestApp, workspace: usize) -> PaneId {
    app.state.topology.workspaces[workspace].tabs[0].root_pane
}

/// Paths below an agent directory nothing reads or writes.
pub fn untouched_paths() -> DaemonPaths {
    DaemonPaths::new("/nonexistent-maestro-agent-dir", None).unwrap()
}

/// A settings owner over an empty in-memory document.
pub fn settings() -> SettingsManager {
    SettingsManager::from_storage(Arc::new(InMemorySettingsStorage::new()))
}

/// Decodes `command` from its wire form, panicking on a malformed fixture.
pub fn command(wire: Value) -> RpcCommand {
    let text = wire.to_string();
    serde_json::from_value(wire).unwrap_or_else(|error| panic!("{text}: {error}"))
}

/// Answers the wire-form `command` through the daemon's handler.
pub fn request(app: &mut TestApp, wire: Value) -> Result<ResponseResult, ErrorBody> {
    block_on(answer(app, wire))
}

/// Answers the wire-form `wire` from inside a running test runtime.
pub async fn answer(app: &mut TestApp, wire: Value) -> Result<ResponseResult, ErrorBody> {
    answer_command(app, command(wire)).await
}

/// Answers a decoded `command` from inside a running test runtime.
pub async fn answer_command(
    app: &mut TestApp,
    command: RpcCommand,
) -> Result<ResponseResult, ErrorBody> {
    let mut settings = settings();
    app.handle_api_request(command, &mut settings, &untouched_paths())
        .await
}

/// The failure with `code` and `message`.
pub fn failure(code: &str, message: &str) -> Result<ResponseResult, ErrorBody> {
    Err(ErrorBody {
        code: code.into(),
        message: message.into(),
    })
}

/// Every event published after `sequence`.
pub fn events_after(app: &TestApp, sequence: u64) -> Vec<EventData> {
    app.event_hub
        .events_after(sequence)
        .into_iter()
        .map(|(_, event)| event)
        .collect()
}

/// The wire names of every event published after `sequence`, in order.
pub fn event_names(app: &TestApp, sequence: u64) -> Vec<String> {
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
pub fn workspace_ids(app: &TestApp) -> Vec<String> {
    app.state
        .topology
        .workspaces
        .iter()
        .map(|workspace| workspace.id.clone())
        .collect()
}

/// Types `script` into the shell of `terminal` and returns the visible text once a line
/// starts with `marker`.
pub async fn run_in_pane(
    app: &TestApp,
    terminal: &TerminalId,
    script: &str,
    marker: &str,
) -> String {
    let runtime = app.terminal_runtimes.get(terminal).unwrap();
    runtime
        .send_bytes(script.as_bytes().to_vec())
        .await
        .unwrap();
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
pub fn with_live_daemon(test: &str, body: impl AsyncFnOnce(&mut TestApp)) {
    use maestro_daemon::settings::model::NewTerminalCwdConfig;
    use maestro_test_process::{TestProcess, run_alone};
    if run_alone(test).unwrap() {
        return;
    }
    TestProcess::named("daemon_events::setup_entry")
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
            let mut app = App::new(
                &config,
                (|| 0) as fn() -> u128,
                launch,
                receiver,
                EventHub::default(),
            );
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
