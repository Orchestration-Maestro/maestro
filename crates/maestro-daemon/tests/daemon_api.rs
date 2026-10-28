//! The daemon's workspace command handler and its arm in the daemon loop.
#![cfg(target_os = "linux")]
use crate::support::{launch::launch_inputs, manifest_cache, runtime::block_on};
use maestro_daemon::api::{RequestMessage, event_hub::EventHub};
use maestro_daemon::app::App;
use maestro_daemon::paths::DaemonPaths;
use maestro_daemon::settings::model::Config;
use maestro_protocol::rpc_types::RpcCommand;
use maestro_protocol::schema::{EmptyParams, ErrorBody, ResponseResult};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};

/// A daemon without workspaces.
fn daemon() -> App<fn() -> u128> {
    let (launch, receiver) = launch_inputs(&[], manifest_cache());
    App::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    )
}

#[test]
fn daemon_server_stop_request_sets_should_quit_flag() {
    let mut app = daemon();
    let stopped = block_on(app.handle_api_request(RpcCommand::ServerStop(EmptyParams {})));
    assert_eq!(stopped, Ok(ResponseResult::Ok {}));
    assert!(app.state.should_quit.0);

    // A stop arriving through the loop's request arm ends the run without its shutdown.
    let mut app = daemon();
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let paths = DaemonPaths::new(directory.path().to_str().unwrap(), None).unwrap();
    let (sender, mut requests) = mpsc::unbounded_channel();
    let (respond_to, mut response) = oneshot::channel();
    let (_attempted, response_write_complete) = oneshot::channel();
    sender
        .send(RequestMessage {
            command: RpcCommand::ServerStop(EmptyParams {}),
            respond_to,
            response_write_complete,
        })
        .unwrap();
    block_on(app.run(&paths, &mut requests, std::future::pending()));
    assert_eq!(response.try_recv(), Ok(Ok(ResponseResult::Ok {})));
    assert!(app.state.should_quit.0);
}

#[test]
fn undelivered_families_use_one_source_fallback() {
    let mut app = daemon();
    for command in [
        json!({"type": "server.reload_config"}),
        json!({"type": "server.agent_manifests"}),
        json!({"type": "server.live_handoff"}),
        json!({"type": "notification.show", "title": "done"}),
        json!({"type": "client.window_title.clear"}),
        json!({"type": "session.snapshot"}),
        json!({"type": "workspace.list"}),
        json!({"type": "worktree.list"}),
        json!({"type": "tab.list"}),
        json!({"type": "agent.list"}),
        json!({"type": "pane.list"}),
        json!({"type": "layout.export"}),
        json!({"type": "events.wait", "match_event": {"event": "workspace_created"}}),
        json!({"type": "integration.install", "target": "pi"}),
        json!({"type": "action.list"}),
    ] {
        let command: RpcCommand = serde_json::from_value(command.clone())
            .unwrap_or_else(|error| panic!("{command}: {error}"));
        assert_eq!(
            block_on(app.handle_api_request(command.clone())),
            Err(ErrorBody {
                code: "not_implemented".into(),
                message: "method not implemented yet".into(),
            }),
            "{command:?}"
        );
    }
    assert!(!app.state.should_quit.0);
}
