#![cfg(test)]

//! Controlled option calls and exact request/stream observations.

use crate::{
    chat::TestResult,
    child_process::{child_case, rerun},
    messages,
};
use maestro_models::providers::messages::anthropic::AnthropicEffort;
use maestro_models::{
    AnthropicOptions, AnthropicThinkingDisplay, HttpResponse, SimpleStreamOptions, StreamOptions,
    ThinkingBudgets, stream_anthropic, stream_simple_anthropic,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

/// One uniquely owned controlled call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Diagnostic identity.
    id: u32,
    /// Owning test.
    test: String,
    /// Whether this uses the simple entry.
    simple: bool,
    /// Invocation inputs.
    input: Input,
    /// Independently recorded result.
    expected: Expected,
}

/// Exact payload or closed invocation expectation.
#[derive(Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "lowercase",
    deny_unknown_fields
)]
enum Expected {
    /// Whole payload at the public callback boundary.
    Payload(Value),
    /// All observations after producer closure.
    Entry(Entry),
}

/// Closed invocation observations; unrecognized fields are fixture errors.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    /// Callback, send and event ordering.
    trace: Vec<String>,
    /// Requests through the selected public seam.
    requests: Vec<Value>,
    /// Callback payloads before replacement.
    payloads: Vec<Value>,
    /// Callback response records.
    responses: Vec<Value>,
    /// Announced updates.
    events: Vec<Value>,
    /// Stream result, absent only for an immediate error.
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    /// Immediate simple-entry failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    thrown: Option<Value>,
}

/// Invocation inputs and scripted callback/body behavior.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Model overrides.
    model: Value,
    /// Conversation.
    context: Value,
    /// Options.
    options: Value,
    /// Isolated environment.
    #[serde(default)]
    env: BTreeMap<String, String>,
    /// Use an injected client.
    #[serde(default)]
    injected: bool,
    /// Incoming tool names.
    #[serde(default)]
    names: Vec<String>,
    /// Replacement payload.
    replacement: Option<Value>,
    /// Failing callback phase.
    fail: Option<String>,
    /// Caller error message.
    error: Option<String>,
    /// Complete alternate answer.
    answer: Option<String>,
}

/// Observations collected only after the invocation closes.
#[derive(Default)]
struct Seen {
    /// Callback/transport/event order.
    trace: Vec<String>,
    /// Sent requests.
    requests: Vec<Value>,
    /// Payload-hook inputs.
    payloads: Vec<Value>,
    /// Response-hook inputs.
    responses: Vec<Value>,
}

/// Read tagged numeric options as native doubles.
pub fn number(value: &Value) -> Option<f64> {
    match value["$number"].as_str() {
        Some("NaN") => Some(f64::NAN),
        Some("Infinity") => Some(f64::INFINITY),
        Some("-Infinity") => Some(f64::NEG_INFINITY),
        Some("-0") => Some(-0.0),
        _ => value.as_f64(),
    }
}

/// Decode one nonnumeric option.
fn decode<T: serde::de::DeserializeOwned>(spec: &Value, name: &str) -> TestResult<Option<T>> {
    Ok(spec
        .get(name)
        .filter(|v| **v != json!({"$number":"undefined"}))
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?)
}

/// Reject options that no decoder consumes.
fn validate_options(spec: &Value) -> TestResult {
    for name in spec.as_object().ok_or("options must be an object")?.keys() {
        assert!(
            matches!(
                name.as_str(),
                "apiKey"
                    | "temperature"
                    | "maxTokens"
                    | "maxRetries"
                    | "timeoutMs"
                    | "cacheRetention"
                    | "headers"
                    | "metadata"
                    | "sessionId"
                    | "thinkingEnabled"
                    | "thinkingBudgetTokens"
                    | "effort"
                    | "thinkingDisplay"
                    | "interleavedThinking"
                    | "toolChoice"
                    | "reasoning"
                    | "thinkingBudgets"
            ),
            "unread option {name}"
        );
    }
    Ok(())
}

