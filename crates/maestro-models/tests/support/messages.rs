//! Fixture rows, a scripted transport and call observation for the message protocol tests.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use maestro_models::{
    AnthropicClient, AnthropicOptions, AssistantMessageEventStream, Context, DiagnosticErrorInfo,
    FetchError, HttpBody, HttpResponse, Model, StreamOptions, stream_anthropic,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::sync::{mpsc, oneshot};

use crate::chat::TestResult;
use crate::child_process::{child_case, rerun};
use crate::json::canonical;

/// Ends a message stream and nothing else.
pub const STOP: &str = "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

/// One fixture row: a call, the process environment it needs and what it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Row name in failure messages.
    pub id: String,
    /// Test that owns the row.
    pub test: String,
    /// The call.
    pub case: Case,
    /// What the call must produce.
    pub expected: Expected,
}

/// A call described by data.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Case {
    /// Fields merged over the fixture model.
    pub model: Value,
    /// The conversation, with the fields fixtures omit left out.
    pub context: Value,
    /// Caller options.
    pub options: Value,
    /// Variables of the only process environment the call may see.
    pub env: Option<BTreeMap<String, String>>,
    /// Status of the scripted response.
    pub status: Option<u16>,
    /// Body of the scripted response, one chunk per entry.
    pub chunks: Option<Vec<Chunk>>,
}

/// One chunk of a scripted response body.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum Chunk {
    /// Text sent as UTF-8.
    Text(String),
    /// Bytes sent as they are.
    Bytes(Vec<u8>),
}

/// What a call must produce; absent fields are not checked.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Expected {
    /// Request URL.
    pub url: Option<String>,
    /// Request headers that must have these values; `null` means absent.
    pub headers: BTreeMap<String, Value>,
    /// Request body.
    pub payload: Option<Value>,
    /// Updates, each reduced to its kind, position, delta and content.
    pub events: Option<Vec<Value>>,
    /// Final message without its timestamp.
    pub result: Option<Value>,
    /// Failure text of the final message.
    pub error: Option<String>,
    /// Number of requests the transport must have received.
    pub requests: Option<usize>,
}

