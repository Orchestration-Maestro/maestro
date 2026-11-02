//! Registered controlled completion and full request comparison.
use super::*;
use maestro_models::{
    ApiProvider, AssistantMessage, AssistantMessageEventStream, Context, DiagnosticErrorInfo,
    Model, SimpleStreamOptions,
};
use maestro_session::{BranchSummaryResult, GenerateBranchSummaryOptions, generate_branch_summary};
use std::sync::{Arc, Mutex, RwLock};
/// Isolated registration, removed before scenario data is released.
struct Adapter {
    /// Unique source/API key.
    api: String,
    /// Requests in invocation order.
    calls: Mutex<Vec<(Model, Context, SimpleStreamOptions)>>,
}
impl Drop for Adapter {
    fn drop(&mut self) {
        maestro_models::unregister_api_providers(&self.api);
    }
}
/// Build a full execution response from the recorded consumed response fields.
fn response(value: &Value) -> AssistantMessage {
    let mut full = json!({"role":"assistant","api":"controlled","provider":"controlled","model":"controlled","timestamp":17,"usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}});
    for (key, value) in value.as_object().unwrap() {
        full[key] = value.clone();
    }
    serde_json::from_value(full).unwrap()
}
/// Register a single response script using a non-owning callback capture.
fn adapter(script: Value) -> Arc<Adapter> {
    let api = format!("branch-{}", uuid::Uuid::new_v4());
    let state = Arc::new(Adapter {
        api: api.clone(),
        calls: Mutex::new(Vec::new()),
    });
    let weak = Arc::downgrade(&state);
    maestro_models::register_api_provider(
        ApiProvider {
            api: api.clone(),
            stream: Arc::new(UnexpectedRaw),
            stream_simple: Arc::new(BranchProvider { weak, script }),
        },
        Some(api),
    );
    state
}
/// Forward the exact supplied configuration to a uniquely registered model.
fn options(args: &Value, api: &str) -> GenerateBranchSummaryOptions {
    let base = recording()
        .rows
        .into_iter()
        .find(|row| !row.calls.is_empty())
        .unwrap()
        .calls[0]["model"]
        .clone();
    let mut model: Model = serde_json::from_value(base).unwrap();
    model.api = api.into();
    if let Some(window) = args.get("contextWindow") {
        model.context_window = window.as_f64().unwrap();
    }
    let signal = maestro_models::Cancellation::new();
    if args["signal"] == "aborted" {
        signal.abort();
    }
    GenerateBranchSummaryOptions {
        model,
        api_key: args
            .get("apiKey")
            .and_then(Value::as_str)
            .unwrap_or("controlled-key")
            .into(),
        headers: args
            .get("headers")
            .map(|value| serde_json::from_value(value.clone()).unwrap()),
        signal,
        custom_instructions: args.get("focus").and_then(Value::as_str).map(str::to_owned),
        replace_instructions: args["replace"].as_bool().unwrap_or(false),
        reserve_tokens: args.get("reserve").and_then(Value::as_f64),
        stream_fn: None,
    }
}
/// Execute and compare the complete generated object and eager requests.
pub(super) fn run(row: &NativeRow) {
    let state = adapter(row.response.clone().unwrap());
    let options = options(&row.args, &state.api);
    let entries = row.args["entries"].as_array().unwrap();
    let before = maestro_models::timestamp_now();
    let future = generate_branch_summary(entries, &options);
    let after = maestro_models::timestamp_now();
    let actual = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future);
    match actual {
        Ok(result) => {
            assert_eq!(row.expected["kind"], "value");
            let bytes = serde_json::to_string(&result).unwrap();
            assert_eq!(
                bytes,
                serde_json::to_string(&row.expected["value"]).unwrap(),
                "{}",
                row.id
            );
            let decoded: Value = serde_json::from_str(&bytes).unwrap();
            super::super::value::assert_value(&decoded, &row.expected["value"], &row.id);
        }
        Err(error) => {
            assert_eq!(row.expected["kind"], "error");
            if let Some(message) = row.expected.get("message") {
                assert_eq!(error.message, message.as_str().unwrap());
            }
        }
    }
    compare_calls(row, &state, &options, before..=after);
}
/// Compare full model/context, exact options, ordered headers and live shared signal.
fn compare_calls(
    row: &NativeRow,
    state: &Adapter,
    supplied: &GenerateBranchSummaryOptions,
    clock: std::ops::RangeInclusive<f64>,
) {
    let calls = state.calls.lock().unwrap();
    assert_eq!(calls.len(), row.calls.len(), "{}", row.id);
    for ((model, context, options), expected) in calls.iter().zip(&row.calls) {
        let mut expected = expected.clone();
        expected["model"]["api"] = json!(state.api);
        let expected_model: Model = serde_json::from_value(expected["model"].clone()).unwrap();
        assert!(model == &expected_model, "model differs");
        let mut context = serde_json::to_value(context).unwrap();
        assert!(clock.contains(&context["messages"][0]["timestamp"].as_f64().unwrap()));
        context["messages"][0]["timestamp"] =
            expected["context"]["messages"][0]["timestamp"].clone();
        super::super::value::assert_value(&context, &expected["context"], &row.id);
        super::super::summary_options::compare_options(options, &expected["options"]);
        assert_eq!(expected["sameSignal"], true);
    }
    supplied.signal.abort();
    assert!(
        calls
            .iter()
            .all(|(_, _, options)| options.common.signal.as_ref().unwrap().is_aborted())
    );
}

/// Hold the producer across caller input release, then observe its original signal.
pub(super) fn lifetime() {
    let case = recording().lifetime;
    assert_eq!(
        case.test,
        "branch_completion_owns_request_after_inputs_drop"
    );
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let pending = Arc::new(Mutex::new(None));
    let state = adapter(json!({}));
    register_pending(&state, &events, &pending);
    let options = options(&json!({}), &state.api);
    let signal = options.signal.clone();
    let entries = vec![json!({"type":"message","message":{"role":"user","content":"owned"}})];
    let future = generate_branch_summary(&entries, &options);
    assert_eq!(*events.lock().unwrap(), ["request-start"]);
    drop(entries);
    drop(options);
    events.lock().unwrap().push("caller-inputs-released".into());
    signal.abort();
    let calls = state.calls.lock().unwrap();
    assert!(calls[0].2.common.signal.as_ref().unwrap().is_aborted());
    drop(calls);
    events
        .lock()
        .unwrap()
        .push("shared-cancellation-observed".into());
    let stream = pending.lock().unwrap().take().unwrap();
    stream.end(Some(Arc::new(RwLock::new(response(&json!({"role":"assistant","content":[{"type":"text","text":"after release"}],"stopReason":"stop"}))))));
    let result = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
        .unwrap();
    events.lock().unwrap().push("observed-result".into());
    assert_eq!(*events.lock().unwrap(), case.events);
    super::super::value::assert_value(
        &serde_json::to_value(result).unwrap(),
        &case.result,
        "lifetime",
    );
    let weak = Arc::downgrade(&state);
    drop(state);
    assert!(weak.upgrade().is_none());
}

/// A supplied stream function receives the request and its options in place of the
/// registered completion, whose adapter is never called.
pub(super) fn stream_function() {
    let state = adapter(
        json!({"content":[{"type":"text","text":"from the registered completion"}],"stopReason":"stop"}),
    );
    let mut options = options(&json!({}), &state.api);
    let seen = Arc::new(Mutex::new(Vec::<(Option<String>, Option<f64>)>::new()));
    let record = Arc::clone(&seen);
    options.stream_fn = Some(Arc::new(move |_, _, simple, _| {
        record
            .lock()
            .unwrap()
            .push((simple.common.api_key.clone(), simple.common.max_tokens));
        Box::pin(async {
            let stream = AssistantMessageEventStream::new();
            stream.end(Some(Arc::new(RwLock::new(response(&json!({"role":"assistant","content":[{"type":"text","text":"from the stream function"}],"stopReason":"stop"}))))));
            Ok(stream)
        })
    }));
    let entries = vec![json!({"type":"message","message":{"role":"user","content":"owned"}})];
    let result = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(generate_branch_summary(&entries, &options))
        .unwrap();
    let BranchSummaryResult::Summary { summary, .. } = result else {
        panic!("expected a summary, got {result:?}");
    };
    assert!(summary.contains("from the stream function"), "{summary}");
    assert_eq!(
        *seen.lock().unwrap(),
        [(Some("controlled-key".to_owned()), Some(2048.0))]
    );
    assert!(state.calls.lock().unwrap().is_empty());
}

/// Install a producer that settles only when its retained stream is explicitly ended.
fn register_pending(
    state: &Arc<Adapter>,
    events: &Arc<Mutex<Vec<String>>>,
    pending: &Arc<Mutex<Option<AssistantMessageEventStream>>>,
) {
    let captured = Arc::downgrade(state);
    let (recorded, retained) = (Arc::clone(events), Arc::clone(pending));
    maestro_models::register_api_provider(
        ApiProvider {
            api: state.api.clone(),
            stream: Arc::new(UnexpectedRaw),
            stream_simple: Arc::new(PendingBranch {
                captured,
                recorded,
                retained,
            }),
        },
        Some(state.api.clone()),
    );
}

/// Retained invocation owner for the controlled provider.
struct BranchProvider {
    weak: std::sync::Weak<Adapter>,
    script: Value,
}
impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    > for BranchProvider
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(
        &self,
        call: maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    ) -> Self::Return {
        let maestro_models::ProviderCall {
            model,
            context,
            options,
            ..
        } = call;
        let weak = &self.weak;
        let script = &self.script;

        let state = weak.upgrade().unwrap();
        state.calls.lock().unwrap().push((
            model,
            context,
            crate::support::summary_options::observe(options.unwrap()),
        ));
        if let Some(error) = script.get("throw") {
            return Err(DiagnosticErrorInfo::error(error.as_str().unwrap()));
        }
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(Arc::new(RwLock::new(response(script)))));
        Ok(stream)
    }
}

/// Retained invocation owner for the controlled provider.
struct UnexpectedRaw;
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for UnexpectedRaw
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        panic!("raw dispatch selected")
    }
}

/// Retained invocation owner for the controlled provider.
struct PendingBranch {
    captured: std::sync::Weak<Adapter>,
    recorded: Arc<Mutex<Vec<String>>>,
    retained: Arc<Mutex<Option<AssistantMessageEventStream>>>,
}
impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    > for PendingBranch
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(
        &self,
        call: maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    ) -> Self::Return {
        let maestro_models::ProviderCall {
            model,
            context,
            options,
            ..
        } = call;
        let captured = &self.captured;
        let recorded = &self.recorded;
        let retained = &self.retained;

        let state = captured.upgrade().unwrap();
        state.calls.lock().unwrap().push((
            model,
            context,
            crate::support::summary_options::observe(options.unwrap()),
        ));
        recorded.lock().unwrap().push("request-start".into());
        let stream = AssistantMessageEventStream::new();
        *retained.lock().unwrap() = Some(stream.clone());
        Ok(stream)
    }
}
