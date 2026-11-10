//! Shared recorded response inputs, transport assembly and comparison.
#[cfg(not(target_arch = "wasm32"))]
use crate::chat::{TestResult, context, model, overlay};
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::providers::responses_api::openai::OpenAIResponsesReasoningSummary;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{Cancellation, Fetch, FetchError, HttpRequest, HttpResponse, StreamOptions};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};
#[cfg(not(target_arch = "wasm32"))]
use std::fmt::Write as _;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex, PoisonError};

#[cfg(not(target_arch = "wasm32"))]
/// Model with the recording's rates and limits.
pub fn target(case: &Value, descriptor: Value) -> TestResult<maestro_models::Model> {
    model(&overlay(descriptor, &case["model"]))
}
#[cfg(not(target_arch = "wasm32"))]
/// Conversation defaults overridden by a selected fixture history.
pub fn history(case: &Value) -> TestResult<maestro_models::Context> {
    context(&overlay(
        json!({"systemPrompt":"instruction","messages":[{"role":"user","content":"query"}]}),
        &case["context"],
    ))
}
#[cfg(not(target_arch = "wasm32"))]
/// Read a tagged nonfinite test number.
pub fn number(value: &Value) -> Option<f64> {
    match value["number"].as_str() {
        Some("NaN") => Some(f64::NAN),
        Some("Infinity") => Some(f64::INFINITY),
        _ => value.as_f64(),
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Decode an optional typed fixture option.
pub fn option<T: serde::de::DeserializeOwned>(value: &Value, key: &str) -> TestResult<Option<T>> {
    Ok(value
        .get(key)
        .filter(|v| !v.is_null())
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?)
}
/// Hook failure returned as data.
pub fn failure(text: &str) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::extract_diagnostic_error(maestro_models::DiagnosticInput::Text(text))
}
#[cfg(not(target_arch = "wasm32"))]
/// Return the raw body, else framed events, else `complete`.
pub fn body(case: &Value, complete: &str) -> Vec<u8> {
    if let Some(body) = case["body"].as_str() {
        return body.as_bytes().to_vec();
    }
    let Some(events) = case["events"].as_array() else {
        return complete.as_bytes().to_vec();
    };
    events
        .iter()
        .fold(String::new(), |mut s, e| {
            let _ = write!(s, "data: {e}\n\n");
            s
        })
        .into_bytes()
}
#[cfg(not(target_arch = "wasm32"))]
/// Finite observation logs containing only owned JSON data.
#[derive(Clone, Default)]
pub struct Log {
    /// Captured wire requests.
    pub requests: Arc<Mutex<Vec<Value>>>,
    /// Payload/response hook observations.
    pub hooks: Arc<Mutex<Vec<Value>>>,
}
#[cfg(not(target_arch = "wasm32"))]
/// Controlled transport; body construction never runs under a log lock.
pub fn fetch(
    case: &Value,
    log: &Log,
    signal: Cancellation,
    complete: &str,
    capture: impl Fn(&HttpRequest) -> Value + Send + Sync + 'static,
) -> TestResult<Fetch> {
    let (sent, wire, status, fragmented) = (
        Arc::clone(&log.requests),
        body(case, complete),
        u16::try_from(case["status"].as_u64().unwrap_or(200))?,
        case["chunked"] == true,
    );
    let body_abort = case["cancel"] == "body";
    let body_failure = case["cancel"] == "body_failure";
    let content_type = case["contentType"]
        .as_str()
        .unwrap_or("text/event-stream")
        .to_owned();
    Ok(Arc::new(move |request| {
        let captured = capture(&request);
        sent.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(captured);
        let chunks = if body_failure {
            signal.abort();
            vec![Err(FetchError::Connection(failure("body failed")))]
        } else if body_abort {
            signal.abort();
            vec![Err(FetchError::Aborted)]
        } else if fragmented {
            wire.chunks(3).map(|c| Ok(c.to_vec())).collect()
        } else {
            vec![Ok(wire.clone())]
        };
        Box::pin(std::future::ready(Ok(HttpResponse {
            status,
            status_text: String::new(),
            headers: [
                ("content-type".into(), content_type.clone()),
                ("x-controlled".into(), "yes".into()),
            ]
            .into(),
            body: Box::pin(futures_util::stream::iter(chunks)),
        })))
    }))
}
#[cfg(not(target_arch = "wasm32"))]
/// Common option projection from the reduced recording.
pub fn common(case: &Value) -> TestResult<StreamOptions> {
    let signal = Cancellation::new();
    if case["cancel"] == "before" {
        signal.abort();
    }
    let spec = &case["options"];
    let api_key = if case["missingKey"] == true {
        None
    } else {
        Some(spec["apiKey"].as_str().unwrap_or("test-key").to_owned())
    };
    Ok(StreamOptions {
        api_key,
        max_retries: Some(spec["maxRetries"].as_f64().unwrap_or(0.0)),
        signal: Some(signal),
        max_tokens: number(&spec["maxTokens"]),
        temperature: number(&spec["temperature"]),
        session_id: spec["sessionId"].as_str().map(str::to_owned),
        cache_retention: option(spec, "cacheRetention")?,
        headers: option(spec, "headers")?,
        transport: option(spec, "transport")?,
        metadata: option(spec, "metadata")?,
        max_retry_delay_ms: spec["maxRetryDelayMs"].as_f64(),
        ..Default::default()
    })
}
#[cfg(not(target_arch = "wasm32"))]
/// Decode the recorded summary selection.
pub fn reasoning_summary(value: &Value) -> Option<OpenAIResponsesReasoningSummary> {
    match value.as_str() {
        Some("auto") => Some(OpenAIResponsesReasoningSummary::Auto),
        Some("detailed") => Some(OpenAIResponsesReasoningSummary::Detailed),
        Some("concise") => Some(OpenAIResponsesReasoningSummary::Concise),
        _ => None,
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Capture the common request fields before provider-specific wire observations.
pub fn request(request: &HttpRequest) -> Value {
    json!({"url":request.url,"headers":request.headers,"body":serde_json::from_slice::<Value>(&request.body).unwrap_or(Value::Null)})
}
#[cfg(not(target_arch = "wasm32"))]
/// Preserve the body's authored key order.
pub fn body_keys(captured: &mut Value) {
    captured["bodyKeys"] = json!(
        captured["body"]
            .as_object()
            .map(|body| body.keys().collect::<Vec<_>>())
    );
}
#[cfg(not(target_arch = "wasm32"))]
/// Observe a closed stream and its recorded requests and hooks.
pub async fn observe(
    stream: &maestro_models::AssistantMessageEventStream,
    log: &Log,
) -> TestResult<Value> {
    let (events, result) = crate::response_output::collect(stream).await?;
    let requests = log.requests.lock().map_err(|e| e.to_string())?.clone();
    let hooks = log.hooks.lock().map_err(|e| e.to_string())?.clone();
    Ok(json!({"requests":requests,"hooks":hooks,"events":events,"result":result}))
}
#[cfg(not(target_arch = "wasm32"))]
/// Compare recorded observations, with native parser text checked at its own boundary.
pub fn compare(row: &Value, mut actual: Value, replacement: &Value) {
    if row["case"]["hook"] == "replace" {
        assert_eq!(actual["requests"][0]["body"], *replacement);
    }
    if row["expected"]["result"]["errorMessage"] == json!({"nativeDiagnostic":true}) {
        assert!(
            actual["result"]["errorMessage"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        );
        actual["result"]["errorMessage"] = json!({"nativeDiagnostic":true});
    }
    assert_eq!(
        crate::json::canonical(actual),
        crate::json::canonical(row["expected"].clone()),
        "{}",
        row["case"]
    );
}
