//! Awaited compiled-guest edits reach a real protocol request only after each gate opens.
#![cfg(test)]
mod http;

use maestro_extensions::{
    CommandContextOperations, ExtensionCommandContextActions, ExtensionContextActions,
    ExtensionExecution, Local, types::ProviderActions,
};
use maestro_models::{ApiProvider, Cancellation, register_api_provider, unregister_api_providers};
use serde_json::{Value, json};
use std::{rc::Rc, sync::Arc};
use tokio::sync::{mpsc, oneshot};

#[test]
fn payload_edits_are_read_after_the_awaited_callback() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for simple in [false, true] {
            for action in ["keep", "edit", "replace", "add", "delete", "abort"] {
                exercise(simple, action).await;
            }
        }
    });
}

/// The guest waits inside both original hook exports while native observations stay pending.
async fn exercise(simple: bool, action: &str) {
    let probe = Arc::new(http::Probe::default());
    let server = (action != "abort").then(http::Server::new);
    let base_url = server
        .as_ref()
        .map_or("http://127.0.0.1:1", |s| s.url.as_str());
    let provider = Arc::new(http::Protocol(probe.clone()));
    register_api_provider(
        ApiProvider {
            api: "component-gated-http".into(),
            stream: provider.clone(),
            stream_simple: provider,
        },
        Some("component-gated-http".into()),
    );
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
    let signal = Cancellation::new();
    let (entered, incoming) = mpsc::unbounded_channel();
    bind(&runner, signal.clone(), entered);
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let request = format!(
        "model-hook-gates {}",
        json!({"simple":simple,"action":action,"baseUrl":base_url})
    );
    let ((), ()) = futures_util::future::join(
        async {
            let output = command(request, runner.create_command_context())
                .await
                .unwrap_err()
                .to_string();
            let output: Value =
                serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"));
            check_output(&output, action);
        },
        drive(incoming, &probe, action),
    )
    .await;
    assert_eq!(signal.is_aborted(), action == "abort");
    if let Some(server) = server {
        let request = server.finish();
        check_request(&request, action);
        assert!(probe.body_polls() > 0);
    } else {
        assert_eq!(probe.dispatches(), 0);
        assert_eq!(probe.body_polls(), 0);
    }
    bus.clear();
    unregister_api_providers("component-gated-http");
}

/// Native command imports provide explicit admission and acknowledgment, not elapsed-time checks.
fn bind(
    runner: &maestro_extensions::ExtensionRunner<Local>,
    signal: Cancellation,
    entered: mpsc::UnboundedSender<oneshot::Sender<()>>,
) {
    let original = signal.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_signal: Rc::new(move || Ok(Some(original.clone()))),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                let (resume, done) = oneshot::channel();
                entered.send(resume).unwrap();
                Box::pin(async move {
                    done.await?;
                    Ok(())
                })
            }),
            reload: Rc::new(move || {
                signal.abort();
                Box::pin(async { Ok(()) })
            }),
            ..super::context_actions::silent_command_operations()
        },
    )));
}

/// Neither dispatch nor body polling may happen while its corresponding hook is parked.
async fn drive(
    mut incoming: mpsc::UnboundedReceiver<oneshot::Sender<()>>,
    probe: &http::Probe,
    action: &str,
) {
    let payload = incoming.recv().await.unwrap();
    assert_eq!(probe.dispatches(), 0);
    assert_eq!(probe.body_polls(), 0);
    probe.assert_pending();
    probe.allow_payload();
    payload.send(()).unwrap();
    if action != "abort" {
        let response = incoming.recv().await.unwrap();
        assert_eq!(probe.dispatches(), 1);
        assert_eq!(probe.body_polls(), 0);
        probe.assert_pending();
        probe.allow_response();
        response.send(()).unwrap();
    }
}

/// Compare the selected JSON at the real local HTTP peer, including an unchanged neighbor.
fn check_request(request: &Value, action: &str) {
    assert_eq!(request["stream"], true);
    if action == "replace" {
        assert_eq!(
            request,
            &json!({"model":"replacement","stream":true,"marker":"whole payload"})
        );
        return;
    }
    assert_eq!(request["model"], "gated-model");
    assert_eq!(
        request["temperature"],
        if action == "edit" { 0.75 } else { 0.25 }
    );
    assert_eq!(
        request["messages"],
        json!([{"role":"user","content":"unchanged neighbor"}])
    );
    assert_eq!(
        request.get("marker"),
        (action == "add").then_some(&json!({"nested":[false,0,"雪"]}))
    );
    assert_eq!(
        request.get("max_tokens"),
        (action != "delete").then_some(&json!(17))
    );
}

/// A parked hook still belongs to the same invocation and yields its normal terminal protocol.
fn check_output(output: &Value, action: &str) {
    if action == "abort" {
        assert_eq!(output["calls"], json!(["payload"]));
        assert_eq!(output["events"].as_array().unwrap().len(), 1);
        assert_eq!(output["events"][0]["type"], "error");
        assert_eq!(output["events"][0]["reason"], "aborted");
        assert_eq!(output["message"]["stopReason"], "aborted");
        assert_eq!(output["message"]["errorMessage"], "Request was aborted.");
    } else {
        assert_eq!(output["calls"], json!(["payload", "response"]));
        assert_eq!(output["events"][0]["type"], "start");
        assert_eq!(
            output["events"].as_array().unwrap().last().unwrap()["type"],
            "done"
        );
        assert_eq!(output["message"]["content"][0]["text"], "native body 雪");
        assert_eq!(output["message"]["stopReason"], "stop");
    }
    check_terminal(output);
}

/// The terminal event and independently observed result select the same message.
fn check_terminal(output: &Value) {
    let terminal = output["events"].as_array().unwrap().last().unwrap();
    assert_eq!(
        terminal
            .get("message")
            .or_else(|| terminal.get("error"))
            .unwrap(),
        &output["message"]
    );
    assert_eq!(output["sameOwner"], true);
    assert_eq!(output["eof"], true);
}
