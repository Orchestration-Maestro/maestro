//! Compiled guest hooks preserve the existing native protocol exceptions and retry route.
#![cfg(test)]
mod transport;

use maestro_models::{ApiProvider, register_api_provider, unregister_api_providers};
use serde_json::Value;
use std::{rc::Rc, sync::Arc};

#[test]
fn hook_routes_keep_protocol_specific_exceptions() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let probes = register();
        let bus = maestro_extensions::create_event_bus_for::<maestro_extensions::Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _) = crate::retained_components::loaded_on(bus.for_extension()).await;
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let output = command(
            "model-protocol-routes".into(),
            runner.create_command_context(),
        )
        .await
        .unwrap_err()
        .to_string();
        let output: Value = serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
        check_output(&output);
        check_bodies(&probes);
        bus.clear();
        unregister_api_providers("component-protocol-routes");
    });
}

/// The original bundled adapters, not a second provider implementation, consume guest hooks.
fn register() -> Vec<Arc<transport::Probe>> {
    [
        "openai-completions",
        "google-generative-ai",
        "google-vertex",
        "openai-codex-responses",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, api)| {
        let probe = Arc::new(transport::Probe::default());
        let provider = Arc::new(transport::Protocol {
            api,
            probe: probe.clone(),
        });
        register_api_provider(
            ApiProvider {
                api: format!("component-protocol-{index}"),
                stream: provider.clone(),
                stream_simple: provider,
            },
            Some("component-protocol-routes".into()),
        );
        probe
    })
    .collect()
}

/// Each raw/simple invocation retains its protocol's callback sequence and native terminal result.
fn check_output(output: &Value) {
    let rows = output.as_array().unwrap();
    assert_eq!(rows.len(), 8);
    for (index, row) in rows.iter().enumerate() {
        let expected = match index % 4 {
            0 => serde_json::json!(["payload", "response"]),
            1 | 2 => serde_json::json!(["payload"]),
            3 => serde_json::json!(["payload", "response", "response"]),
            _ => unreachable!(),
        };
        assert_eq!(row["simple"], index >= 4);
        assert_eq!(row["calls"], expected);
        assert_eq!(row["message"]["content"][0]["text"], "routed");
        assert_eq!(row["message"]["stopReason"], "stop");
        assert_eq!(row["sameOwner"], true);
        assert_eq!(row["terminal"], "done");
        assert_eq!(row["eof"], true);
    }
}

/// Rejected response attempts release their real native bodies without a first poll.
fn check_bodies(probes: &[Arc<transport::Probe>]) {
    for (index, probe) in probes.iter().enumerate() {
        let bodies = probe.bodies.lock().unwrap();
        assert_eq!(bodies.len(), if index == 3 { 4 } else { 2 });
        for (attempt, body) in bodies.iter().enumerate() {
            assert_eq!(body.drops(), 1);
            if index == 3 && attempt % 2 == 0 {
                assert_eq!(body.polls(), 0);
            } else {
                assert!(body.polls() > 0);
            }
        }
    }
}
