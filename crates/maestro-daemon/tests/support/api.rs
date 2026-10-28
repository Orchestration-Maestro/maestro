//! A daemon answering workspace commands, with workspaces that need no live process.

use super::{launch::launch_inputs, manifest_cache, populate_terminals, runtime::block_on};
use maestro_daemon::{
    api::event_hub::EventHub, app::App, paths::DaemonPaths, settings::model::Config,
};
use maestro_protocol::{
    events::EventData,
    rpc_types::RpcCommand,
    schema::{ErrorBody, ResponseResult},
};
use maestro_settings::{InMemorySettingsStorage, SettingsManager};
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
    let mut settings = settings();
    block_on(app.handle_api_request(command(wire), &mut settings, &untouched_paths()))
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

/// The stable identities of the workspaces in display order.
pub fn workspace_ids(app: &TestApp) -> Vec<String> {
    app.state
        .topology
        .workspaces
        .iter()
        .map(|workspace| workspace.id.clone())
        .collect()
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
