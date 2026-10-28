//! Returned model invocations keep their own hooks and cancellation after registration release.
#![cfg(test)]
mod overlap;
mod producer;

use maestro_extensions::{
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, types::ProviderActions,
};
use maestro_models::{ApiProvider, Cancellation, register_api_provider, unregister_api_providers};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::sync::{Notify, mpsc};

use crate::model_registered::pending;

/// Effects from real guest callback and destructor calls into application actions.
#[derive(Default)]
struct Trace {
    /// Every distinct call or capture release.
    labels: RefCell<Vec<String>>,
    /// Acknowledges native effects without elapsed-time assumptions.
    changed: Notify,
}
impl Trace {
    /// Wait for actual destruction acknowledgments and reject duplicates or foreign releases.
    async fn expect(&self, expected: &[&str]) {
        while self.labels.borrow().len() < expected.len() {
            self.changed.notified().await;
        }
        let mut actual = self.labels.borrow().clone();
        actual.sort();
        assert_eq!(actual, expected);
    }
}

#[test]
fn returned_stream_retains_hooks_and_cancellation() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        for simple in [false, true] {
            retained(simple, None).await;
        }
    });
}

/// Hooks are first called only after their initiating registration and temporary options end.
pub(super) async fn retained(simple: bool, object_drop: Option<&'static str>) {
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
    let (reporter, teardown) = super::lifetimes::observed_reporter(&errors);
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        reporter,
    );
    let (owner, runner, events) = super::retained_components::loaded_on(bus.for_extension()).await;
    let trace = Rc::new(Trace::default());
    let signal = Cancellation::new();
    bind(&runner, &trace, signal.clone());
    let invocation = start(&owner, &runner, &mut incoming, (simple, object_drop)).await;
    trace.expect(&[]).await;
    invocation.start.send(()).unwrap();
    invocation.ready.await.unwrap();
    trace.expect(&["first:payload", "first:response"]).await;
    pending(&invocation.stream);
    Cancellation::new().abort();
    pending(&invocation.stream);
    signal.abort();
    invocation.complete.await.unwrap();
    let result = invocation.stream.result().await;
    assert_eq!(
        result.read().unwrap().error_message.as_deref(),
        Some("Request was aborted")
    );
    observe_releases(&runner, &trace, object_drop).await;
    bus.clear();
    drop((invocation.stream, owner, runner, events, bus));
    teardown.await.unwrap();
    assert!(errors.borrow().is_empty());
    unregister_api_providers("component-retained-model");
}

/// Repeated final owners and every capture release are observed through the retained component.
async fn observe_releases(
    runner: &ExtensionRunner<Local>,
    trace: &Trace,
    object_drop: Option<&str>,
) {
    let output = observe(runner, "observe first").await;
    check_output(&output);
    observe(runner, "release first").await;
    let mut expected = vec![
        "first:payload",
        "first:payload-drop",
        "first:response",
        "first:response-drop",
        "first:signal-drop",
    ];
    if let Some(label) = object_drop {
        expected.insert(0, label);
    }
    trace.expect(&expected).await;
}

/// Release the initiating registration before allowing any provider hook to run.
async fn start(
    owner: &maestro_extensions::types::ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    incoming: &mut mpsc::UnboundedReceiver<producer::Invocation>,
    case: (bool, Option<&str>),
) -> producer::Invocation {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    assert_eq!(
        command(
            format!(
                "model-retained {}",
                json!({"simple":case.0,"id":"first","opaque":case.1.is_some()})
            ),
            runner.create_command_context()
        )
        .await
        .unwrap_err()
        .to_string(),
        "created"
    );
    owner
        .write()
        .unwrap()
        .commands
        .shift_remove("bridge-observe")
        .unwrap();
    drop(command);
    incoming.recv().await.unwrap()
}

/// Bind one original signal and observe through independently retained native contexts.
fn bind(runner: &ExtensionRunner<Local>, trace: &Rc<Trace>, signal: Cancellation) {
    let observed = trace.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                compact: Rc::new(move |options| {
                    let label = options.unwrap().custom_instructions.unwrap();
                    observed.labels.borrow_mut().push(label);
                    observed.changed.notify_one();
                    Ok(())
                }),
                get_signal: Rc::new(move || Ok(Some(signal.clone()))),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Observe through a separately registered guest callback, not the released initiating closure.
async fn observe(runner: &ExtensionRunner<Local>, action: &str) -> Value {
    let command = runner
        .get_command("model-retained-observe")
        .unwrap()
        .command
        .handler;
    let output = command(action.into(), runner.create_command_context())
        .await
        .unwrap_err()
        .to_string();
    serde_json::from_str(&output).unwrap_or_else(|_| panic!("{output}"))
}

/// Independent result observation leaves exactly the terminal event and then EOF.
fn check_output(output: &Value) {
    assert_eq!(output["sameOwner"], true);
    assert_eq!(output["message"]["stopReason"], "aborted");
    assert_eq!(output["message"]["errorMessage"], "Request was aborted");
    assert_eq!(output["events"].as_array().unwrap().len(), 1);
    assert_eq!(output["events"][0]["type"], "error");
    assert_eq!(output["events"][0]["reason"], "aborted");
    assert_eq!(output["events"][0]["error"], output["message"]);
    assert_eq!(output["eof"], true);
}