/// The fixture model with `patch` merged over it.
pub fn model(patch: &Value) -> TestResult<Model> {
    let mut value = json!({
        "id": "claude-sonnet-4-5", "name": "Fixture", "api": "anthropic-messages",
        "provider": "fixture", "baseUrl": "https://fixture.invalid", "reasoning": true,
        "input": ["text", "image"], "cost": {"input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4},
        "contextWindow": 100_000, "maxTokens": 32_000
    });
    if let (Some(base), Some(changes)) = (value.as_object_mut(), patch.as_object()) {
        base.extend(changes.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    Ok(serde_json::from_value(value)?)
}

/// Decode a conversation after filling the fields fixtures omit; no conversation asks `ask`.
pub fn context(compact: &Value) -> TestResult<Context> {
    let mut value = if compact.is_null() {
        json!({"messages": [{"role": "user", "content": "ask"}]})
    } else {
        compact.clone()
    };
    let messages = value
        .get_mut("messages")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten();
    for message in messages {
        let defaults = match message["role"].as_str() {
            Some("assistant") => json!({
                "api": "anthropic-messages", "provider": "fixture", "model": "claude-sonnet-4-5",
                "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
                    "totalTokens": 0, "cost": {"input": 0, "output": 0, "cacheRead": 0,
                    "cacheWrite": 0, "total": 0}},
                "stopReason": "stop", "timestamp": 0
            }),
            Some("user") => json!({"timestamp": 0}),
            _ => json!({"isError": false, "timestamp": 0}),
        };
        if let (Some(target), Some(defaults)) = (message.as_object_mut(), defaults.as_object()) {
            for (key, default) in defaults {
                target.entry(key.clone()).or_insert_with(|| default.clone());
            }
        }
    }
    Ok(serde_json::from_value(value)?)
}

/// A scripted response body.
fn body(chunks: &[Chunk]) -> HttpBody {
    let pieces: Vec<Result<Vec<u8>, FetchError>> = chunks
        .iter()
        .map(|chunk| match chunk {
            Chunk::Text(text) => Ok(text.clone().into_bytes()),
            Chunk::Bytes(bytes) => Ok(bytes.clone()),
        })
        .collect();
    Box::pin(futures_util::stream::iter(pieces))
}

/// The requests a scripted transport received.
pub type Requests = Arc<Mutex<Vec<Value>>>;

/// A transport that records each request and answers with the case's scripted response.
pub fn scripted_fetch(case: &Case, requests: &Requests) -> maestro_models::Fetch {
    let chunks = case
        .chunks
        .clone()
        .unwrap_or_else(|| vec![Chunk::Text(STOP.to_owned())]);
    let status = case.status.unwrap_or(200);
    let requests = Arc::clone(requests);
    Arc::new(move |request| {
        if let Ok(mut seen) = requests.lock() {
            seen.push(json!({
                "url": request.url,
                "headers": request.headers,
                "body": serde_json::from_slice::<Value>(&request.body).unwrap_or(Value::Null),
                "text": String::from_utf8_lossy(&request.body),
            }));
        }
        Box::pin(std::future::ready(Ok(HttpResponse {
            status,
            headers: [("content-type".to_owned(), "text/event-stream".to_owned())].into(),
            body: body(&chunks),
        })))
    })
}

/// Decode an optional option the case names.
fn decode<T: serde::de::DeserializeOwned>(spec: &Value, key: &str) -> TestResult<Option<T>> {
    Ok(spec
        .get(key)
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()?)
}

/// Options a case names, over the defaults every case shares: a fixed key and no retries.
pub fn options(case: &Case) -> TestResult<AnthropicOptions> {
    let spec = &case.options;
    let api_key = match spec.get("apiKey") {
        Some(key) => key.as_str().map(str::to_owned),
        None => Some("fixture-key".to_owned()),
    };
    Ok(AnthropicOptions {
        common: StreamOptions {
            api_key,
            max_tokens: spec["maxTokens"].as_f64(),
            temperature: spec["temperature"].as_f64(),
            cache_retention: decode(spec, "cacheRetention")?,
            headers: decode(spec, "headers")?,
            metadata: decode(spec, "metadata")?,
            max_retries: Some(0.0),
            ..StreamOptions::default()
        },
        interleaved_thinking: spec["interleavedThinking"].as_bool(),
        tool_choice: decode(spec, "toolChoice")?,
        client: None,
    })
}

/// What a call produced.
pub struct Run {
    /// Requests the transport received.
    pub requests: Vec<Value>,
    /// Updates reduced to kind, position, delta and content.
    pub events: Vec<Value>,
    /// Final message without its timestamp.
    pub result: Value,
}

/// Drain a stream into its reduced updates and its final message.
pub async fn collect(stream: &AssistantMessageEventStream) -> TestResult<(Vec<Value>, Value)> {
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let value = serde_json::to_value(&event)?;
        let mut reduced = Map::new();
        for key in ["type", "contentIndex", "delta", "content"] {
            if let Some(field) = value.get(key) {
                reduced.insert(key.to_owned(), field.clone());
            }
        }
        events.push(Value::Object(reduced));
    }
    let message = stream.result().await;
    let mut result = serde_json::to_value(&*message.read().map_err(|error| error.to_string())?)?;
    if let Some(members) = result.as_object_mut() {
        members.remove("timestamp");
    }
    Ok((events, result))
}

/// Run the call a case describes against its scripted transport.
pub async fn run_case(case: &Case) -> TestResult<Run> {
    let requests = Requests::default();
    let mut options = options(case)?;
    options.common.fetch = Some(scripted_fetch(case, &requests));
    let stream = stream_anthropic(model(&case.model)?, context(&case.context)?, Some(options));
    let (events, result) = collect(&stream).await?;
    let requests = requests.lock().map_err(|error| error.to_string())?.clone();
    Ok(Run {
        requests,
        events,
        result,
    })
}

/// One server-sent event as the service writes it: its type names the event.
pub fn sse(data: &Value) -> String {
    let kind = data["type"].as_str().unwrap_or_default();
    format!("event: {kind}\ndata: {data}\n\n")
}

/// A response body the test feeds chunk by chunk: the body ends when the sender is dropped.
pub struct Feed {
    /// Sends the next chunk of the body.
    pub chunks: mpsc::UnboundedSender<Vec<u8>>,
    /// Resolves when the reader drops the body.
    pub released: oneshot::Receiver<()>,
}

/// Tells the test that the reader dropped the body.
struct Release(Option<oneshot::Sender<()>>);

impl Drop for Release {
    fn drop(&mut self) {
        if let Some(released) = self.0.take() {
            released.send(()).ok();
        }
    }
}

/// A body fed by the test, with the handle that feeds it.
pub fn feed() -> (Feed, HttpBody) {
    let (chunks, receiver) = mpsc::unbounded_channel();
    let (release, released) = oneshot::channel();
    let body = futures_util::stream::unfold(
        (receiver, Release(Some(release))),
        |(mut receiver, release)| async move {
            let chunk = receiver.recv().await?;
            Some((Ok(chunk), (receiver, release)))
        },
    );
    (Feed { chunks, released }, Box::pin(body))
}

/// A client that answers once with a successful response holding `body`.
pub fn client_for(body: HttpBody) -> AnthropicClient {
    let body = Mutex::new(Some(body));
    Arc::new(move |_, _| {
        let taken = body.lock().ok().and_then(|mut body| body.take());
        Box::pin(std::future::ready(match taken {
            Some(body) => Ok(HttpResponse {
                status: 200,
                headers: BTreeMap::new(),
                body,
            }),
            None => Err(diagnostic("the body was already taken")),
        }))
    })
}

/// A failure that carries `message` as an error of the given name.
pub fn diagnostic(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("TypeError".to_owned()),
        message: message.to_owned(),
        stack: None,
        code: None,
    }
}

