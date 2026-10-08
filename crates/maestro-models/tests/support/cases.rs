//! Stream cases: calls described by fixture rows, run against a controlled transport.

use std::sync::{Arc, Mutex};

use maestro_models::{
    AssistantMessageEventStream, Cancellation, DiagnosticErrorInfo, DiagnosticInput, Fetch,
    FetchError, HttpResponse, OnPayload, OnResponse, OpenAICompletionsOptions, SimpleStreamOptions,
    StreamOptions, extract_diagnostic_error, stream_openai_completions,
    stream_simple_openai_completions,
};
use serde_json::{Value, json};

use crate::chat::{TestResult, context, model, rows};
use crate::child_process::{child_case, rerun};

/// Rewrite integral floats as integers so values compare as their JSON text would.
pub fn canonical(value: Value) -> Value {
    match value {
        Value::Number(number) => number
            .as_f64()
            .filter(|float| number.is_f64() && float.fract() == 0.0 && float.abs() < 9.0e15)
            .and_then(|float| format!("{float}").parse::<serde_json::Number>().ok())
            .map_or(Value::Number(number), Value::Number),
        Value::Array(items) => Value::Array(items.into_iter().map(canonical).collect()),
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .map(|(k, v)| (k, canonical(v)))
                .collect(),
        ),
        other => other,
    }
}

/// What a case observed.
pub struct Observed {
    /// Failure that stopped the call before a stream existed.
    pub error: Option<String>,
    /// Requests the controlled transport received.
    pub requests: Vec<Value>,
    /// Hook, transport and event labels in the order they happened.
    pub order: Vec<String>,
    /// Updates without their message handles.
    pub events: Vec<Value>,
    /// Final message without its timestamp.
    pub result: Value,
}

impl Observed {
    /// The observation in the shape fixtures record.
    pub fn to_value(&self) -> Value {
        if let Some(error) = &self.error {
            return json!({"error": error, "requests": [], "order": []});
        }
        canonical(json!({
            "requests": self.requests, "order": self.order,
            "events": self.events, "result": self.result
        }))
    }
}

/// Requests received and the order of hooks, transport calls and updates.
#[derive(Clone, Default)]
struct Log {
    order: Arc<Mutex<Vec<String>>>,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl Log {
    /// Record that a step happened.
    fn note(&self, label: &str) {
        if let Ok(mut order) = self.order.lock() {
            order.push(label.to_owned());
        }
    }

    /// Record a request the transport received.
    fn request(&self, request: Value) {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request);
        }
    }

    /// Snapshot of the order.
    fn order(&self) -> Vec<String> {
        self.order
            .lock()
            .map(|order| order.clone())
            .unwrap_or_default()
    }

    /// Snapshot of the requests.
    fn requests(&self) -> Vec<Value> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }
}

/// The conversation fixtures use when a case names none.
fn default_context() -> Value {
    json!({"messages": [{"role": "user", "content": "hello"}]})
}

/// Build the response body bytes a case scripts.
fn response_body(case: &Value) -> Vec<u8> {
    if let Some(error) = case.get("error") {
        return error.to_string().into_bytes();
    }
    if let Some(raw) = case["raw"].as_str() {
        return raw.as_bytes().to_vec();
    }
    let mut text = String::new();
    for chunk in case["chunks"].as_array().into_iter().flatten() {
        text.push_str("data: ");
        text.push_str(&chunk.to_string());
        text.push_str("\n\n");
    }
    if case["noDone"] != json!(true) {
        text.push_str("data: [DONE]\n\n");
    }
    text.into_bytes()
}

