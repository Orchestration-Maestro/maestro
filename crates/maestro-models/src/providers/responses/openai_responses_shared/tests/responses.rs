//! Fixture rows, builders and reduction runs shared by the response-protocol tests.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use super::super::messages::{convert_responses_messages, convert_responses_tools};
use super::super::{
    ConvertResponsesMessagesOptions, ConvertResponsesToolsOptions, OpenAIResponsesStreamOptions,
    process_responses_stream,
};
use crate as maestro_models;
use crate::providers::json_text::{is_truthy, member, raw_json};
use maestro_models::{
    AssistantMessage, AssistantMessageEventStream, Context, DiagnosticCode, DiagnosticErrorInfo,
    Model, SharedAssistantMessage, Tool, Usage,
};
use serde_json::value::RawValue;
use serde_json::{Map, Value, json};

use super::TestResult;

/// A fixture object whose members must all be read before the row is accepted.
pub struct Fields {
    /// Name of the row or member, for failure messages.
    owner: String,
    /// Members not read yet.
    members: Map<String, Value>,
}

impl Fields {
    /// Wrap a fixture object.
    pub fn new(owner: &str, value: Value) -> TestResult<Self> {
        match value {
            Value::Object(members) => Ok(Self {
                owner: owner.to_owned(),
                members,
            }),
            other => Err(format!("{owner} is not an object: {other}").into()),
        }
    }

    /// Read a member, removing it from the unread ones.
    pub fn take(&mut self, key: &str) -> Option<Value> {
        self.members.remove(key)
    }

    /// Read a member the row must have.
    pub fn require(&mut self, key: &str) -> TestResult<Value> {
        self.take(key)
            .ok_or_else(|| format!("{} lacks {key}", self.owner).into())
    }

    /// Name of the row.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Accept the row only when every member was read.
    pub fn finish(self) -> TestResult {
        if self.members.is_empty() {
            return Ok(());
        }
        let unread: Vec<&String> = self.members.keys().collect();
        Err(format!("{} has unread members {unread:?}", self.owner).into())
    }
}

/// The rows a test owns, checked to be exactly `count`.
pub fn rows(fixture: &str, test: &str, count: usize) -> TestResult<Vec<Fields>> {
    let all: Vec<Value> = serde_json::from_str(fixture)?;
    let owned: Vec<Value> = all.into_iter().filter(|row| row["test"] == test).collect();
    assert_eq!(owned.len(), count, "{test} owns {count} rows");
    owned
        .into_iter()
        .map(|row| {
            let id = row["id"].as_str().unwrap_or_default().to_owned();
            let mut fields = Fields::new(&id, row)?;
            fields.take("test");
            fields.take("id");
            Ok(fields)
        })
        .collect()
}

/// Fail when two rows of a fixture ask the same question, whichever test owns them.
///
/// A row's question is every member except its owner, name and expectation.
pub fn assert_unique_queries(fixture: &str) -> TestResult {
    let all: Vec<Value> = serde_json::from_str(fixture)?;
    let mut seen = BTreeSet::new();
    for mut row in all {
        let id = row["id"].to_string();
        if let Some(members) = row.as_object_mut() {
            for key in ["test", "id", "expected"] {
                members.remove(key);
            }
        }
        for event in row
            .get_mut("events")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(raw) = event.as_str()
                && let Ok(parsed) = serde_json::from_str::<Value>(raw)
            {
                *event = parsed;
            }
        }
        if !seen.insert(row.to_string()) {
            return Err(format!("row {id} repeats an earlier query").into());
        }
    }
    Ok(())
}

/// Rewrite integral floats as integers so values compare as their JSON text would.
pub fn canonical(value: Value) -> Value {
    match value {
        Value::Number(number) => number
            .as_f64()
            .filter(|float| number.is_f64() && float.fract() == 0.0 && float.abs() < 9.0e18)
            .and_then(|float| format!("{float}").parse::<serde_json::Number>().ok())
            .map_or(Value::Number(number), Value::Number),
        Value::Array(items) => Value::Array(items.into_iter().map(canonical).collect()),
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect(),
        ),
        other => other,
    }
}

/// The model every row uses unless it overrides members.
pub fn model(overrides: Option<Value>) -> TestResult<Model> {
    let mut value = json!({
        "id": "fixture", "name": "Fixture", "api": "openai-responses", "provider": "openai",
        "baseUrl": "https://example.invalid/v1", "reasoning": true, "input": ["text", "image"],
        "cost": {"input": 2, "output": 8, "cacheRead": 0.5, "cacheWrite": 3},
        "contextWindow": 1000, "maxTokens": 100
    });
    if let (Some(base), Some(Value::Object(changes))) = (value.as_object_mut(), overrides) {
        base.extend(changes);
    }
    Ok(serde_json::from_value(value)?)
}

