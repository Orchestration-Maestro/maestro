//! Summary corpus consumer and independently captured completion requests.
use maestro_models::{
    ApiProvider, AssistantMessage, AssistantMessageEventStream, Context, DiagnosticErrorInfo,
    Model, SimpleStreamOptions,
};
use maestro_session::{
    CompactionPreparation, CompactionSettings, Presence, SummaryOptions, compact, generate_summary,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};
/// Recorded operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Case label.
    id: String,
    /// Public test owning this row.
    test: String,
    /// Selected operation.
    operation: String,
    /// Supplied operands.
    args: Value,
    /// Controlled responses.
    responses: Value,
    /// Expected result.
    expected: Value,
    /// Captured request sequence.
    calls: Vec<Value>,
}
/// Execute the selected group with a fresh registry identity per case.
pub fn group(name: &str) {
    let rows = corpus().rows;
    let selected: Vec<_> = rows.into_iter().filter(|r| r.test == name).collect();
    assert!(!selected.is_empty());
    for row in selected {
        run(&row);
    }
}
/// Retained capture; registry entries are removed by their unique source identity.
pub struct Adapter {
    /// Isolated registry identity.
    api: String,
    /// Supplied response scripts.
    responses: Value,
    /// Requests in invocation order.
    calls: Mutex<Vec<(Model, Context, SimpleStreamOptions)>>,
}
impl Drop for Adapter {
    fn drop(&mut self) {
        maestro_models::unregister_api_providers(&self.api);
    }
}
/// Create an isolated registered completion adapter.
fn adapter(responses: Value) -> Arc<Adapter> {
    let api = format!("summary-{}", uuid::Uuid::new_v4());
    let state = Arc::new(Adapter {
        api: api.clone(),
        responses,
        calls: Mutex::new(vec![]),
    });
    let retained = Arc::downgrade(&state);
    maestro_models::register_api_provider(
        ApiProvider {
            api: api.clone(),
            stream: Arc::new(UnexpectedRaw),
            stream_simple: Arc::new(SummaryProvider(retained)),
        },
        Some(api),
    );
    state
}
/// Controlled success response using complete execution records only at the adapter seam.
fn default_response() -> Value {
    json!({"role":"assistant","content":[{"type":"text","text":"## Goal\nControlled summary"}],"api":"controlled","provider":"controlled","model":"controlled","timestamp":1_710_000_000_124_f64,"usage":{"input":10,"output":10,"cacheRead":0,"cacheWrite":0,"totalTokens":20,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop"})
}
/// Construct a raw preparation exactly as supplied by the corpus.
fn preparation(args: &Value) -> CompactionPreparation {
    CompactionPreparation {
        first_kept_entry_id: args
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("kept")
            .into(),
        messages_to_summarize: args.get("messages").map_or_else(
            || vec![json!({"role":"user","content":"history","timestamp":17})],
            |v| v.as_array().unwrap().clone(),
        ),
        turn_prefix_messages: args
            .get("prefix")
            .map_or_else(Vec::new, |v| v.as_array().unwrap().clone()),
        is_split_turn: args["split"].as_bool().unwrap_or(false),
        tokens_before: args["tokens"].as_f64().unwrap_or(12_345.0),
        previous_summary: args.get("previous").map_or(Presence::Missing, |v| {
            serde_json::from_value(v.clone()).unwrap()
        }),
        file_ops: serde_json::from_value(
            args.get("files")
                .cloned()
                .unwrap_or_else(|| json!({"read":[],"written":[],"edited":[]})),
        )
        .unwrap(),
        settings: CompactionSettings {
            enabled: false,
            reserve_tokens: args["reserve"].as_f64().unwrap_or(2000.0),
            keep_recent_tokens: 0.0,
        },
    }
}
/// Obtain a full supplied model; invalid-input rows intentionally have no calls.
fn model(row: &Row, api: &str) -> Model {
    let mut value = row.calls.first().map_or_else(
        || {
            let rows = corpus().rows;
            rows[0].calls[0]["model"].clone()
        },
        |call| call["model"].clone(),
    );
    value["api"] = json!(api);
    serde_json::from_value(value).unwrap()
}
/// Execute one operation and compare its complete captured requests and result.
fn run(row: &Row) {
    let state = adapter(row.responses.clone());
    let model = model(row, &state.api);
    let options = options(&row.args);
    let preparation = preparation(&row.args);
    let preparation_before = serde_json::to_value(&preparation).unwrap();
    let before = maestro_models::timestamp_now();
    let future = match row.operation.as_str() {
        "generateSummary" => {
            let future = generate_summary(
                row.args["messages"].as_array().unwrap(),
                &model,
                row.args
                    .get("reserve")
                    .and_then(Value::as_f64)
                    .unwrap_or(2000.0),
                row.args.get("previous").and_then(Value::as_str),
                &options,
            );
            Box::pin(async move { future.await.map(Value::String) }) as maestro_models::BoxFuture<_>
        }
        "compact" => {
            let future = compact(&preparation, &model, &options);
            Box::pin(async move {
                future
                    .await
                    .map(|result| serde_json::to_value(result).unwrap())
            })
        }
        _ => panic!("unknown operation"),
    };
    let actual = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future);
    match row.expected["kind"].as_str().unwrap() {
        "value" => super::value::assert_value(&actual.unwrap(), &row.expected["value"], &row.id),
        "error" => {
            let error = actual.unwrap_err();
            if let Some(message) = row.expected.get("message") {
                assert_eq!(error.message, message.as_str().unwrap());
            }
        }
        _ => panic!("unknown outcome"),
    }
    assert_eq!(
        serde_json::to_value(&preparation).unwrap(),
        preparation_before
    );
    let after = maestro_models::timestamp_now();
    compare_calls(row, &state, &options, before..=after);
}
/// Compare the consuming request, including shared cancellation and ordered headers.
fn compare_calls(
    row: &Row,
    state: &Adapter,
    supplied: &SummaryOptions,
    clock: std::ops::RangeInclusive<f64>,
) {
    let calls = state.calls.lock().unwrap();
    assert_eq!(calls.len(), row.calls.len(), "{}", row.id);
    for ((model, context, options), expected) in calls.iter().zip(&row.calls) {
        let mut expected = expected.clone();
        expected["model"]["api"] = json!(state.api);
        let expected_model: Model = serde_json::from_value(expected["model"].clone()).unwrap();
        let same_model = model == &expected_model;
        assert!(same_model, "model differs");
        let mut context = serde_json::to_value(context).unwrap();
        let timestamp = context["messages"][0]["timestamp"].as_f64().unwrap();
        assert!(clock.contains(&timestamp));
        context["messages"][0]["timestamp"] =
            expected["context"]["messages"][0]["timestamp"].clone();
        assert_eq!(context, expected["context"], "{}", row.id);
        let prefix = context["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("This is the PREFIX of a turn");
        assert_eq!(expected["kind"], if prefix { "prefix" } else { "history" });
        compare_options(options, &expected["options"]);
    }
    if let Some(signal) = &supplied.signal {
        signal.abort();
        assert!(
            calls.iter().all(|(_, _, options)| options
                .common
                .signal
                .as_ref()
                .unwrap()
                .is_aborted())
        );
    }
}

/// Consumed corpus sections, with source-only metadata omitted.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Request/result cases.
    rows: Vec<Row>,
    /// Pending-sibling cases.
    lifetime: Vec<Lifetime>,
    /// Full-history controlled summaries.
    large: Large,
}
/// Observable pending-sibling transaction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lifetime {
    /// Failed side.
    failing: String,
    /// Outer failure.
    error: String,
    /// Ordered start/reject/settlement sequence.
    events: Vec<String>,
}
/// Consumed full-history observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Large {
    /// Controlled history response.
    history: String,
    /// Controlled prefix response.
    prefix: String,
    /// Complete appended result.
    summary: String,
    /// Original context size.
    before: usize,
    /// Compacted context size.
    after: usize,
}
/// Decode all sections strictly.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../../fixtures/summary_generation.json")).unwrap()
}
/// Register an adapter answering with `text` and `usage`, with a model and controls for it.
pub fn answering(text: &str, usage: &Value) -> (Arc<Adapter>, Model, SummaryOptions) {
    let mut response = default_response();
    response["content"][0]["text"] = json!(text);
    response["usage"] = usage.clone();
    let state = adapter(json!({"history": response}));
    let model = model(&corpus().rows[0], &state.api);
    (state, model, options(&json!({})))
}
/// Construct the exact supplied control operands.
fn options(args: &Value) -> SummaryOptions {
    let signal = args.get("signal").map(|value| {
        let signal = maestro_models::Cancellation::new();
        if value == "aborted" {
            signal.abort();
        }
        signal
    });
    SummaryOptions {
        api_key: args
            .get("apiKey")
            .and_then(Value::as_str)
            .unwrap_or("controlled-key")
            .into(),
        headers: args
            .get("headers")
            .map(|v| serde_json::from_value(v.clone()).unwrap()),
        signal,
        custom_instructions: args.get("focus").and_then(Value::as_str).map(str::to_owned),
        thinking_level: args
            .get("level")
            .map(|v| serde_json::from_value(v.clone()).unwrap()),
    }
}

use super::summary_options::compare_options;

/// Controlled sibling lifetime witnesses.
mod lifetime;
pub use lifetime::lifetime;
/// Full-history generation and application.
mod large;
pub use large::large_history;

/// Retained invocation owner for the controlled provider.
struct SummaryProvider(std::sync::Weak<Adapter>);
impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    > for SummaryProvider
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
        let retained = &self.0;

        let state = retained.upgrade().unwrap();
        let prefix = serde_json::to_value(&context).unwrap()["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("This is the PREFIX of a turn");
        let response = state
            .responses
            .get(if prefix { "prefix" } else { "history" })
            .cloned()
            .unwrap_or_else(default_response);
        state.calls.lock().unwrap().push((
            model,
            context,
            crate::support::summary_options::observe(options.unwrap()),
        ));
        if let Some(message) = response.get("throw") {
            return Err(DiagnosticErrorInfo::error(message.as_str().unwrap()));
        }
        let response: AssistantMessage = serde_json::from_value(response).unwrap();
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(Arc::new(RwLock::new(response))));
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
