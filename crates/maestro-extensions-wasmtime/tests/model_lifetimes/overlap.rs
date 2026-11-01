//! Overlapping returned model resources release only their own operation scopes.
use super::{Trace, bind, observe, producer};
use maestro_extensions::{ExtensionRunner, Local};
use maestro_models::{ApiProvider, Cancellation, register_api_provider, unregister_api_providers};
use serde_json::json;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};

#[test]
fn stream_aliases_release_only_their_own_scopes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for simple in [false, true] {
            overlap(simple).await;
        }
    });
}

/// The final containing-owner release occurs while two actual native producers are still live.
async fn overlap(simple: bool) {
    let (entered, mut incoming) = mpsc::unbounded_channel();
    let protocol = Arc::new(producer::Protocol(Mutex::new(entered)));
    register_api_provider(
        ApiProvider {
            api: "component-retained-model".into(),
            stream: protocol.clone(),
            stream_simple: protocol,
        },
        Some("component-retained-model".into()),
    );
    let errors = Rc::new(RefCell::new(Vec::new()));
    let (reporter, mut teardown) = crate::lifetimes::observed_reporter(&errors);
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        reporter,
    );
    let (owner, runner, events) = crate::retained_components::loaded_on(bus.for_extension()).await;
    let trace = Rc::new(Trace::default());
    let signals = [Cancellation::new(), Cancellation::new()];
    let [first, second] = prepare(&runner, &trace, &signals, simple, &mut incoming).await;
    trace.expect(&[]).await;
    bus.clear();
    drop((owner, runner, events, bus));
    pending(&mut teardown);
    release_scopes([first, second], &signals, &trace, &mut teardown).await;
    teardown.await.unwrap();
    assert!(errors.borrow().is_empty());
    unregister_api_providers("component-retained-model");
}

/// Exercise early observer drops and a surviving alias before releasing both returned resources.
async fn prepare(
    runner: &ExtensionRunner<Local>,
    trace: &Rc<Trace>,
    signals: &[Cancellation; 2],
    simple: bool,
    incoming: &mut mpsc::UnboundedReceiver<producer::Invocation>,
) -> [producer::Invocation; 2] {
    let first = begin(runner, trace, &signals[0], (simple, "first")).await;
    let second = begin(runner, trace, &signals[1], (!simple, "second")).await;
    assert_eq!(first, "created");
    assert_eq!(second, "created");
    let first = incoming.recv().await.unwrap();
    let second = incoming.recv().await.unwrap();
    observe(runner, "drop-observers first").await;
    observe(runner, "drop-observers second").await;
    observe(runner, "drop-alias second").await;
    observe(runner, "finish second").await;
    check_explicit_result(&second.stream).await;
    observe(runner, "release first").await;
    observe(runner, "release second").await;
    [first, second]
}

/// One producer completing cannot release the other one's still-uncalled hooks or signal.
async fn release_scopes(
    invocations: [producer::Invocation; 2],
    signals: &[Cancellation; 2],
    trace: &Rc<Trace>,
    teardown: &mut oneshot::Receiver<()>,
) {
    let [first, second] = invocations;
    let stream = second.stream.clone();
    let result = stream.result().await;
    settle(first, &signals[0]).await;
    trace
        .expect(&[
            "first:payload",
            "first:payload-drop",
            "first:response",
            "first:response-drop",
            "first:signal-drop",
        ])
        .await;
    pending(teardown);
    settle(second, &signals[1]).await;
    trace
        .expect(&[
            "first:payload",
            "first:payload-drop",
            "first:response",
            "first:response-drop",
            "first:signal-drop",
            "second:payload",
            "second:payload-drop",
            "second:response",
            "second:response-drop",
            "second:signal-drop",
        ])
        .await;
    assert!(Arc::ptr_eq(&result, &stream.result().await));
    assert_eq!(
        result.read().unwrap().stop_reason,
        maestro_models::StopReason::Stop
    );
    assert!(stream.next().await.is_none());
}

/// The same initiating handler starts distinct calls with their original arbitrary signals.
async fn begin(
    runner: &ExtensionRunner<Local>,
    trace: &Rc<Trace>,
    signal: &Cancellation,
    case: (bool, &str),
) -> String {
    bind(runner, trace, signal.clone());
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    command(
        format!("model-retained {}", json!({"simple":case.0,"id":case.1})),
        runner.create_command_context(),
    )
    .await
    .unwrap_err()
    .to_string()
}

/// Original callback identities remain callable even with no handler, option or stream alias left.
async fn settle(invocation: producer::Invocation, signal: &Cancellation) {
    invocation.start.send(()).unwrap();
    invocation.ready.await.unwrap();
    signal.abort();
    invocation.complete.await.unwrap();
}

/// Returned model resources admit explicit pushes and results through their native buffer.
async fn check_explicit_result(stream: &maestro_models::AssistantMessageEventStream) {
    let result = stream.result().await;
    let output = serde_json::to_value(&*result.read().unwrap()).unwrap();
    assert_eq!(output["content"][0]["text"], "explicit guest result");
    assert_eq!(output["stopReason"], "stop");
    assert!(Arc::ptr_eq(&result, &stream.result().await));
    let event = stream.next().await.unwrap();
    assert!(matches!(
        event,
        maestro_models::AssistantMessageEvent::Start { .. }
    ));
    assert_eq!(
        serde_json::to_value(&*event.message().read().unwrap()).unwrap(),
        output
    );
    assert!(stream.next().await.is_none());
}

/// Producer admissions, rather than guest registration cycles, keep the Store alive.
fn pending(teardown: &mut oneshot::Receiver<()>) {
    assert!(matches!(
        teardown.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
}