/// A client that fails with the given message.
pub fn failing_client(message: &str) -> AnthropicClient {
    let error = diagnostic(message);
    Arc::new(move |_, _| Box::pin(std::future::ready(Err(error.clone()))))
}

/// Check the request a run sent against what its row expects.
fn assert_request(row: &Row, run: &Run) {
    let (id, expected) = (&row.id, &row.expected);
    if let Some(count) = expected.requests {
        assert_eq!(run.requests.len(), count, "{id} requests");
    }
    let first = run.requests.first();
    if let Some(url) = &expected.url {
        assert_eq!(
            first.map(|request| &request["url"]),
            Some(&json!(url)),
            "{id} url"
        );
    }
    for (name, value) in &expected.headers {
        let sent = first
            .and_then(|request| request["headers"].get(name))
            .unwrap_or(&Value::Null);
        assert_eq!(sent, value, "{id} header {name}");
    }
    if let Some(payload) = &expected.payload {
        let sent = first.map(|request| canonical(request["body"].clone()));
        assert_eq!(sent, Some(canonical(payload.clone())), "{id} payload");
    }
}

/// Check a run against what its row expects.
pub fn assert_run(row: &Row, run: &Run) {
    assert_request(row, run);
    let (id, expected) = (&row.id, &row.expected);
    if let Some(events) = &expected.events {
        assert_eq!(&run.events, events, "{id} events");
    }
    if let Some(result) = &expected.result {
        assert_eq!(
            canonical(run.result.clone()),
            canonical(result.clone()),
            "{id} result"
        );
    }
    if let Some(error) = &expected.error {
        assert_eq!(run.result["errorMessage"], json!(error), "{id} error");
        assert_eq!(run.result["stopReason"], json!("error"), "{id} stop reason");
    }
}

/// Rows of a fixture file owned by one test.
pub fn rows_of(fixture: &str, test: &str) -> TestResult<Vec<Row>> {
    let all: Vec<Row> = serde_json::from_str(fixture)?;
    let owned: Vec<Row> = all.into_iter().filter(|row| row.test == test).collect();
    assert!(!owned.is_empty(), "{test} has fixture rows");
    Ok(owned)
}

/// Check every row a test owns in fresh processes, so ambient environment cannot reach a call.
///
/// Rows without an environment run together in one child with an empty environment; each row
/// with one runs alone in a child holding exactly those variables.
pub async fn assert_rows(fixture: &str, test: &str) -> TestResult {
    let owned = rows_of(fixture, test)?;
    let Some(selected) = child_case() else {
        rerun(test, "*", &[])?;
        for row in owned.iter().filter(|row| row.case.env.is_some()) {
            let variables: Vec<(&str, &str)> = row
                .case
                .env
                .iter()
                .flatten()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            rerun(test, &row.id, &variables)?;
        }
        return Ok(());
    };
    for row in owned
        .iter()
        .filter(|row| (selected == "*" && row.case.env.is_none()) || row.id == selected)
    {
        assert_run(row, &run_case(&row.case).await?);
    }
    Ok(())
}