/// A transport that records requests and answers with the case's scripted body.
fn controlled_fetch(case: &Value, log: &Log) -> TestResult<Fetch> {
    let body = response_body(case);
    let status = u16::try_from(case["status"].as_u64().unwrap_or(200))?;
    let fragment = case["fragment"] == json!(true);
    let log = log.clone();
    Ok(Arc::new(move |request| {
        log.note("send");
        log.request(json!({
            "url": request.url,
            "headers": request.headers,
            "body": serde_json::from_slice::<Value>(&request.body).unwrap_or(Value::Null)
        }));
        let chunks: Vec<Result<Vec<u8>, FetchError>> = if fragment {
            body.iter().map(|byte| Ok(vec![*byte])).collect()
        } else {
            vec![Ok(body.clone())]
        };
        Box::pin(std::future::ready(Ok(HttpResponse {
            status,
            headers: [("content-type".to_owned(), "text/event-stream".to_owned())].into(),
            body: Box::pin(futures_util::stream::iter(chunks)),
        })))
    }))
}

/// The failure a hook reports for a scripted message.
fn hook_failure(message: &str) -> DiagnosticErrorInfo {
    extract_diagnostic_error(DiagnosticInput::Text(message))
}

/// A payload hook that logs, may fail and may replace the payload.
fn payload_hook(case: &Value, log: &Log) -> OnPayload {
    let (log, replacement) = (log.clone(), case.get("replace").cloned());
    let failure = case["payloadError"].as_str().map(str::to_owned);
    Arc::new(move |payload, _| {
        log.note("payload");
        let outcome = match &failure {
            Some(message) => Err(hook_failure(message)),
            None => Ok(replacement.clone().unwrap_or(payload)),
        };
        Box::pin(std::future::ready(outcome))
    })
}

/// A response hook that logs and may fail.
fn response_hook(case: &Value, log: &Log) -> OnResponse {
    let log = log.clone();
    let failure = case["responseError"].as_str().map(str::to_owned);
    Arc::new(move |_, _| {
        log.note("response");
        let outcome = failure
            .as_deref()
            .map_or(Ok(()), |message| Err(hook_failure(message)));
        Box::pin(std::future::ready(outcome))
    })
}

/// Decode an optional option the case names.
fn decode<T: serde::de::DeserializeOwned>(spec: &Value, key: &str) -> TestResult<Option<T>> {
    Ok(spec
        .get(key)
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()?)
}

/// Decode the options a case names into common stream options with the hooks installed.
fn common_options(case: &Value, log: &Log) -> TestResult<StreamOptions> {
    let spec = &case["options"];
    let default_key = (case["noApiKey"] != json!(true)).then(|| "fixture-key".to_owned());
    let signal = Cancellation::new();
    if case["abort"] == json!(true) {
        signal.abort();
    }
    Ok(StreamOptions {
        api_key: spec["apiKey"].as_str().map(str::to_owned).or(default_key),
        max_retries: Some(0.0),
        session_id: spec["sessionId"].as_str().map(str::to_owned),
        max_tokens: spec["maxTokens"].as_f64(),
        temperature: spec["temperature"].as_f64(),
        cache_retention: decode(spec, "cacheRetention")?,
        headers: decode(spec, "headers")?,
        signal: Some(signal),
        fetch: (case["noFetch"] != json!(true))
            .then(|| controlled_fetch(case, log))
            .transpose()?,
        on_payload: Some(payload_hook(case, log)),
        on_response: Some(response_hook(case, log)),
        ..StreamOptions::default()
    })
}

/// Start the call a case describes, or report the failure that prevented a stream.
fn start(
    case: &Value,
    common: StreamOptions,
) -> TestResult<Result<AssistantMessageEventStream, String>> {
    let spec = &case["options"];
    let target = model(&case["model"])?;
    let history = context(case.get("context").unwrap_or(&default_context()))?;
    let tool_choice = decode(spec, "toolChoice")?;
    if case["simple"] == json!(true) {
        let simple = SimpleStreamOptions {
            common,
            reasoning: decode(spec, "reasoning")?,
            tool_choice,
            ..SimpleStreamOptions::default()
        };
        return Ok(
            stream_simple_openai_completions(target, history, Some(simple))
                .map_err(|error| error.message),
        );
    }
    let raw = OpenAICompletionsOptions {
        common,
        tool_choice,
        reasoning_effort: decode(spec, "reasoningEffort")?,
    };
    Ok(Ok(stream_openai_completions(target, history, Some(raw))))
}

