//! Registered guest stream functions consume original native hooks and option variants.
#![cfg(test)]
mod affinity;
mod oauth;
mod retention;
use maestro_models::*;
use serde_json::{Value, json};
use std::{rc::Rc, sync::Arc};
use tokio::sync::{mpsc, oneshot};

#[test]
fn registered_guest_streams_invoke_native_hook_handles() {
    native_pending_read_oracle();
    cases(&[
        "keep",
        "null",
        "false",
        "zero",
        "empty-string",
        "number",
        "string",
        "object",
        "array",
        "edit",
        "replace",
        "add",
        "delete",
        "abort",
        "payload-failure",
        "response-failure",
    ]);
    fault_orders();
    affinity::run();
    retention::run();
}

/// Each case reaches the actual provider installed through the public guest registration facade.
pub(super) fn cases(actions: &[&str]) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = bus();
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        for route in ["create", "catalog"] {
            route_cases(&runner, route, actions).await;
        }
        unregister_api_providers("provider:compiled-stream");
        bus.clear();
    });
}
/// Reuse each real registration route across its original raw and simple option cases.
async fn route_cases(
    runner: &maestro_extensions::ExtensionRunner<Local>,
    route: &str,
    actions: &[&str],
) {
    register(runner, route).await;
    for simple in [false, true] {
        for action in actions {
            exercise(runner, simple, action).await;
        }
        native_admission(runner, simple).await;
    }
}
/// Register through the compiled guest's public facade, never a test-only callback route.
pub(super) async fn register(runner: &maestro_extensions::ExtensionRunner<Local>, route: &str) {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    assert_eq!(
        command(
            format!("model-registered {route}"),
            runner.create_command_context()
        )
        .await
        .unwrap_err()
        .to_string(),
        "registered"
    );
}
/// Native callbacks own both gates; the returned authored stream stays genuinely pending at each.
async fn exercise(runner: &maestro_extensions::ExtensionRunner<Local>, simple: bool, action: &str) {
    let stream = produced(runner, simple, action).await;
    check(&stream, action).await;
}
/// Drive the real guest producer without yet observing its queued terminal or final result.
async fn produced(
    runner: &maestro_extensions::ExtensionRunner<Local>,
    simple: bool,
    action: &str,
) -> AssistantMessageEventStream {
    let (entered, mut incoming) = mpsc::unbounded_channel();
    let signal = Cancellation::new();
    let common = options(action, entered, signal.clone());
    let stream = start(simple, common);
    pending(&stream);
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let producer = command(
        "model-registered produce".into(),
        runner.create_command_context(),
    );
    let control = async {
        let (phase, resume) = incoming.recv().await.unwrap();
        assert_eq!(phase, "payload");
        pending(&stream);
        resume.send(()).unwrap();
        if !matches!(action, "abort" | "payload-failure") {
            let (phase, resume) = incoming.recv().await.unwrap();
            assert_eq!(phase, "response");
            pending(&stream);
            resume.send(()).unwrap();
        }
    };
    let (outcome, ()) = futures_util::future::join(producer, control).await;
    assert_eq!(outcome.unwrap_err().to_string(), "produced");
    stream
}
/// Exact selected values are independent native operands, not recomputed from guest output.
fn selected(action: &str, mut original: Value) -> Value {
    match action {
        "null" => Value::Null,
        "false" => json!(false),
        "zero" => json!(0),
        "empty-string" => json!(""),
        "number" => json!(7.5),
        "string" => json!("replacement 雪"),
        "object" => json!({"selected":true}),
        "array" => json!([null, false, 0]),
        "replace" => json!({"whole":"replacement"}),
        "edit" => {
            original["original"] = json!(false);
            original
        }
        "add" => {
            original["added"] = json!([false, 0]);
            original
        }
        "delete" => {
            original.as_object_mut().unwrap().shift_remove("original");
            original
        }
        _ => original,
    }
}
/// Await native readiness before returning the selected payload or response failure.
fn options(
    action: &str,
    entered: mpsc::UnboundedSender<(&'static str, oneshot::Sender<()>)>,
    signal: Cancellation,
) -> StreamOptions<Local> {
    let action = action.to_owned();
    let payload_action = action.clone();
    let payload_entered = entered.clone();
    let abort = signal.clone();
    StreamOptions {
        temperature: Some(-0.0),
        timeout_ms: Some(17.5),
        max_retries: Some(2.5),
        signal: Some(signal),
        on_payload: Some(Rc::new(move |mut input, model| {
            assert_eq!(model.id, "registered-case");
            assert_eq!(
                input.payload,
                json!({"original":true,"unchanged":"neighbor"})
            );
            assert!(!input.signal.as_ref().unwrap().is_aborted());
            let (action, entered, abort) = (
                payload_action.clone(),
                payload_entered.clone(),
                abort.clone(),
            );
            Box::pin(async move {
                gate(&entered, "payload").await;
                if action == "payload-failure" {
                    return Err(DiagnosticErrorInfo::error("payload gate failed"));
                }
                if action == "abort" {
                    abort.abort();
                }
                input.payload = selected(&action, input.payload);
                Ok(input)
            })
        })),
        on_response: Some(Rc::new(move |response, model| {
            assert_eq!(model.id, "registered-case");
            assert_eq!(response.status.to_bits(), 207.5_f64.to_bits());
            assert_eq!(
                response.headers.get("x-native").map(String::as_str),
                Some("literal")
            );
            let (action, entered) = (action.clone(), entered.clone());
            Box::pin(async move {
                gate(&entered, "response").await;
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
/// One native completion channel proves the callback has actually been entered and awaited.
async fn gate(
    entered: &mpsc::UnboundedSender<(&'static str, oneshot::Sender<()>)>,
    phase: &'static str,
) {
    let (resume, done) = oneshot::channel();
    entered.send((phase, resume)).unwrap();
    done.await.unwrap();
}
/// Native event and result observations remain independent and pending at either original hook.
pub(super) fn pending(stream: &AssistantMessageEventStream) {
    use std::{
        future::Future,
        task::{Context, Waker},
    };
    let mut context = Context::from_waker(Waker::noop());
    assert!(
        std::pin::pin!(stream.result())
            .as_mut()
            .poll(&mut context)
            .is_pending()
    );
    assert!(
        std::pin::pin!(stream.next())
            .as_mut()
            .poll(&mut context)
            .is_pending()
    );
}
/// Native final ownership is shared by repeated result observations and its terminal event.
async fn check(stream: &AssistantMessageEventStream, action: &str) {
    let message = stream.result().await;
    assert!(Arc::ptr_eq(&message, &stream.result().await));
    let value = serde_json::to_value(&*message.read().unwrap()).unwrap();
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    let terminal = events.last().expect("imported stream must retain events after the same dropped-pending-read pattern as the native oracle");
    assert!(Arc::ptr_eq(terminal.message(), &message));
    check_outcome(&events, action, &value);
    assert!(stream.next().await.is_none());
}

/// The native owner removes unassigned dropped reads; assigned abandoned reads still advance.
fn native_pending_read_oracle() {
    use std::{
        future::Future,
        task::{Context, Waker},
    };
    let stream = AssistantMessageEventStream::new();
    for _ in 0..3 {
        pending(&stream);
    }
    let message = Arc::new(std::sync::RwLock::new(faux_assistant_message(
        "native oracle",
        FauxAssistantMessageOptions::default(),
    )));
    stream.push(AssistantMessageEvent::Done {
        reason: DoneReason::Stop,
        message,
    });
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        std::pin::pin!(stream.next()).as_mut().poll(&mut context),
        std::task::Poll::Ready(Some(AssistantMessageEvent::Done { .. }))
    ));
    assert!(matches!(
        std::pin::pin!(stream.next()).as_mut().poll(&mut context),
        std::task::Poll::Ready(None)
    ));
}

/// A decoded final wins over a later Store fault; an unread guest terminal does not.
fn fault_orders() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for cached in [false, true] {
            fault_order(cached).await;
        }
    });
}
/// Compare both final-cache orders through an actual Store trap and native result consumer.
async fn fault_order(cached: bool) {
    let bus = bus();
    let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    register(&runner, "create").await;
    let stream = produced(&runner, false, "keep").await;
    assert!(matches!(
        stream.next().await,
        Some(AssistantMessageEvent::Start { .. })
    ));
    let before = if cached {
        Some(stream.result().await)
    } else {
        None
    };
    let fault = command("trap".into(), runner.create_command_context())
        .await
        .unwrap_err()
        .to_string();
    let (result, terminal) = fault_observations(&stream).await;
    check_fault(before, &result, &terminal, &fault);
    assert!(stream.next().await.is_none());
    unregister_api_providers("provider:compiled-stream");
    bus.clear();
}

/// Dispatch on the actual provider registry while retaining the supplied option variant.
fn start(simple: bool, common: StreamOptions<Local>) -> AssistantMessageEventStream {
    let (model, context) = operands(simple);
    let provider = get_api_provider("guest-owned-stream").unwrap();
    let submit = Rc::new(|future| {
        tokio::task::spawn_local(future);
        true
    });
    if simple {
        provider
            .stream_simple
            .call(
                model,
                context,
                ProviderInvocation::Local(Some(simple_options(common)), submit),
            )
            .unwrap()
    } else {
        provider
            .stream
            .call(
                model,
                context,
                ProviderInvocation::Local(Some(raw_options(common)), submit),
            )
            .unwrap()
    }
}
/// Deliberately unequal raw policies must not become simple reasoning or budgets.
fn raw_options<E: ProviderExecution>(common: StreamOptions<E>) -> ProviderStreamOptions<E> {
    ProviderStreamOptions {
        common,
        extra: serde_json::from_value(json!({"reasoning":"raw-low","thinkingBudgets":{"high":19}}))
            .unwrap(),
        ..Default::default()
    }
}
/// Requested simple policies are independent of similarly named raw extras.
fn simple_options<E: ProviderExecution>(common: StreamOptions<E>) -> SimpleStreamOptions<E> {
    SimpleStreamOptions {
        common,
        reasoning: Some(ThinkingLevel::High),
        thinking_budgets: Some(ThinkingBudgets {
            high: Some(31.5),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Compare the actual terminal protocol and ordered events with literal expectations.
fn check_outcome(events: &[AssistantMessageEvent], action: &str, value: &Value) {
    if action.ends_with("-failure") || action == "abort" {
        check_failure(events, action, value);
    } else {
        check_success(events, action, value);
    }
}
/// Hook failures and selected cancellation retain their distinct terminal reasons and text.
fn check_failure(events: &[AssistantMessageEvent], action: &str, value: &Value) {
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], AssistantMessageEvent::Error { .. }));
    assert_eq!(
        value["stopReason"],
        if action == "abort" {
            "aborted"
        } else {
            "error"
        }
    );
    assert_eq!(
        value["errorMessage"],
        match action {
            "payload-failure" => "payload gate failed",
            "response-failure" => "response gate failed",
            _ => "Request was aborted",
        }
    );
}
/// Selected payloads reach the final native consumer after the ordered guest updates.
fn check_success(events: &[AssistantMessageEvent], action: &str, value: &Value) {
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], AssistantMessageEvent::Start { .. }));
    assert!(matches!(events[1], AssistantMessageEvent::TextDelta { .. }));
    assert!(matches!(events[2], AssistantMessageEvent::Done { .. }));
    assert_eq!(
        value["content"][0]["text"],
        selected(action, json!({"original":true,"unchanged":"neighbor"})).to_string()
    );
}

/// A cached guest result remains canonical after a trap, including its terminal event.
fn check_fault(
    before: Option<SharedAssistantMessage>,
    result: &SharedAssistantMessage,
    terminal: &AssistantMessageEvent,
    fault: &str,
) {
    if let Some(before) = before {
        assert!(Arc::ptr_eq(&before, result));
        assert!(matches!(terminal, AssistantMessageEvent::Done { .. }));
        assert_eq!(result.read().unwrap().stop_reason, StopReason::Stop);
    } else {
        check_uncached_fault(result, terminal, fault);
    }
}
/// An unread guest terminal cannot replace the native fault projection.
fn check_uncached_fault(
    result: &SharedAssistantMessage,
    terminal: &AssistantMessageEvent,
    fault: &str,
) {
    assert!(matches!(
        terminal,
        AssistantMessageEvent::Error {
            reason: ErrorReason::Error,
            ..
        }
    ));
    assert_eq!(result.read().unwrap().stop_reason, StopReason::Error);
    assert_eq!(result.read().unwrap().error_message.as_deref(), Some(fault));
}

/// Shared callback diagnostics leave native test behavior unchanged across fresh Stores.
fn bus() -> maestro_extensions::EventBusController<Local> {
    maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    )
}
/// Native admission reaches the returned guest's original FIFO and independent final result.
async fn native_admission(runner: &maestro_extensions::ExtensionRunner<Local>, simple: bool) {
    let (entered, incoming) = mpsc::unbounded_channel();
    let stream = start(simple, options("keep", entered, Cancellation::new()));
    let message = Arc::new(std::sync::RwLock::new(faux_assistant_message(
        "native admission",
        FauxAssistantMessageOptions::default(),
    )));
    stream.push(AssistantMessageEvent::Start {
        partial: message.clone(),
    });
    stream.end(Some(message.clone()));
    let result = stream.result().await;
    assert!(Arc::ptr_eq(&result, &stream.result().await));
    assert_eq!(
        serde_json::to_value(&*result.read().unwrap()).unwrap(),
        serde_json::to_value(&*message.read().unwrap()).unwrap()
    );
    assert!(matches!(
        stream.next().await,
        Some(AssistantMessageEvent::Start { .. })
    ));
    assert!(stream.next().await.is_none());
    finish_ended_producer(runner, incoming).await;
    assert!(Arc::ptr_eq(&result, &stream.result().await));
}
/// Ending a buffer does not stop its independently admitted producer or native hook calls.
async fn finish_ended_producer(
    runner: &maestro_extensions::ExtensionRunner<Local>,
    mut incoming: mpsc::UnboundedReceiver<(&'static str, oneshot::Sender<()>)>,
) {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let control = async {
        for expected in ["payload", "response"] {
            let (phase, resume) = incoming.recv().await.unwrap();
            assert_eq!(phase, expected);
            resume.send(()).unwrap();
        }
    };
    let (outcome, ()) = futures_util::future::join(
        command(
            "model-registered produce".into(),
            runner.create_command_context(),
        ),
        control,
    )
    .await;
    assert_eq!(outcome.unwrap_err().to_string(), "produced");
}

/// Faulted observations still share one result owner with the single terminal event.
async fn fault_observations(
    stream: &AssistantMessageEventStream,
) -> (SharedAssistantMessage, AssistantMessageEvent) {
    let result = stream.result().await;
    assert!(Arc::ptr_eq(&result, &stream.result().await));
    let terminal = stream
        .next()
        .await
        .expect("one terminal after the Store fault");
    assert!(Arc::ptr_eq(terminal.message(), &result));
    (result, terminal)
}

/// Use the same literal native operands in local and off-thread invocation.
fn operands(simple: bool) -> (Model, Context) {
    let model: Model = serde_json::from_value(json!({"id":"registered-case","name":"registered","api":"guest-owned-stream","provider":"compiled-stream","baseUrl":"https://example.invalid","reasoning":false,"input":[],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":100,"maxTokens":10})).unwrap();
    let context = Context {
        system_prompt: Some(if simple { "simple" } else { "raw" }.into()),
        messages: Vec::new(),
        tools: None,
    };
    (model, context)
}