/// Resolve the fixture's common options without changing numeric inputs.
fn options(input: &Input) -> TestResult<AnthropicOptions> {
    let spec = &input.options;
    validate_options(spec)?;
    Ok(AnthropicOptions {
        common: StreamOptions {
            api_key: spec["apiKey"].as_str().map(str::to_owned),
            temperature: number(&spec["temperature"]),
            max_tokens: number(&spec["maxTokens"]),
            max_retries: number(&spec["maxRetries"]).or(Some(0.0)),
            timeout_ms: number(&spec["timeoutMs"]),
            cache_retention: decode(spec, "cacheRetention")?,
            headers: decode(spec, "headers")?,
            metadata: decode(spec, "metadata")?,
            session_id: decode(spec, "sessionId")?,
            ..StreamOptions::default()
        },
        thinking_enabled: spec["thinkingEnabled"].as_bool(),
        thinking_budget_tokens: number(&spec["thinkingBudgetTokens"]),
        effort: match spec["effort"].as_str() {
            None => None,
            Some("low") => Some(AnthropicEffort::Low),
            Some("medium") => Some(AnthropicEffort::Medium),
            Some("high") => Some(AnthropicEffort::High),
            Some("xhigh") => Some(AnthropicEffort::Xhigh),
            Some("max") => Some(AnthropicEffort::Max),
            Some(other) => return Err(format!("unknown effort {other}").into()),
        },
        thinking_display: match spec["thinkingDisplay"].as_str() {
            None => None,
            Some("summarized") => Some(AnthropicThinkingDisplay::Summarized),
            Some("omitted") => Some(AnthropicThinkingDisplay::Omitted),
            Some(other) => return Err(format!("unknown display {other}").into()),
        },
        interleaved_thinking: spec["interleavedThinking"].as_bool(),
        tool_choice: match spec.get("toolChoice") {
            Some(Value::Object(choice)) => {
                assert_eq!(choice["type"], "tool");
                Some(maestro_models::ToolChoice::Function {
                    name: choice["name"].as_str().ok_or("missing tool name")?.into(),
                })
            }
            Some(Value::String(mode)) if mode == "any" => {
                Some(maestro_models::ToolChoice::Required)
            }
            _ => decode(spec, "toolChoice")?,
        },
        ..AnthropicOptions::default()
    })
}

/// A closed answer with discriminating argument updates.
fn answer(input: &Input) -> String {
    if let Some(answer) = &input.answer {
        return answer.clone();
    }
    let mut events = vec![
        json!({"type":"message_start","message":{"id":"controlled","usage":{"input_tokens":2,"output_tokens":0}}}),
    ];
    for (index, name) in input.names.iter().enumerate() {
        events.extend([
            json!({"type":"content_block_start","index":index,"content_block":{"type":"tool_use","id":format!("tool{index}"),"name":name,"input":{"q":"retained"}}}),
            json!({"type":"content_block_delta","index":index,"delta":{"type":"input_json_delta","partial_json":"{\"q\":\"different\"}"}}),
            json!({"type":"content_block_stop","index":index}),
        ]);
    }
    events.extend([
        json!({"type":"message_delta","delta":{"stop_reason":if input.names.is_empty(){"end_turn"}else{"tool_use"}},"usage":{"output_tokens":1}}),
        json!({"type":"message_stop"}),
    ]);
    events.iter().map(messages::sse).collect()
}

