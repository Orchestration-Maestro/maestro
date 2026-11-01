//! Original native hooks cross a registered guest provider before the real HTTP consumer.
use super::{check_consumption, check_output, drive, http, selected_payload};
use maestro_models::*;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use tokio::sync::{mpsc, oneshot};

/// Run the same selections through one real registered guest in the reverse direction.
pub(super) fn run(actions: &[&str]) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.for_extension()).await;
        super::super::model_registered::register(&runner, "http").await;

        for simple in [false, true] {
            for action in actions {
                exercise(simple, action).await;
            }
        }
        unregister_api_providers("provider:compiled-stream");
        bus.clear();
    });
}

/// Register the compiled forwarding provider and inspect the actual selected HTTP request.
async fn exercise(simple: bool, action: &str) {
    let probe = Arc::new(http::Probe::default());
    let server = (!matches!(action, "abort" | "payload-failure")).then(http::Server::new);
    let provider = Arc::new(http::Protocol(probe.clone()));
    register_api_provider(
        ApiProvider {
            api: "component-gated-http".into(),
            stream: provider.clone(),
            stream_simple: provider,
        },
        Some("component-gated-http".into()),
    );
    let signal = Cancellation::new();
    let (entered, incoming) = mpsc::unbounded_channel();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let common = options(action, &signal, entered, calls.clone());
    let stream = start(simple, server.as_ref(), common);
    drive(incoming, &probe, action, Some(&stream)).await;
    check_stream(&stream, action, &calls).await;
    assert_eq!(signal.is_aborted(), action == "abort");
    check_consumption(server, &probe, action);
    unregister_api_providers("component-gated-http");
}

/// Call the real registered guest with the original raw or simple option variant.
fn start(
    simple: bool,
    server: Option<&http::Server>,
    common: StreamOptions<Local>,
) -> AssistantMessageEventStream {
    let model = serde_json::from_value(json!({"id":"gated-model","name":"gated","api":"guest-owned-stream","provider":"compiled-stream", "baseUrl": server.map_or("http://127.0.0.1:1", |s| s.url.as_str()),"reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":10000,"maxTokens":100,"compat":{"maxTokensField":"max_tokens"}})).unwrap();
    let context = maestro_models::Context {
        system_prompt: None,
        messages: vec![
            serde_json::from_value(
                json!({"role":"user","content":"unchanged neighbor","timestamp":1}),
            )
            .unwrap(),
        ],
        tools: None,
    };
    let submit = Rc::new(|future| {
        tokio::task::spawn_local(future);
        true
    });
    let provider = get_api_provider("guest-owned-stream").unwrap();
    if simple {
        provider
            .stream_simple
            .call(
                model,
                context,
                ProviderInvocation::Local(
                    Some(SimpleStreamOptions {
                        common,
                        ..Default::default()
                    }),
                    submit,
                ),
            )
            .unwrap()
    } else {
        provider
            .stream
            .call(
                model,
                context,
                ProviderInvocation::Local(
                    Some(ProviderStreamOptions {
                        common,
                        ..Default::default()
                    }),
                    submit,
                ),
            )
            .unwrap()
    }
}

/// Native callbacks select literal data only after their independently observed readiness.
fn options(
    action: &str,
    signal: &Cancellation,
    entered: mpsc::UnboundedSender<oneshot::Sender<()>>,
    calls: Rc<RefCell<Vec<&'static str>>>,
) -> StreamOptions<Local> {
    let action = action.to_owned();
    let payload_action = action.clone();
    let payload_entered = entered.clone();
    let abort = signal.clone();
    let payload_calls = calls.clone();
    StreamOptions {
        api_key: Some("controlled".into()),
        max_retries: Some(0.0),
        temperature: Some(0.25),
        max_tokens: Some(17.0),
        signal: Some(signal.clone()),
        on_payload: Some(Rc::new(move |mut input, model| {
            payload_calls.borrow_mut().push("payload");
            assert_eq!(model.id, "gated-model");
            assert_eq!(input.payload["temperature"], 0.25);
            assert_eq!(
                input.payload["messages"],
                json!([{"role":"user","content":"unchanged neighbor"}])
            );
            let (action, entered, abort) = (
                payload_action.clone(),
                payload_entered.clone(),
                abort.clone(),
            );
            Box::pin(async move {
                gate(&entered).await;
                if action == "payload-failure" {
                    return Err(DiagnosticErrorInfo::error("payload gate failed"));
                }
                if action == "abort" {
                    abort.abort();
                }
                edit(&mut input.payload, &action);
                Ok(input)
            })
        })),
        on_response: Some(Rc::new(move |response, model| {
            calls.borrow_mut().push("response");
            assert_eq!(model.id, "gated-model");
            assert_eq!(response.status.to_bits(), 200.0_f64.to_bits());
            assert_eq!(response.headers["x-controlled"], "literal");
            let (action, entered) = (action.clone(), entered.clone());
            Box::pin(async move {
                gate(&entered).await;
                if action == "response-failure" {
                    Err(DiagnosticErrorInfo::error("response gate failed"))
                } else {
                    Ok(())
                }
            })
        })),
        ..Default::default()
    }
}

/// Return the selected JSON kind without truthiness fallback or a second payload writer.
fn edit(payload: &mut Value, action: &str) {
    if let Some(selected) = selected_payload(action) {
        *payload = selected;
        return;
    }
    match action {
        "edit" => payload["temperature"] = json!(0.75),
        "replace" => {
            *payload = json!({"model":"replacement","stream":true,"marker":"whole payload"});
        }
        "add" => payload["marker"] = json!({"nested":[false,0,"雪"]}),
        "delete" => {
            payload.as_object_mut().unwrap().shift_remove("max_tokens");
        }
        _ => (),
    }
}

/// The controller resumes this same native hook only after checking real dispatch/body state.
async fn gate(entered: &mpsc::UnboundedSender<oneshot::Sender<()>>) {
    let (resume, done) = oneshot::channel();
    entered.send(resume).unwrap();
    done.await.unwrap();
}

/// Observe final ownership before draining all queued events, preserving supplied hook errors.
async fn check_stream(
    stream: &AssistantMessageEventStream,
    action: &str,
    calls: &RefCell<Vec<&str>>,
) {
    let result = stream.result().await;
    assert!(Arc::ptr_eq(&result, &stream.result().await));
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    assert!(Arc::ptr_eq(events.last().unwrap().message(), &result));
    let eof = stream.next().await.is_none();
    let output = json!({"calls":*calls.borrow(),"events":events,"message":*result.read().unwrap(),"sameOwner":true,"eof":eof});
    check_output(&output, action);
}
