//! Real queued factories preserve hooks, order and their retained registration.
#![cfg(test)]
use super::registered::{CallbackInput, Trace, local_hooks, record, threaded_hooks};
use maestro_models::*;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex},
};

/// The same factory body runs with either invocation family's original options.
struct Factory {
    /// Observations belong to the current request, not a synthesized result label.
    trace: Trace,
    /// Caller-selected failure after the hooks have completed.
    outcome: String,
}
/// Erase only the associated future; factory logic never selects execution modes.
macro_rules! factory {
    ($mode:ty) => {
        impl Invoke<$mode, FauxResponseInput<$mode>> for Factory {
            type Return = <$mode as Execution>::Future<
                'static,
                Result<AssistantMessage, DiagnosticErrorInfo>,
            >;
            fn invoke(&self, input: FauxResponseInput<$mode>) -> Self::Return {
                Box::pin(factory_body(
                    input,
                    self.trace.clone(),
                    self.outcome.clone(),
                ))
            }
        }
    };
}
factory!(Local);
factory!(Threaded);
/// Observe live state after the response hook, then retain it across a real yield.
fn factory_body<E: ProviderExecution>(
    input: FauxResponseInput<E>,
    trace: Trace,
    outcome: String,
) -> impl std::future::Future<Output = Result<AssistantMessage, DiagnosticErrorInfo>> {
    assert!(
        trace
            .response_complete
            .load(std::sync::atomic::Ordering::Acquire)
    );
    record(&trace, json!(["factory", input.state.call_count()]));
    async move {
        tokio::task::yield_now().await;
        assert_eq!(input.model.id, "model");
        assert_eq!(input.context.messages.len(), 1);
        let common = match input.options.unwrap() {
            StreamOptionsInput::Raw(options) => options.common,
            StreamOptionsInput::Simple(options) => options.common,
        };
        let mut selected = Vec::new();
        for phase in [1, 2] {
            let output = common.on_payload.as_ref().unwrap()(
                PayloadHookInput {
                    payload: json!({"phase":phase}),
                    signal: None,
                },
                Arc::new(input.model.clone()),
            )
            .await?;
            selected.push(output.payload["selected"].clone());
        }
        if outcome == "factory_failure" {
            return Err(DiagnosticErrorInfo::error("factory stopped"));
        }
        record(&trace, json!(["selected", selected[0], selected[1]]));
        Ok(faux_assistant_message(
            "ok",
            FauxAssistantMessageOptions {
                timestamp: Some(0.0),
                ..Default::default()
            },
        ))
    }
}
/// Deterministic short chunks use the existing simulator pacing owner.
fn registration() -> FauxProviderRegistration {
    register_faux_provider(RegisterFauxProviderOptions {
        api: Some("controlled-scripted".into()),
        models: Some(vec![FauxModelDefinition {
            id: "model".into(),
            ..Default::default()
        }]),
        token_size: Some(FauxTokenSize {
            min: Some(1.0),
            max: Some(1.0),
        }),
        ..Default::default()
    })
}
/// Retain one callback alias while replacing and removing its registry entry.
fn enqueue(
    registration: &FauxProviderRegistration,
    trace: &Trace,
    outcome: &str,
) -> std::sync::Weak<Factory> {
    let factory = Arc::new(Factory {
        trace: trace.clone(),
        outcome: outcome.into(),
    });
    let weak = Arc::downgrade(&factory);
    registration.set_responses(vec![FauxResponseStep::Factory(factory)]);
    weak
}
/// Record only the fields present in the source recording, keeping all events.
fn observation(
    trace: &Trace,
    registration: &FauxProviderRegistration,
    result: (Vec<String>, AssistantMessage),
) -> Value {
    let (events, message) = result;
    let mut value = json!({ "trace": *trace.lock().unwrap(), "events": events,
        "callCount": registration.state.call_count(),
        "pending": registration.get_pending_response_count(), "stopReason": message.stop_reason });
    if let Some(error) = message.error_message {
        value["errorMessage"] = json!(error);
    } else {
        let [AssistantContent::Text(text)] = message.content.as_slice() else {
            panic!("one text block must remain")
        };
        value["text"] = json!(text.text);
    }
    value
}
#[test]
fn scripted_factories_keep_hooks_across_suspension() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    crate::chat::block_on(
        false,
        tokio::task::LocalSet::new().run_until(async {
            for row in super::corpus::load().scripted.rows {
                let input: CallbackInput = serde_json::from_value(row.input)?;
                for local in [true, false] {
                    let trace = Trace::default();
                    let registration = registration();
                    let weak = enqueue(&registration, &trace, &input.outcome);
                    let retained = get_api_provider(&registration.api).unwrap();
                    registration.unregister();
                    assert!(trace.lock().unwrap().is_empty());
                    let result = run(&registration, &retained, &trace, &input, local).await?;
                    assert_eq!(
                        crate::json::canonical(observation(&trace, &registration, result)),
                        row.expected,
                        "{} local={local}",
                        row.name
                    );
                    assert!(weak.upgrade().is_none());
                }
            }
            Ok(())
        }),
    )
}
/// Local and supplied threaded routes both close admission before counting the queue.
async fn run(
    registration: &FauxProviderRegistration,
    provider: &ApiProvider,
    trace: &Trace,
    input: &CallbackInput,
    local: bool,
) -> Result<(Vec<String>, AssistantMessage), Box<dyn std::error::Error>> {
    trace
        .response_complete
        .store(false, std::sync::atomic::Ordering::Release);
    if local {
        run_local(registration, provider, trace, input).await
    } else {
        run_threaded(registration, provider, trace, input).await
    }
}
/// Execute the retained local callback with non-Send hook futures.
async fn run_local(
    registration: &FauxProviderRegistration,
    provider: &ApiProvider,
    trace: &Trace,
    input: &CallbackInput,
) -> Result<(Vec<String>, AssistantMessage), Box<dyn std::error::Error>> {
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let admissions = Rc::new(Cell::new(0));
    let spawn = super::local_submission(tasks.clone(), admissions.clone());
    let common = local_hooks(trace.clone(), input.outcome.clone());
    let stream = invoke::<Local>(
        registration,
        provider,
        input,
        common,
        Local::provided(spawn),
    )?;
    assert_eq!(admissions.get(), 1);
    assert!(trace.lock().unwrap().is_empty());
    let result = crate::transport::drain(&stream).await?;
    for task in tasks.take() {
        task.await?;
    }
    Ok(result)
}
/// Threaded hooks use the same factory and never fall back to simulator admission.
async fn run_threaded(
    registration: &FauxProviderRegistration,
    provider: &ApiProvider,
    trace: &Trace,
    input: &CallbackInput,
) -> Result<(Vec<String>, AssistantMessage), Box<dyn std::error::Error>> {
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let retained = tasks.clone();
    let spawn: ProviderSpawner<Threaded> = Arc::new(move |task| {
        let task = tokio::spawn(task);
        retained.lock().unwrap().push(task);
        true
    });
    let common = threaded_hooks(trace.clone(), input.outcome.clone());
    let stream = invoke::<Threaded>(
        registration,
        provider,
        input,
        common,
        Threaded::provided(spawn),
    )?;
    assert_eq!(tasks.lock().unwrap().len(), 1);
    assert!(trace.lock().unwrap().is_empty());
    let result = crate::transport::drain(&stream).await?;
    let tasks = std::mem::take(&mut *tasks.lock().unwrap());
    for task in tasks {
        task.await?;
    }
    Ok(result)
}
/// The real callback pops and increments synchronously before either producer can run.
fn invoke<E: ProviderExecution>(
    registration: &FauxProviderRegistration,
    provider: &ApiProvider,
    input: &CallbackInput,
    common: StreamOptions<E>,
    submit: E::Submission,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let model = registration.get_model(None).unwrap().clone();
    let context = crate::bundled::conversation().unwrap();
    let count = registration.state.call_count();
    let pending = registration.get_pending_response_count();
    let stream = if input.form == "raw" {
        provider.stream.call(
            model,
            context,
            E::submission::<ProviderStreamOptions>(
                Some(ProviderStreamOptions {
                    common,
                    ..Default::default()
                }),
                submit,
            ),
        )?
    } else {
        assert_eq!(input.form, "simple");
        provider.stream_simple.call(
            model,
            context,
            E::submission::<SimpleStreamOptions>(
                Some(SimpleStreamOptions {
                    common,
                    ..Default::default()
                }),
                submit,
            ),
        )?
    };
    assert_eq!(registration.state.call_count(), count + 1);
    assert_eq!(
        registration.get_pending_response_count(),
        pending.saturating_sub(1)
    );
    Ok(stream)
}
#[test]
fn scripted_queue_and_registration_survive_cleanup_across_families() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    crate::chat::block_on(
        false,
        tokio::task::LocalSet::new().run_until(async {
            let trace = Trace::default();
            let registration = registration();
            let factory = Arc::new(Factory {
                trace: trace.clone(),
                outcome: "success".into(),
            });
            let weak = Arc::downgrade(&factory);
            registration.set_responses(vec![FauxResponseStep::Factory(factory.clone())]);
            registration.append_responses(vec![FauxResponseStep::Factory(factory)]);
            let retained = get_api_provider(&registration.api).unwrap();
            registration.unregister();
            assert!(get_api_provider(&registration.api).is_none());
            let corpus = super::corpus::load().retained_sequence;
            assert_eq!(corpus.observations.len(), 2);
            for (index, (form, local)) in [("raw", true), ("simple", false)].into_iter().enumerate()
            {
                trace.lock().unwrap().clear();
                let input = CallbackInput {
                    form: form.into(),
                    outcome: "success".into(),
                };
                let result = run(&registration, &retained, &trace, &input, local).await?;
                let mut observed = observation(&trace, &registration, result);
                observed.as_object_mut().unwrap().shift_remove("events");
                assert_eq!(crate::json::canonical(observed), corpus.observations[index]);
                assert_eq!(weak.upgrade().is_some(), index == 0);
            }
            drop(registration);
            drop(retained);
            assert!(weak.upgrade().is_none());
            Ok(())
        }),
    )
}