/// Attach hooks that record their inputs and can end their own phase.
fn hooks(options: &mut AnthropicOptions, row: &Row, seen: &Arc<Mutex<Seen>>) {
    let capture = Arc::clone(seen);
    let replacement = row.input.replacement.clone();
    let failure = (row.input.fail.as_deref() == Some("payload"))
        .then(|| row.input.error.clone().unwrap_or_default());
    options.common.on_payload = Some(Arc::new(move |payload, _| {
        let observed = payload.clone();
        {
            let mut seen = capture.lock().unwrap();
            seen.trace.push("payload".into());
            seen.payloads.push(observed);
        }
        let result = failure.as_ref().map_or_else(
            || Ok(replacement.clone().unwrap_or(payload)),
            |error| Err(messages::diagnostic(error)),
        );
        Box::pin(std::future::ready(result))
    }));
    let capture = Arc::clone(seen);
    let failure = (row.input.fail.as_deref() == Some("response"))
        .then(|| row.input.error.clone().unwrap_or_default());
    options.common.on_response = Some(Arc::new(move |response, _| {
        let observed = serde_json::to_value(response).unwrap();
        {
            let mut seen = capture.lock().unwrap();
            seen.trace.push("response".into());
            seen.responses.push(observed);
        }
        let result = failure
            .as_ref()
            .map_or(Ok(()), |error| Err(messages::diagnostic(error)));
        Box::pin(std::future::ready(result))
    }));
}

/// A complete controlled response.
fn response(wire: &str, status: u16) -> HttpResponse {
    HttpResponse {
        status,
        headers: [
            ("content-type".into(), "text/plain;charset=UTF-8".into()),
            ("x-controlled".into(), "yes".into()),
        ]
        .into(),
        body: Box::pin(futures_util::stream::iter([Ok(wire.as_bytes().to_vec())])),
    }
}

/// Attach the public transport or injected-client seam.
fn transport(options: &mut AnthropicOptions, input: &Input, seen: &Arc<Mutex<Seen>>) {
    let wire = answer(input);
    let capture = Arc::clone(seen);
    if input.injected {
        options.client = Some(Arc::new(move |payload, settings| {
            let mut settings_value = serde_json::Map::new();
            if let Some(timeout) = settings.timeout_ms {
                settings_value.insert("timeout".into(), json!(timeout));
            }
            if let Some(retries) = settings.max_retries {
                settings_value.insert("maxRetries".into(), json!(retries));
            }
            let request = json!({"body":payload,"options":settings_value});
            {
                let mut seen = capture.lock().unwrap();
                seen.trace.push("client".into());
                seen.requests.push(request);
            }
            Box::pin(std::future::ready(Ok(response(&wire, 201))))
        }));
    } else {
        options.common.fetch = Some(Arc::new(move |request| {
            let request = json!({"url":request.url,"headers":request.headers,"body":serde_json::from_slice::<Value>(&request.body).unwrap()});
            {
                let mut seen = capture.lock().unwrap();
                seen.trace.push("fetch".into());
                seen.requests.push(request);
            }
            Box::pin(std::future::ready(Ok(response(&wire, 200))))
        }));
    }
}

/// Remove missing-value tags only at the typed conversation boundary.
fn without_undefined(value: Value) -> Value {
    match value {
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .filter(|(_, v)| *v != json!({"$number":"undefined"}))
                .map(|(k, v)| (k, without_undefined(v)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(without_undefined).collect()),
        other => other,
    }
}

/// Start either exported entry with the fixture's typed options.
fn start_call(
    row: &Row,
    options: AnthropicOptions,
    model: maestro_models::Model,
    context: maestro_models::Context,
) -> TestResult<
    Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>,
> {
    if row.simple {
        let budgets = &row.input.options["thinkingBudgets"];
        let simple = SimpleStreamOptions {
            common: options.common,
            reasoning: decode(&row.input.options, "reasoning")?,
            thinking_budgets: (!budgets.is_null()).then(|| ThinkingBudgets {
                minimal: number(&budgets["minimal"]),
                low: number(&budgets["low"]),
                medium: number(&budgets["medium"]),
                high: number(&budgets["high"]),
            }),
            tool_choice: options.tool_choice,
        };
        Ok(stream_simple_anthropic(model, context, Some(simple)))
    } else {
        Ok(Ok(stream_anthropic(model, context, Some(options))))
    }
}

/// Read all events and the final message after producer settlement.
async fn observe(
    stream: &maestro_models::AssistantMessageEventStream,
    seen: &Arc<Mutex<Seen>>,
) -> TestResult<(Vec<Value>, Value)> {
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let value = serde_json::to_value(&event)?;
        let mut observed = serde_json::Map::new();
        for name in ["type", "contentIndex", "toolCall", "delta"] {
            if let Some(v) = value.get(name) {
                observed.insert(name.into(), v.clone());
            }
        }
        let kind = value["type"].as_str().unwrap().to_owned();
        seen.lock().unwrap().trace.push(kind);
        events.push(Value::Object(observed));
    }
    let result = stream.result().await;
    let snapshot = result.read().unwrap().clone();
    let mut result = serde_json::to_value(snapshot)?;
    result.as_object_mut().unwrap().remove("timestamp");
    Ok((events, result))
}

