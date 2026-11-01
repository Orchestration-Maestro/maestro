//! Selected guest cancellation controls the native pending content-generation transport.
#![cfg(test)]
mod transport;

use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, Local, types::ProviderActions,
};
use maestro_models::{ApiProvider, Cancellation, register_api_provider, unregister_api_providers};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc, sync::Arc};

#[test]
fn payload_signal_selection_reaches_its_actual_consumer() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for simple in [false, true] {
            for action in ["keep", "replace", "remove"] {
                exercise(simple, action).await;
            }
        }
    });
}

/// Keep each invocation and its independently chosen signals isolated.
async fn exercise(simple: bool, action: &str) {
    let (probe, incoming) = transport::Probe::new();
    let server = transport::Server::new();
    let provider = Arc::new(transport::Protocol(probe.clone()));
    register_api_provider(
        ApiProvider {
            api: "component-selected-signal".into(),
            stream: provider.clone(),
            stream_simple: provider,
        },
        Some("component-selected-signal".into()),
    );
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
    let signals = [
        Cancellation::new(),
        Cancellation::new(),
        Cancellation::new(),
    ];
    bind(&runner, &signals);
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let request = format!(
        "model-selected-signal {}",
        json!({"simple":simple,"action":action,"baseUrl":server.url})
    );
    futures_util::future::join(
        async {
            let output = command(request, runner.create_command_context())
                .await
                .unwrap_err()
                .to_string();
            let output: Value =
                serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
            check_output(&output);
        },
        transport::drive(&probe, incoming, server, &signals, action),
    )
    .await;
    bus.clear();
    unregister_api_providers("component-selected-signal");
}

/// The second context lookup supplies a different, still-live caller signal to the hook.
fn bind(runner: &maestro_extensions::ExtensionRunner<Local>, signals: &[Cancellation; 3]) {
    let pair = [signals[0].clone(), signals[1].clone()];
    let read = Cell::new(0);
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_signal: Rc::new(move || {
                    let index = read.replace(read.get() + 1);
                    Ok(Some(pair[index].clone()))
                }),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Native terminal policy remains separate from cancellation of the selected transport.
fn check_output(output: &Value) {
    assert_eq!(output["sameOwner"], true);
    assert_eq!(output["eof"], true);
    assert_eq!(output["message"]["stopReason"], "aborted");
    assert_eq!(output["message"]["errorMessage"], "Request was aborted");
    let events = output["events"].as_array().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "error")
            .count(),
        1
    );
    let terminal = events.last().unwrap();
    assert_eq!(terminal["type"], "error");
    assert_eq!(terminal["reason"], "aborted");
    assert_eq!(terminal["error"], output["message"]);
}