/// Drain a stream into its labelled events and its result message.
async fn collect(
    stream: &AssistantMessageEventStream,
    log: &Log,
) -> TestResult<(Vec<Value>, Value)> {
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let mut value = serde_json::to_value(&event)?;
        if let Some(object) = value.as_object_mut() {
            log.note(object["type"].as_str().unwrap_or_default());
            for handle in ["partial", "message", "error"] {
                object.remove(handle);
            }
        }
        events.push(value);
    }
    let message = stream.result().await;
    let mut result = serde_json::to_value(&*message.read().map_err(|e| e.to_string())?)?;
    if let Some(object) = result.as_object_mut() {
        object.remove("timestamp");
    }
    Ok((events, result))
}

/// Run one stream case against a controlled transport and record what happened.
pub async fn run_case(case: &Value) -> TestResult<Observed> {
    let log = Log::default();
    let common = common_options(case, &log)?;
    let stream = match start(case, common)? {
        Ok(stream) => stream,
        Err(error) => {
            return Ok(Observed {
                error: Some(error),
                requests: Vec::new(),
                order: Vec::new(),
                events: Vec::new(),
                result: Value::Null,
            });
        }
    };
    let (events, result) = collect(&stream, &log).await?;
    Ok(Observed {
        error: None,
        requests: log.requests(),
        order: log.order(),
        events,
        result,
    })
}

/// Compare a case's observation with the row's expectation.
pub fn assert_observed(row: &Value, observed: &Observed) {
    let id = &row["id"];
    let mut actual = observed.to_value();
    let expected = canonical(row["expected"].clone());
    if row["check"] == json!("usage") {
        assert_eq!(actual["result"]["usage"], expected, "{id}");
        return;
    }
    if row["nativeMessage"] == json!(true) {
        let message = actual["result"]
            .as_object_mut()
            .and_then(|result| result.remove("errorMessage"));
        assert!(
            message.is_some_and(|text| text.as_str().is_some_and(|text| !text.is_empty())),
            "{id} reports a native cause"
        );
    }
    assert_eq!(actual, expected, "{id}");
}

/// The environment variables a row's case names.
fn environment(row: &Value) -> Vec<(&str, &str)> {
    row["case"]["env"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(name, value)| Some((name.as_str(), value.as_str()?)))
        .collect()
}

/// Run every fixture row a test owns in child processes with controlled environments.
///
/// Rows without an `env` object run together in one child with an empty environment;
/// each row with one runs alone in a child holding exactly those variables.
pub async fn assert_rows(fixture: &str, test: &str) -> TestResult {
    let owned = rows(fixture, test)?;
    assert!(!owned.is_empty(), "{test} has fixture rows");
    let Some(selected) = child_case() else {
        rerun(test, "*", &[])?;
        for row in owned.iter().filter(|row| row["case"].get("env").is_some()) {
            rerun(test, row["id"].as_str().ok_or("row id")?, &environment(row))?;
        }
        return Ok(());
    };
    for row in owned.iter().filter(|row| {
        let has_env = row["case"].get("env").is_some();
        (selected == "*" && !has_env) || row["id"] == selected.as_str()
    }) {
        assert_observed(row, &run_case(&row["case"]).await?);
    }
    Ok(())
}

/// Containers the deepest path of accepted JSON text may cross; the parser's recursion limit of
/// 128 rejects the 128th.
pub const DEEPEST_NESTING: usize = 127;

/// `containers` arrays nested around `null`.
pub fn nested_json(containers: usize) -> String {
    format!("{}null{}", "[".repeat(containers), "]".repeat(containers))
}