/// Object equality does not check the protocol's top-level field order.
fn assert_payload_order(actual: &Value, expected: &Value) {
    let actual: Vec<_> = actual.as_object().unwrap().keys().collect();
    let expected: Vec<_> = expected.as_object().unwrap().keys().collect();
    assert_eq!(actual, expected, "payload field order");
}

/// Apply overrides to the zero-cost controlled model.
fn controlled_model(overrides: &Value) -> TestResult<maestro_models::Model> {
    let mut patch = json!({"provider":"controlled","contextWindow":200_000,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0}});
    patch
        .as_object_mut()
        .unwrap()
        .extend(overrides.as_object().unwrap().clone());
    messages::model(&patch)
}

/// Run one public call and compare its closed observations.
async fn run(row: &Row) -> TestResult {
    let model = controlled_model(&row.input.model)?;
    let context = messages::context(&without_undefined(row.input.context.clone()))?;
    let seen = Arc::new(Mutex::new(Seen::default()));
    let mut options = options(&row.input)?;
    hooks(&mut options, row, &seen);
    transport(&mut options, &row.input, &seen);
    let stream = match start_call(row, options, model, context)? {
        Ok(stream) => stream,
        Err(error) => {
            let observed = std::mem::take(&mut *seen.lock().unwrap());
            let actual = json!({"trace":observed.trace,"requests":observed.requests,"payloads":observed.payloads,"responses":observed.responses,"events":[],"thrown":{"name":error.name,"message":error.message}});
            assert_eq!(
                crate::json::canonical(actual),
                crate::json::canonical(match &row.expected {
                    Expected::Entry(expected) => serde_json::to_value(expected)?,
                    Expected::Payload(payload) => payload.clone(),
                }),
                "row {}",
                row.id
            );
            return Ok(());
        }
    };
    let (events, result) = observe(&stream, &seen).await?;
    let seen = std::mem::take(&mut *seen.lock().unwrap());
    assert_run(row, &seen, &events, result)
}

/// Compare all closed observations, including payload member order.
fn assert_run(row: &Row, seen: &Seen, events: &[Value], mut result: Value) -> TestResult {
    match &row.expected {
        Expected::Entry(expected) => {
            for (actual, expected) in seen.payloads.iter().zip(&expected.payloads) {
                assert_payload_order(actual, expected);
            }
            if expected.result.as_ref().is_some_and(|result| {
                result["stopReason"] == "error" && result.get("errorMessage").is_none()
            }) {
                assert!(
                    result["errorMessage"]
                        .as_str()
                        .is_some_and(|message| !message.is_empty()),
                    "native cause required"
                );
                result.as_object_mut().unwrap().remove("errorMessage");
            }
            let actual = json!({"trace":seen.trace,"requests":seen.requests,"payloads":seen.payloads,"responses":seen.responses,"events":events,"result":result});
            assert_eq!(
                crate::json::canonical(actual),
                crate::json::canonical(serde_json::to_value(expected)?),
                "row {}",
                row.id
            );
        }
        Expected::Payload(payload) => {
            for actual in &seen.payloads {
                assert_payload_order(actual, payload);
            }
            assert_eq!(
                crate::json::canonical(json!(seen.payloads)),
                crate::json::canonical(json!([payload])),
                "row {}",
                row.id
            );
        }
    }

    Ok(())
}