/// Recorded source-factory outcomes, not a second callback dispatch mechanism.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceInput {
    /// Async context, supplied exception, or exhausted FIFO.
    outcome: String,
}
/// The factory reads the live context/state after suspension.
struct SourceFactory(String);
/// Erase only each concrete factory return.
macro_rules! source_factory {
    ($mode:ty) => {
        impl Invoke<$mode, FauxResponseInput<$mode>> for SourceFactory {
            type Return = <$mode as Execution>::Future<
                'static,
                Result<AssistantMessage, DiagnosticErrorInfo>,
            >;
            fn invoke(&self, input: FauxResponseInput<$mode>) -> Self::Return {
                Box::pin(source_body(input, self.0.clone()))
            }
        }
    };
}
source_factory!(Local);
source_factory!(Threaded);
/// Preserve caller error text and the selected live context/count.
async fn source_body<E: ProviderExecution>(
    input: FauxResponseInput<E>,
    outcome: String,
) -> Result<AssistantMessage, DiagnosticErrorInfo> {
    tokio::task::yield_now().await;
    match outcome.as_str() {
        "async_context" => Ok(faux_assistant_message(
            format!(
                "{}:{}",
                input.context.messages.len(),
                input.state.call_count()
            ),
            FauxAssistantMessageOptions::default(),
        )),
        "factory_throws" => Err(DiagnosticErrorInfo::error("boom")),
        _ => panic!("only source factory inputs are admitted"),
    }
}
#[test]
fn recorded_source_factories_keep_all_fields() -> crate::TestResult {
    let _serial = crate::bundled::registry();
    crate::chat::block_on(
        false,
        tokio::task::LocalSet::new().run_until(async {
            for row in super::corpus::load()
                .dispatch
                .rows
                .into_iter()
                .filter(|row| row.input.get("outcome").is_some())
            {
                let input: SourceInput = serde_json::from_value(row.input)?;
                for local in [true, false] {
                    let registration = registration();
                    queue_source(&registration, &input.outcome);
                    let provider = get_api_provider(&registration.api).unwrap();
                    let trace = Trace::default();
                    let call = CallbackInput {
                        form: "raw".into(),
                        outcome: "success".into(),
                    };
                    let result = run(&registration, &provider, &trace, &call, local).await?;
                    let observed = source_observation(&trace, &registration, result);
                    assert_eq!(observed, row.expected, "{} local={local}", row.name);
                    registration.unregister();
                }
            }
            Ok(())
        }),
    )
}
/// Exhaustion is represented by an empty real queue, not by a fabricated factory error.
fn queue_source(registration: &FauxProviderRegistration, outcome: &str) {
    if outcome == "exhausted" {
        return;
    }
    registration.set_responses(vec![FauxResponseStep::Factory(Arc::new(SourceFactory(
        outcome.into(),
    )))]);
}

/// The source-factory recording names the actual error field `error` and has no phase trace.
fn source_observation(
    trace: &Trace,
    registration: &FauxProviderRegistration,
    result: (Vec<String>, AssistantMessage),
) -> Value {
    let mut observed = observation(trace, registration, result);
    let object = observed.as_object_mut().unwrap();
    object.shift_remove("trace");
    if let Some(error) = object.shift_remove("errorMessage") {
        object.insert("error".into(), error);
    }
    observed
}
