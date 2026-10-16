#![cfg(test)]
//! Queued factories through both real registered execution families.
use maestro_models::*;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

/// The five source-backed factory outcomes extended by the named callers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Scenario {
    /// Live model context and request count.
    Live,
    /// Caller-supplied terminal rejection.
    Failure,
    /// Original raw and simple carriers.
    Options,
    /// Suspended response before immediate factory entry.
    Order,
    /// Response observation before an exhausted queue.
    Exhaustion,
    /// A rejecting response wins over queue exhaustion.
    RejectedExhaustion,
}
/// One original identity held across the factory's suspension.
struct Factory {
    /// Selected source branch.
    scenario: Scenario,
    /// Response completion must already be visible at immediate factory entry.
    phases: Arc<Mutex<Vec<&'static str>>>,
    /// Original provider object, never reconstructed as JSON.
    object: Arc<u32>,
    /// Original hook addresses, excluding trait metadata.
    hooks: [usize; 2],
}
/// Future erasure differs; all observable factory work is shared.
macro_rules! factory {
    ($mode:ty) => {
        impl Invoke<$mode, FauxResponseInput<$mode>> for Factory {
            type Return = <$mode as Execution>::Future<
                'static,
                Result<AssistantMessage, DiagnosticErrorInfo>,
            >;
            fn invoke(&self, input: FauxResponseInput<$mode>) -> Self::Return {
                assert_eq!(
                    *self.phases.lock().unwrap(),
                    ["response start", "response end"]
                );
                self.phases.lock().unwrap().push("factory");
                Box::pin(respond(
                    input,
                    self.scenario,
                    self.object.clone(),
                    self.hooks,
                ))
            }
        }
    };
}
factory!(Local);
factory!(Threaded);
/// The real factory observes state and original carriers only after resuming.
async fn respond<E: ProviderExecution>(
    input: FauxResponseInput<E>,
    scenario: Scenario,
    object: Arc<u32>,
    hooks: [usize; 2],
) -> Result<AssistantMessage, DiagnosticErrorInfo> {
    tokio::task::yield_now().await;
    assert_eq!(input.state.call_count(), 1);
    assert_eq!(input.context.messages.len(), 1);
    if scenario == Scenario::Failure {
        return Err(DiagnosticErrorInfo::error("boom"));
    }
    if scenario == Scenario::Options {
        let common = original_common(input.options.unwrap(), &object, hooks);
        let selected = common.on_payload.unwrap()(
            PayloadHookInput {
                payload: serde_json::json!(11),
                signal: common.signal,
            },
            Arc::new(input.model),
        )
        .await?;
        assert_eq!(selected.payload, serde_json::json!(22));
    }
    Ok(faux_assistant_message(
        format!(
            "{}:{}",
            input.context.messages.len(),
            input.state.call_count()
        ),
        FauxAssistantMessageOptions {
            timestamp: Some(0.0),
            ..Default::default()
        },
    ))
}
/// Check original factory option identity after its suspension.
fn original_common<E: ProviderExecution>(
    options: StreamOptionsInput<E>,
    object: &Arc<u32>,
    hooks: [usize; 2],
) -> StreamOptions<E> {
    let common = match options {
        StreamOptionsInput::Raw(raw) => {
            assert_eq!(raw.extra["custom"], serde_json::json!(["z", "a", "z"]));
            assert!(Arc::ptr_eq(raw.objects.get::<Arc<u32>>().unwrap(), object));
            raw.common
        }
        StreamOptionsInput::Simple(simple) => {
            assert_eq!(simple.reasoning, Some(ThinkingLevel::High));
            assert_eq!(simple.thinking_budgets.unwrap().high, Some(123.0));
            assert_eq!(simple.tool_choice, Some(ToolChoice::Required));
            simple.common
        }
    };
    assert_eq!(common.session_id.as_deref(), Some("family-cache"));
    assert_eq!(
        common.metadata.as_ref().unwrap()["original"],
        serde_json::json!(true)
    );
    assert_eq!(
        std::ptr::from_ref(&**common.on_payload.as_ref().unwrap()).cast::<()>() as usize,
        hooks[0]
    );
    assert_eq!(
        std::ptr::from_ref(&**common.on_response.as_ref().unwrap()).cast::<()>() as usize,
        hooks[1]
    );
    common
}
/// Concrete original hooks retain a local or thread-safe counter over their await.
trait Mode: ProviderExecution {
    /// Caller supplies admission and joins every admitted producer.
    fn spawn(tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>) -> ProviderSpawner<Self>;
    /// Build a response observer that cannot complete until explicitly released.
    fn common(
        phases: Arc<Mutex<Vec<&'static str>>>,
        release: tokio::sync::oneshot::Receiver<()>,
        entered: tokio::sync::oneshot::Sender<()>,
        scenario: Scenario,
    ) -> StreamOptions<Self>;
}
/// Only pointer and future erasure differ between the executions.
macro_rules! mode {
    ($mode:ty, $pointer:ident, $spawn:path) => {
        impl Mode for $mode {
            fn spawn(tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>) -> ProviderSpawner<Self> {
                $pointer::new(move |task| { let task = $spawn(task); tasks.lock().unwrap().push(task); true })
            }
            fn common(phases: Arc<Mutex<Vec<&'static str>>>, release: tokio::sync::oneshot::Receiver<()>, entered: tokio::sync::oneshot::Sender<()>, scenario: Scenario) -> StreamOptions<Self> {
                let gates = Arc::new(Mutex::new(Some((release, entered))));
                let retained = $pointer::new(37);
                StreamOptions {
                    on_response: Some($pointer::new(move |_, _| {
                        let (release, entered) = gates.lock().unwrap().take().unwrap();
                        let phases = phases.clone(); let retained = retained.clone();
                        Box::pin(async move {
                            phases.lock().unwrap().push("response start");
                            entered.send(()).unwrap(); release.await.unwrap();
                            assert_eq!(*retained, 37);
                            phases.lock().unwrap().push("response end");
                            if scenario == Scenario::RejectedExhaustion { Err(DiagnosticErrorInfo::error("response stopped")) } else { Ok(()) }
                        })
                    })),
                    on_payload: Some($pointer::new(|mut input, _| Box::pin(async move {
                        tokio::task::yield_now().await; input.payload = serde_json::json!(22); Ok(input)
                    }))),
                    signal: Some(Cancellation::new()), session_id: Some("family-cache".into()),
                    metadata: Some(serde_json::from_value(serde_json::json!({"original":true})).unwrap()),
                    ..Default::default()
                }
            }
        }
    };
}
mode!(Local, Rc, tokio::task::spawn_local);
mode!(Threaded, Arc, tokio::spawn);
/// Closed public stream observation, retaining repeated events in their original order.
async fn drain(stream: &AssistantMessageEventStream) -> Vec<&'static str> {
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(match event {
            AssistantMessageEvent::Start { .. } => "start",
            AssistantMessageEvent::TextStart { .. } => "text_start",
            AssistantMessageEvent::TextDelta { .. } => "text_delta",
            AssistantMessageEvent::TextEnd { .. } => "text_end",
            AssistantMessageEvent::Done { .. } => "done",
            AssistantMessageEvent::Error { .. } => "error",
            _ => panic!("unexpected event"),
        });
    }
    events
}
/// Preserve the original raw or simple record at the public registered entry.
fn invoke<E: Mode>(
    registration: &FauxProviderRegistration,
    simple: bool,
    common: StreamOptions<E>,
    spawn: ProviderSpawner<E>,
    object: Arc<u32>,
) -> AssistantMessageEventStream {
    let model = registration.get_model(None).unwrap().clone();
    let context = super::context("hi");
    if simple {
        stream_simple_for::<E>(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                reasoning: Some(ThinkingLevel::High),
                thinking_budgets: Some(ThinkingBudgets {
                    high: Some(123.0),
                    ..Default::default()
                }),
                tool_choice: Some(ToolChoice::Required),
            }),
            spawn,
        )
    } else {
        let mut raw = ProviderStreamOptions {
            common,
            extra: serde_json::from_value(serde_json::json!({"custom":["z","a","z"]})).unwrap(),
            ..Default::default()
        };
        raw.objects.insert(object);
        stream_for::<E>(model, context, Some(raw), spawn)
    }
    .unwrap()
}
/// One admission is observed while the response hook is suspended, before any factory call.
async fn run<E: Mode>(scenario: Scenario, simple: bool) {
    let registration = super::fixed(None);
    let phases = Arc::new(Mutex::new(Vec::new()));
    let (release, released) = tokio::sync::oneshot::channel();
    let (entered, entry) = tokio::sync::oneshot::channel();
    let common = E::common(phases.clone(), released, entered, scenario);
    let object = Arc::new(37);
    let hooks = [
        std::ptr::from_ref(&**common.on_payload.as_ref().unwrap()).cast::<()>() as usize,
        std::ptr::from_ref(&**common.on_response.as_ref().unwrap()).cast::<()>() as usize,
    ];
    if !matches!(
        scenario,
        Scenario::Exhaustion | Scenario::RejectedExhaustion
    ) {
        registration.set_responses(vec![FauxResponseStep::Factory(Arc::new(Factory {
            scenario,
            phases: phases.clone(),
            object: object.clone(),
            hooks,
        }))]);
    }
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let stream = invoke::<E>(
        &registration,
        simple,
        common,
        E::spawn(tasks.clone()),
        object,
    );
    entry.await.unwrap();
    assert_eq!(*phases.lock().unwrap(), ["response start"]);
    assert_eq!(registration.state.call_count(), 1);
    assert_eq!(registration.get_pending_response_count(), 0);
    release.send(()).unwrap();
    let events = drain(&stream).await;
    let result = stream.result().await;
    let tasks = std::mem::take(&mut *tasks.lock().unwrap());
    assert_eq!(tasks.len(), 1);
    for task in tasks {
        task.await.unwrap();
    }
    assert_result(&result.read().unwrap(), &events, scenario);
    if scenario == Scenario::Options {
        check_cache::<E>(&registration, simple).await;
    }
    registration.unregister();
}
/// Exact terminal events and caller-owned diagnostics.
fn assert_result(result: &AssistantMessage, events: &[&str], scenario: Scenario) {
    if matches!(
        scenario,
        Scenario::Failure | Scenario::Exhaustion | Scenario::RejectedExhaustion
    ) {
        assert_eq!(events, ["error"]);
        assert_eq!(result.stop_reason, StopReason::Error);
        assert_eq!(
            result.error_message.as_deref(),
            Some(if scenario == Scenario::Failure {
                "boom"
            } else if scenario == Scenario::RejectedExhaustion {
                "response stopped"
            } else {
                "No more faux responses queued"
            })
        );
    } else {
        assert_eq!(
            events,
            ["start", "text_start", "text_delta", "text_end", "done"]
        );
        assert_eq!(result.content, [AssistantContent::Text(faux_text("1:1"))]);
    }
}
/// Replacing and appending responses keeps the original session cache and live count.
async fn check_cache<E: Mode>(registration: &FauxProviderRegistration, simple: bool) {
    registration.set_responses(vec![super::scripted("discard")]);
    registration.set_responses(vec![]);
    registration.append_responses(vec![super::scripted("cached")]);
    assert_eq!(registration.state.call_count(), 1);
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let stream = invoke::<E>(
        registration,
        simple,
        StreamOptions {
            session_id: Some("family-cache".into()),
            ..Default::default()
        },
        E::spawn(tasks.clone()),
        Arc::new(37),
    );
    let _ = drain(&stream).await;
    let result = stream.result().await;
    let tasks = std::mem::take(&mut *tasks.lock().unwrap());
    for task in tasks {
        task.await.unwrap();
    }
    let result = result.read().unwrap();
    assert_eq!(
        result.content,
        [AssistantContent::Text(faux_text("cached"))]
    );
    super::number(result.usage.cache_read, 2.0);
    assert_eq!(registration.state.call_count(), 2);
    assert_eq!(registration.get_pending_response_count(), 0);
}
/// Run through both public families while retaining and joining local work.
pub(super) fn exercise(scenario: Scenario) {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(tokio::task::LocalSet::new().run_until(async {
            for simple in [false, true] {
                run::<Local>(scenario, simple).await;
                run::<Threaded>(scenario, simple).await;
                if scenario == Scenario::Exhaustion {
                    run::<Local>(Scenario::RejectedExhaustion, simple).await;
                    run::<Threaded>(Scenario::RejectedExhaustion, simple).await;
                }
            }
        }));
}