/// Identify the typed invocation before attaching its controlled callbacks.
fn normalized_query(row: &Row) -> TestResult<String> {
    let options = options(&row.input)?;
    let common = &options.common;
    let mut selected = json!({
        "apiKey": common.api_key,
        "temperature": common.temperature.map(f64::to_bits),
        "maxTokens": common.max_tokens.map(f64::to_bits),
        "maxRetries": common.max_retries.map(f64::to_bits),
        "timeoutMs": common.timeout_ms.map(f64::to_bits),
        "cacheRetention": common.cache_retention,
        "headers": common.headers,
        "metadata": common.metadata,
        "sessionId": common.session_id,
    });
    if row.simple {
        selected["reasoning"] = serde_json::to_value(decode::<maestro_models::ThinkingLevel>(
            &row.input.options,
            "reasoning",
        )?)?;
        let budgets = &row.input.options["thinkingBudgets"];
        selected["thinkingBudgets"] = if budgets.is_null() {
            Value::Null
        } else {
            json!(
                ["minimal", "low", "medium", "high"]
                    .map(|level| number(&budgets[level]).map(f64::to_bits))
            )
        };
        selected["toolChoice"] = serde_json::to_value(options.tool_choice)?;
    } else {
        selected["raw"] = json!({
            "thinkingEnabled": options.thinking_enabled,
            "thinkingBudgetTokens": options.thinking_budget_tokens.map(f64::to_bits),
            "effort": format!("{:?}", options.effort),
            "thinkingDisplay": format!("{:?}", options.thinking_display),
            "interleavedThinking": options.interleaved_thinking,
            "toolChoice": options.tool_choice,
        });
    }
    let query = json!({
        "simple": row.simple,
        "options": selected,
        "model": controlled_model(&row.input.model)?,
        "context": messages::context(&without_undefined(row.input.context.clone()))?,
        "env": row.input.env,
        "replacement": row.input.replacement,
        "injected": row.input.injected,
        "fail": row.input.fail,
        "error": row.input.fail.as_ref().map(|_| row.input.error.as_deref().unwrap_or_default()),
        "answer": answer(&row.input),
    });
    Ok(serde_json::to_string(&query)?)
}

/// Dispatch each unique fixture query to its owning test in an isolated process.
pub async fn assert_rows(test: &str) -> TestResult {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../fixtures/message_protocol/options.json"))?;
    let mut ids = HashSet::new();
    let mut queries = HashSet::new();
    for row in &rows {
        assert!(
            crate::TEST_NAMES.contains(&row.test.as_str()),
            "unknown fixture owner {}",
            row.test
        );
        assert!(ids.insert(row.id));
        assert!(
            queries.insert(normalized_query(row)?),
            "duplicate row {}",
            row.id
        );
    }
    let owned: Vec<_> = rows.iter().filter(|row| row.test == test).collect();
    assert!(!owned.is_empty());
    if let Some(selected) = child_case() {
        for row in owned.iter().filter(|row| {
            (selected == "*" && row.input.env.is_empty()) || selected == row.id.to_string()
        }) {
            run(row).await?;
        }
    } else {
        rerun(test, "*", &[])?;
        for row in owned.into_iter().filter(|row| !row.input.env.is_empty()) {
            let env: Vec<_> = row
                .input
                .env
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            rerun(test, &row.id.to_string(), &env)?;
        }
    }
    Ok(())
}