/// An all-zero usage record.
pub fn zero_usage() -> Value {
    json!({"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}})
}

/// Fill the members a fixture message leaves out, then decode the context.
pub fn context(compact: Value) -> TestResult<Context> {
    let mut value = compact;
    let messages = value
        .get_mut("messages")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten();
    for message in messages {
        let defaults = match message["role"].as_str() {
            Some("assistant") => json!({
                "api": "openai-responses", "provider": "openai", "model": "fixture",
                "usage": zero_usage(), "stopReason": "stop", "timestamp": 1
            }),
            Some("user") => json!({"timestamp": 0}),
            _ => json!({"toolName": "lookup", "isError": false, "timestamp": 2}),
        };
        if let (Some(target), Some(defaults)) = (message.as_object_mut(), defaults.as_object()) {
            for (key, default) in defaults {
                target.entry(key.clone()).or_insert_with(|| default.clone());
            }
        }
    }
    Ok(serde_json::from_value(value)?)
}

/// Providers whose tool-call identifiers keep an item part.
pub fn allowed_providers() -> HashSet<String> {
    ["openai", "openai-codex", "opencode"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// Convert the history of a row and compare it with the row's expectation.
pub fn check_messages(mut row: Fields) -> TestResult {
    let model = model(row.take("model"))?;
    let history = context(row.require("context")?)?;
    let options = row
        .take("options")
        .map(|options| ConvertResponsesMessagesOptions {
            include_system_prompt: options["includeSystemPrompt"].as_bool().unwrap_or(true),
        });
    let expected = row.require("expected")?;
    let converted =
        convert_responses_messages(&model, &history, &allowed_providers(), options.as_ref());
    let id = row.owner().to_owned();
    row.finish()?;
    check_conversion(&id, converted, expected)
}

/// Compare a conversion outcome with a fixture expectation: wire items, or `{"error": text}`.
pub fn check_conversion(
    id: &str,
    converted: Result<Vec<Value>, DiagnosticErrorInfo>,
    expected: Value,
) -> TestResult {
    match (converted, expected) {
        (Ok(items), expected @ Value::Array(_)) => {
            assert_eq!(canonical(Value::Array(items)), canonical(expected), "{id}");
        }
        (Err(failure), Value::Object(mut error)) => {
            assert_eq!(failure.name, None, "{id}: a native cause carries no name");
            assert_eq!(
                Some(Value::String(failure.message)),
                error.remove("error"),
                "{id}"
            );
        }
        (outcome, expected) => {
            return Err(format!("{id}: got {outcome:?}, expected {expected}").into());
        }
    }
    Ok(())
}

/// Convert the tools of a row and compare them with the row's expectation.
pub fn check_tools(mut row: Fields) -> TestResult {
    let tools: Vec<Tool> = serde_json::from_value(row.require("tools")?)?;
    let options = row
        .take("options")
        .map(|options| ConvertResponsesToolsOptions {
            strict: options["strict"].as_bool(),
        });
    let expected = row.require("expected")?;
    let converted = convert_responses_tools(&tools, options.as_ref());
    assert_eq!(
        canonical(Value::Array(converted)),
        canonical(expected),
        "{}",
        row.owner()
    );
    row.finish()
}

/// What one reduction left behind.
pub struct Reduction {
    /// Result the reducer returned.
    pub outcome: Result<(), DiagnosticErrorInfo>,
    /// The shared message after the reduction.
    pub message: AssistantMessage,
    /// The updates the reducer published, without their message handles.
    pub events: Vec<Value>,
    /// Service tiers the callbacks observed, in order.
    pub tiers: Vec<Value>,
}

/// Everything a reduction is run with.
pub struct Script {
    /// Model the response answers.
    pub model: Model,
    /// Usage the message holds before the first event.
    pub usage: Value,
    /// Event texts the source yields in order.
    pub events: Vec<String>,
    /// Failure the source yields instead of ending.
    pub failure: Option<DiagnosticErrorInfo>,
    /// Tier options of the row: `serviceTier`, `pricing` and `resolver`.
    pub options: Value,
}

impl Script {
    /// A reduction of `events` with the shared model and no callbacks.
    pub fn of(events: Vec<String>) -> TestResult<Self> {
        Ok(Self {
            model: model(None)?,
            usage: zero_usage(),
            events,
            failure: None,
            options: json!({}),
        })
    }
}

/// Fail to compile when a future is not `Send`, so endpoint tasks can own the reduction.
fn ensure_send<T: Send>(future: T) -> T {
    future
}

/// The cost multiplier a tier name stands for in the pricing callback.
fn tier_multiplier(tier: Option<&str>) -> f64 {
    match tier {
        Some("chosen") => 3.0,
        Some("priority") => 2.0,
        Some("flex") => 0.5,
        _ => 1.0,
    }
}

/// A fresh shared assistant message answering `model` with the given starting usage.
pub fn shared_output(model: &Model, usage: &Value) -> TestResult<SharedAssistantMessage> {
    let message = serde_json::from_value(json!({
        "role": "assistant", "content": [], "api": model.api, "provider": model.provider,
        "model": model.id, "usage": usage, "stopReason": "stop", "timestamp": 1
    }))?;
    Ok(Arc::new(RwLock::new(message)))
}

/// Every update a finished stream published, without its message handles.
async fn published(stream: &AssistantMessageEventStream) -> TestResult<Vec<Value>> {
    stream.end(None);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let mut value = serde_json::to_value(&event)?;
        if let Some(members) = value.as_object_mut() {
            for handle in ["partial", "message", "error"] {
                members.remove(handle);
            }
        }
        events.push(value);
    }
    Ok(events)
}

/// A source that yields the events and then the failure, if any, instead of ending.
fn finite_source(
    events: Vec<String>,
    failure: Option<DiagnosticErrorInfo>,
) -> impl futures_core::Stream<Item = Result<String, DiagnosticErrorInfo>> + Unpin {
    futures_util::stream::iter(
        events
            .into_iter()
            .map(Ok)
            .chain(failure.map(Err))
            .collect::<Vec<_>>(),
    )
}

/// Run the reducer over a script and collect what it published.
pub async fn reduce(script: Script) -> TestResult<Reduction> {
    let Script {
        model,
        usage,
        events,
        failure,
        options,
    } = script;
    let output = shared_output(&model, &usage)?;
    let stream = AssistantMessageEventStream::new();
    let tiers = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen = Arc::clone(&tiers);
    let pricing = |usage: &mut Usage, tier: Option<&str>| {
        if let Ok(mut log) = seen.lock() {
            log.push(json!(tier));
        }
        let factor = tier_multiplier(tier);
        usage.cost.input *= factor;
        usage.cost.output *= factor;
        usage.cost.cache_read *= factor;
        usage.cost.cache_write *= factor;
        usage.cost.total =
            usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
    };
    let seen = Arc::clone(&tiers);
    let mode = options["resolver"].as_str().map(str::to_owned);
    let resolver = move |response: Option<&str>, request: Option<&str>| {
        if let Ok(mut log) = seen.lock() {
            log.push(json!([response, request]));
        }
        (mode.as_deref() == Some("chosen")).then(|| "chosen".to_owned())
    };
    let wanted = OpenAIResponsesStreamOptions {
        service_tier: options["serviceTier"].as_str(),
        resolve_service_tier: options["resolver"].is_string().then_some(&resolver),
        apply_service_tier_pricing: (options["pricing"] == json!(true)).then_some(&pricing),
    };
    let outcome = ensure_send(process_responses_stream(
        finite_source(events, failure),
        &output,
        &stream,
        &model,
        Some(&wanted),
    ))
    .await;
    let events = published(&stream).await?;
    let message = output.read().map_err(|error| error.to_string())?.clone();
    let tiers = tiers.lock().map_err(|error| error.to_string())?.clone();
    Ok(Reduction {
        outcome,
        message,
        events,
        tiers,
    })
}

/// How the classes of non-finite counts spell, which a JSON value cannot hold.
fn nonfinite(usage: &Usage) -> Value {
    let cost = &usage.cost;
    let counts = [
        ("input", usage.input),
        ("output", usage.output),
        ("cacheRead", usage.cache_read),
        ("cacheWrite", usage.cache_write),
        ("totalTokens", usage.total_tokens),
        ("cost.input", cost.input),
        ("cost.output", cost.output),
        ("cost.cacheRead", cost.cache_read),
        ("cost.cacheWrite", cost.cache_write),
        ("cost.total", cost.total),
    ];
    let spelled = counts
        .into_iter()
        .filter(|(_, count)| !count.is_finite())
        .map(|(key, count)| {
            let text = if count.is_nan() {
                "NaN"
            } else if count > 0.0 {
                "Infinity"
            } else {
                "-Infinity"
            };
            (key.to_owned(), json!(text))
        });
    Value::Object(spelled.collect())
}

/// The message a reduction should leave: the initial one with the row's changes applied.
fn expected_message(run: &Reduction, initial: &Value, mut result: Fields) -> TestResult<Value> {
    let message = &run.message;
    let mut expected = json!({
        "role": "assistant", "content": result.require("content")?, "api": message.api,
        "provider": message.provider, "model": message.model, "usage": initial,
        "stopReason": "stop"
    });
    for key in ["stopReason", "responseId", "usage"] {
        if let Some(value) = result.take(key) {
            expected[key] = value;
        }
    }
    result.finish()?;
    Ok(expected)
}

/// Compare a reduction with the expectation of a row.
pub fn check_reduction(run: &Reduction, initial: &Value, mut expected: Fields) -> TestResult {
    let id = expected.owner().to_owned();
    let result = Fields::new(&id, expected.require("result")?)?;
    let wanted = expected_message(run, initial, result)?;
    let mut actual = serde_json::to_value(&run.message)?;
    if let Some(members) = actual.as_object_mut() {
        members.remove("timestamp");
    }
    assert_eq!(canonical(actual), canonical(wanted), "{id}: message");
    assert_eq!(
        canonical(Value::Array(run.events.clone())),
        canonical(expected.require("events")?),
        "{id}: events"
    );
    assert_eq!(
        Value::Array(run.tiers.clone()),
        expected.take("tiers").unwrap_or_else(|| json!([])),
        "{id}: tiers"
    );
    assert_eq!(
        nonfinite(&run.message.usage),
        expected.take("nonfinite").unwrap_or_else(|| json!({})),
        "{id}: non-finite counts"
    );
    match (&run.outcome, expected.take("error")) {
        (Ok(()), None) => {}
        (Err(failure), Some(Value::String(text))) => assert_eq!(failure.message, text, "{id}"),
        (outcome, error) => {
            return Err(format!("{id}: outcome {outcome:?}, expected error {error:?}").into());
        }
    }
    expected.finish()
}

/// Reduce the events of a row and compare the outcome with the row's expectation.
pub async fn check_events(mut row: Fields) -> TestResult {
    let mut script = Script::of(Vec::new())?;
    script.model = model(row.take("model"))?;
    let initial = row
        .take("initial")
        .map_or_else(zero_usage, |initial| initial["usage"].clone());
    script.usage = initial.clone();
    let mut call_scratch = None;
    script.events = row
        .require("events")?
        .as_array()
        .ok_or("events are a list")?
        .iter()
        .map(|event| {
            let text = match event {
                Value::String(raw) => raw.clone(),
                other => other.to_string(),
            };
            if let Ok(raw) = raw_json(&text) {
                audit_event(raw, "event", call_scratch)?;
                track_call(raw, &mut call_scratch);
            }
            Ok(text)
        })
        .collect::<TestResult<Vec<_>>>()?;
    script.failure = row.take("failure").map(|message| DiagnosticErrorInfo {
        name: Some("Error".to_owned()),
        message: message.as_str().unwrap_or_default().to_owned(),
        stack: Some("source stack".to_owned()),
        code: Some(DiagnosticCode::Text("source-code".to_owned())),
    });
    script.options = row.take("options").unwrap_or_else(|| json!({}));
    let failure = script.failure.clone();
    let run = reduce(script).await?;
    if let (Some(sent), Err(received)) = (&failure, &run.outcome) {
        assert_eq!(
            sent,
            received,
            "{}: the source failure is propagated unchanged",
            row.owner()
        );
    }
    let id = row.owner().to_owned();
    let expected = Fields::new(&id, row.require("expected")?)?;
    check_reduction(&run, &initial, expected)?;
    row.finish()
}

/// Track whether final-call arguments are selected or replaced by nonempty scratch.
fn track_call(raw: &RawValue, scratch: &mut Option<bool>) {
    let kind = raw_kind(raw);
    if kind == "response.output_item.added" {
        if let Some(item) = member(raw, "item") {
            match raw_kind(item).as_str() {
                "function_call" => {
                    *scratch = Some(member(item, "arguments").is_some_and(is_truthy));
                }
                "message" | "reasoning" => *scratch = None,
                _ => {}
            }
        }
    } else if scratch.is_some() && kind == "response.function_call_arguments.delta" {
        if member(raw, "delta").is_none_or(|raw| raw.get() != "\"\"") {
            *scratch = Some(true);
        }
    } else if scratch.is_some() && kind == "response.function_call_arguments.done" {
        *scratch = Some(member(raw, "arguments").is_some_and(|raw| raw.get() != "\"\""));
    } else if kind == "response.output_item.done"
        && member(raw, "item").is_some_and(|item| raw_kind(item) == "function_call")
    {
        *scratch = None;
    }
}

/// Decode only a record's discriminator for fixture consumer selection.
fn raw_kind(raw: &RawValue) -> String {
    member(raw, "type")
        .and_then(|raw| serde_json::from_str(raw.get()).ok())
        .unwrap_or_default()
}

/// Reject ordinary unread members, retaining unrepresentable probes that test skipped reads.
fn audit_event(raw: &RawValue, path: &str, scratch: Option<bool>) -> TestResult {
    if !raw.get().starts_with('{') {
        return Ok(());
    }
    let members: BTreeMap<String, &RawValue> = serde_json::from_str(raw.get())?;
    let kind = raw_kind(raw);
    let allowed = if path == "done" && kind == "function_call" {
        match scratch {
            Some(true) => Some(&["type"][..]),
            Some(false) => Some(&["type", "arguments"][..]),
            None => consumed_fields(path, &kind),
        }
    } else {
        consumed_fields(path, &kind)
    };
    let Some(allowed) = allowed else {
        return Ok(());
    };
    for (key, child) in members {
        if !allowed.contains(&key.as_str()) {
            if serde_json::from_str::<Value>(child.get()).is_err() {
                continue;
            }
            return Err(format!("{path} has unread members [{key}]").into());
        }
        let next = audit_path(path, &kind, &key);
        if child.get().starts_with('[') {
            for item in serde_json::from_str::<Vec<&RawValue>>(child.get())? {
                audit_event(item, &next, scratch)?;
            }
        } else {
            audit_event(child, &next, scratch)?;
        }
    }
    Ok(())
}

/// Fields that each selected branch consumes; dynamic data is compared whole.
fn consumed_fields<'a>(path: &str, kind: &str) -> Option<&'a [&'a str]> {
    Some(match path {
        "event" => match kind {
            "response.created"
            | "response.completed"
            | "response.incomplete"
            | "response.failed" => &["type", "response"],
            "response.output_item.added" | "response.output_item.done" => &["type", "item"],
            "response.content_part.added" | "response.reasoning_summary_part.added" => {
                &["type", "part"]
            }
            "response.function_call_arguments.done" => &["type", "arguments"],
            "error" => &["type", "code", "message"],
            _ if kind.rsplit('.').next() == Some("delta") => &["type", "delta"],
            _ => &["type"],
        },
        "created" => &["id"],
        "completed" => &["id", "status", "usage", "service_tier"],
        "failed" => &["error", "incomplete_details"],
        "error" => &["code", "message"],
        "incomplete_details" => &["reason"],
        "usage" => &[
            "input_tokens",
            "output_tokens",
            "total_tokens",
            "input_tokens_details",
        ],
        "input_tokens_details" => &["cached_tokens"],
        "added" => match kind {
            "reasoning" => &["type", "summary"],
            "message" => &["type", "content"],
            "function_call" => &["type", "id", "call_id", "name", "arguments"],
            _ => &["type"],
        },
        "done" => match kind {
            "reasoning" => return None, // Every member is compared in the persisted signature.
            "message" => &["type", "id", "phase", "content"],
            "function_call" => &["type", "id", "call_id", "name", "arguments"],
            _ => &["type"],
        },
        "part" | "added-content" => &["type"],
        "summary-part" | "added-summary" => &[],
        "content" if kind == "output_text" => &["type", "text"],
        "content" => &["type", "refusal"],
        _ => return None, // Dynamic arguments and interpolation values are compared whole.
    })
}

/// The consumer of a nested fixture record selected by its parent's branch.
fn audit_path(path: &str, kind: &str, key: &str) -> String {
    match (path, key) {
        ("event", "response") => kind
            .strip_prefix("response.")
            .unwrap_or_default()
            .to_owned(),
        ("event", "item") => if kind == "response.output_item.added" {
            "added"
        } else {
            "done"
        }
        .to_owned(),
        ("event", "part") if kind == "response.reasoning_summary_part.added" => {
            "summary-part".to_owned()
        }
        ("added", "content") => "added-content".to_owned(),
        ("added", "summary") => "added-summary".to_owned(),
        _ => key.to_owned(),
    }
}
