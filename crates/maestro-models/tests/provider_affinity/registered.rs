//! The same retained custom provider produces both execution families.
#![cfg(test)]
use maestro_models::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

/// Closed recording of callback phases, not presence-derived labels.
pub(super) type Trace = Arc<Transcript>;
/// Closed event log plus the response hook's completion boundary.
#[derive(Default)]
pub(super) struct Transcript {
    /// Actual ordered callback observations.
    events: Mutex<Vec<Value>>,
    /// Written only after the response hook has resumed and accepted the response.
    pub response_complete: std::sync::atomic::AtomicBool,
}
impl Transcript {
    /// Borrow only the internal log, never caller callbacks.
    pub fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, Vec<Value>>> {
        self.events.lock()
    }
}
/// Keep one phase observation after releasing any caller-owned state.
pub(super) fn record(trace: &Trace, event: Value) {
    trace.lock().unwrap().push(event);
}
/// Recorded callback input selects the original carrier and failure branch.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CallbackInput {
    /// Original options surface.
    pub form: String,
    /// Caller-supplied failure or success.
    pub outcome: String,
}
/// One registered object; only future boxing differs between families.
struct HookProvider(Trace);
/// Concrete future erasure for one generic producer body.
trait Producer: ProviderExecution {
    /// Box the selected producer without transferring its hooks.
    fn produce(
        input: ProviderCall<Self, StreamOptionsInput<Self>>,
        trace: Trace,
        stream: AssistantMessageEventStream,
    ) -> Self::Future<'static, ()>;
}
/// The two concrete boxes are representation only.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            fn produce(
                input: ProviderCall<Self, StreamOptionsInput<Self>>,
                trace: Trace,
                stream: AssistantMessageEventStream,
            ) -> Self::Future<'static, ()> {
                Box::pin(produce(input, trace, stream))
            }
        }
    };
}
producer!(Local);
producer!(Threaded);
impl<E: Producer, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>> for HookProvider {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let submit = call.submit.clone();
        let input = ProviderCall {
            model: call.model,
            context: call.context,
            options: call.options.map(Into::into),
            submit: call.submit,
        };
        let stream = AssistantMessageEventStream::new();
        E::submit(
            E::produce(input, self.0.clone(), stream.clone()),
            submit,
            |_| panic!("supplied admission must own custom production"),
        )?;
        Ok(stream)
    }
}
/// Retain original hooks until after the callback returns and its task is admitted.
async fn produce<E: Producer>(
    call: ProviderCall<E, StreamOptionsInput<E>>,
    trace: Trace,
    stream: AssistantMessageEventStream,
) {
    let model = Arc::new(call.model);
    let common = match call.options.unwrap() {
        StreamOptionsInput::Raw(options) => options.common,
        StreamOptionsInput::Simple(options) => options.common,
    };
    record(&trace, json!(["construct"]));
    let result = callback_body(common, model.clone(), &trace).await;
    let mut message = maestro_models::providers::assistant_output::initial_message(&model);
    message.timestamp = 0.0;
    match result {
        Ok(text) => {
            message.content = vec![AssistantContent::Text(faux_text(text))];
            stream.push(AssistantMessageEvent::Done {
                reason: DoneReason::Stop,
                message: Arc::new(RwLock::new(message)),
            });
        }
        Err(error) => {
            record(&trace, json!(["failure", error.message]));
            message.content = vec![AssistantContent::Text(faux_text(""))];
            message.stop_reason = StopReason::Error;
            message.error_message = Some(error.message);
            stream.push(AssistantMessageEvent::Error {
                reason: ErrorReason::Error,
                error: Arc::new(RwLock::new(message)),
            });
        }
    }
}
/// Hook invocation belongs to the custom callback, not registry preflight.
async fn callback_body<E: ProviderExecution>(
    common: StreamOptions<E>,
    model: Arc<Model>,
    trace: &Trace,
) -> Result<String, DiagnosticErrorInfo> {
    let payload = common.on_payload.unwrap();
    let mut selected = Vec::new();
    for phase in [1, 2] {
        let input = payload(
            PayloadHookInput {
                payload: json!({"phase":phase}),
                signal: None,
            },
            model.clone(),
        )
        .await?;
        selected.push(input.payload);
    }
    record(
        trace,
        json!(["send", selected[0]["selected"], selected[1]["selected"]]),
    );
    common.on_response.unwrap()(
        ProviderResponse {
            status: 201.0,
            headers: [("x-visible".into(), "visible".into())].into(),
        },
        model,
    )
    .await?;
    record(trace, json!(["body"]));
    Ok(serde_json::to_string(&selected).unwrap())
}
/// Each hook captures a genuinely family-specific handle in its returned future.
macro_rules! hooks {
    ($name:ident, $mode:ty, $pointer:ident, $capture:expr, $expected:expr) => {
        pub(super) fn $name(trace: Trace, outcome: String) -> StreamOptions<$mode> {
            let retained = $capture;
            let response_retained = retained.clone();
            let payload_trace = trace.clone();
            let payload_outcome = outcome.clone();
            StreamOptions {
                on_payload: Some($pointer::new(move |mut input, model| {
                    let retained = retained.clone();
                    let trace = payload_trace.clone();
                    let outcome = payload_outcome.clone();
                    Box::pin(async move {
                        let phase = input.payload["phase"].as_i64().unwrap();
                        record(&trace, json!(["payload", phase]));
                        tokio::task::yield_now().await;
                        assert_eq!(model.id, "model");
                        assert_eq!(*retained, $expected);
                        if outcome == "payload_failure" {
                            return Err(DiagnosticErrorInfo::error("payload stopped"));
                        }
                        input.payload["selected"] = json!(phase * 11);
                        Ok(input)
                    })
                })),
                on_response: Some($pointer::new(move |response, model| {
                    let retained = response_retained.clone();
                    let trace = trace.clone();
                    let outcome = outcome.clone();
                    Box::pin(async move {
                        if let Some(header) = response.headers.get("x-visible") {
                            record(&trace, json!(["response", response.status, header]));
                        } else {
                            record(&trace, json!(["response", response.status]));
                        }
                        tokio::task::yield_now().await;
                        assert_eq!(model.id, "model");
                        assert_eq!(*retained, $expected);
                        if outcome == "response_failure" {
                            return Err(DiagnosticErrorInfo::error("response stopped"));
                        }
                        trace
                            .response_complete
                            .store(true, std::sync::atomic::Ordering::Release);
                        Ok(())
                    })
                })),
                ..Default::default()
            }
        }
    };
}
hooks!(local_hooks, Local, Rc, Rc::new(Cell::new(0)), Cell::new(0));
hooks!(threaded_hooks, Threaded, Arc, Arc::new(0), 0);

#[test]
fn registered_callback_preserves_hook_phases_and_counts() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    crate::chat::block_on(
        false,
        tokio::task::LocalSet::new().run_until(async {
            let trace = Trace::default();
            let owner = Arc::new(HookProvider(trace.clone()));
            register_api_provider(
                ApiProvider {
                    api: "controlled".into(),
                    stream: owner.clone(),
                    stream_simple: owner,
                },
                Some("hook-matrix".into()),
            );
            assert!(trace.lock().unwrap().is_empty());
            for row in super::corpus::load().registered.rows {
                if !row.name.starts_with("registered_") {
                    continue;
                }
                let input: CallbackInput = serde_json::from_value(row.input)?;
                for local in [true, false] {
                    trace.lock().unwrap().clear();
                    let (events, message) = run_case(&input, &trace, local).await?;
                    let observed = json!({"trace": *trace.lock().unwrap(), "result": message,
                    "events": events});
                    assert_eq!(
                        crate::json::canonical(observed),
                        row.expected,
                        "{} local={local}",
                        row.name
                    );
                }
            }
            unregister_api_providers("hook-matrix");
            Ok(())
        }),
    )
}
/// Use the recorded model identity so complete output trees remain comparable.
fn descriptor() -> Model {
    let mut model = crate::bundled::descriptor("controlled", "https://fixture.invalid").unwrap();
    model.id = "model".into();
    model.provider = "controlled".into();
    model
}
/// Preserve the original raw/simple carrier through the checked local registry call.
async fn run_local(
    input: &CallbackInput,
    common: StreamOptions<Local>,
) -> Result<(Vec<String>, AssistantMessage), Box<dyn std::error::Error>> {
    let tasks = Rc::new(std::cell::RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let spawn = super::local_submission(tasks.clone(), admissions.clone());
    let context = crate::bundled::conversation()?;
    let stream = if input.form == "raw" {
        stream_for::<Local>(
            descriptor(),
            context,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
            spawn,
        )?
    } else {
        assert_eq!(input.form, "simple");
        stream_simple_for::<Local>(
            descriptor(),
            context,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
            spawn,
        )?
    };
    assert_eq!(admissions.get(), 1);
    let output = crate::transport::drain(&stream).await?;
    for task in tasks.take() {
        task.await?;
    }
    Ok(output)
}
/// Supplied threaded admission drives that same producer and is awaited before observation closes.
async fn run_threaded(
    input: &CallbackInput,
    common: StreamOptions,
) -> Result<(Vec<String>, AssistantMessage), Box<dyn std::error::Error>> {
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let spawn = super::threaded_submission(tasks.clone());
    let context = crate::bundled::conversation()?;
    let stream = if input.form == "raw" {
        stream_for::<Threaded>(
            descriptor(),
            context,
            Some(ProviderStreamOptions {
                common,
                ..Default::default()
            }),
            spawn,
        )?
    } else {
        assert_eq!(input.form, "simple");
        stream_simple_for::<Threaded>(
            descriptor(),
            context,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
            spawn,
        )?
    };
    assert_eq!(tasks.lock().unwrap().len(), 1);
    let output = crate::transport::drain(&stream).await?;
    let tasks = std::mem::take(&mut *tasks.lock().unwrap());
    for task in tasks {
        task.await?;
    }
    Ok(output)
}

/// Select only the caller's hook representation; both paths use checked public dispatch.
async fn run_case(
    input: &CallbackInput,
    trace: &Trace,
    local: bool,
) -> Result<(Vec<String>, AssistantMessage), Box<dyn std::error::Error>> {
    if local {
        run_local(input, local_hooks(trace.clone(), input.outcome.clone())).await
    } else {
        run_threaded(input, threaded_hooks(trace.clone(), input.outcome.clone())).await
    }
}
