//! Controlled proxy transport and lossless observations of the reconstructed stream.
use super::owned::Tasks;
use futures_util::FutureExt;
use maestro_agent::{ProxyStreamOptions, stream_proxy};
use maestro_models::{
    AssistantMessageEvent, AssistantMessageEventStream, BoxFuture, Cancellation, Context, Fetch,
    FetchError, HttpRequest, HttpResponse, SharedAssistantMessage, SimpleStreamOptions,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

/// Reported usage with distinguishable, nonzero counts and costs.
pub const USAGE: &str = r#"{"input":7,"output":3,"cacheRead":2,"cacheWrite":1,"totalTokens":13,"cost":{"input":0.7,"output":0.6,"cacheRead":0.2,"cacheWrite":0.1,"total":9.5}}"#;
/// Proxy base address used by every controlled run.
pub const PROXY: &str = "https://proxy.example/";
/// Local bearer credential used by every controlled run.
pub const TOKEN: &str = "local-credential";

/// One complete wire line.
#[must_use]
pub fn line(payload: &str) -> Vec<u8> {
    format!("data: {payload}\n").into_bytes()
}
/// A terminal success line with the reported usage.
#[must_use]
pub fn done(reason: &str) -> Vec<u8> {
    line(&format!(
        r#"{{"type":"done","reason":"{reason}","usage":{USAGE}}}"#
    ))
}
/// Concatenate wire lines into one chunk.
#[must_use]
pub fn join(lines: &[Vec<u8>]) -> Vec<u8> {
    lines.concat()
}
/// A scripted response.
#[derive(Clone)]
pub struct Reply {
    /// Status code.
    pub status: u16,
    /// Status phrase.
    pub status_text: String,
    /// Body chunks or the failure that ends the body.
    pub body: Vec<Result<Vec<u8>, FetchError>>,
}
impl Reply {
    /// A successful response delivering `chunks` in order.
    #[must_use]
    pub fn chunks(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            status: 200,
            status_text: "OK".into(),
            body: chunks.into_iter().map(Ok).collect(),
        }
    }
    /// A response with an arbitrary status and one body chunk.
    #[must_use]
    pub fn status(status: u16, status_text: &str, body: &[u8]) -> Self {
        Self {
            status,
            status_text: status_text.into(),
            body: vec![Ok(body.to_vec())],
        }
    }
}

/// What the transport received.
#[derive(Clone)]
pub struct Seen {
    /// Request URL.
    pub url: String,
    /// Request method.
    pub method: String,
    /// Header names and values in insertion order.
    pub headers: Vec<(String, String)>,
    /// Request body.
    pub body: Vec<u8>,
    /// Whether the request carried a cancellation signal.
    pub signal: Option<Cancellation>,
}
impl Seen {
    /// Value of a header.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value.as_str())
    }
}
/// Requests recorded by a controlled transport.
pub type Requests = Arc<Mutex<Vec<Seen>>>;

/// Record a request and return its observation.
fn record(requests: &Requests, request: HttpRequest) {
    requests.lock().unwrap().push(Seen {
        url: request.url,
        method: request.method,
        headers: request.headers.into_iter().collect(),
        body: request.body,
        signal: request.signal,
    });
}
/// A transport answering every attempt with `reply`.
#[must_use]
pub fn replying(reply: Reply) -> (Fetch, Requests) {
    let requests = Requests::default();
    let seen = Arc::clone(&requests);
    let fetch: Fetch = Arc::new(move |request| {
        record(&seen, request);
        let reply = reply.clone();
        Box::pin(async move {
            Ok(HttpResponse {
                status: reply.status,
                status_text: reply.status_text,
                headers: std::collections::BTreeMap::default(),
                body: Box::pin(futures_util::stream::iter(reply.body)),
            })
        })
    });
    (fetch, requests)
}
/// A transport whose attempts all fail with `message`.
#[must_use]
pub fn failing(message: &str) -> (Fetch, Requests) {
    let requests = Requests::default();
    let seen = Arc::clone(&requests);
    let message = message.to_owned();
    let fetch: Fetch = Arc::new(move |request| {
        record(&seen, request);
        let cause = maestro_models::DiagnosticErrorInfo::new(None, message.clone());
        Box::pin(async move { Err(FetchError::Connection(cause)) })
    });
    (fetch, requests)
}

/// Handles of a body the test releases chunk by chunk.
pub struct Gate {
    /// Release the next chunk, a failure or (`None`) the end of the body.
    pub send: mpsc::UnboundedSender<Option<Result<Vec<u8>, FetchError>>>,
    /// One message per body read, sent when the read begins.
    pub reads: mpsc::UnboundedReceiver<()>,
    /// Resolves when the body is dropped.
    pub dropped: oneshot::Receiver<()>,
}
/// Retains the drop witness for as long as the body lives.
struct Dropped(Option<oneshot::Sender<()>>);
impl Drop for Dropped {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            sender.send(()).ok();
        }
    }
}
/// A transport answering with a gated body and the handles that drive it.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
#[must_use]
pub fn gated() -> (Fetch, Requests, Gate) {
    let requests = Requests::default();
    let seen = Arc::clone(&requests);
    let (send, chunks) = mpsc::unbounded_channel();
    let (reads_tx, reads) = mpsc::unbounded_channel();
    let (dropped_tx, dropped) = oneshot::channel();
    let state = Mutex::new(Some((chunks, reads_tx, Dropped(Some(dropped_tx)))));
    let fetch: Fetch = Arc::new(move |request| {
        record(&seen, request);
        let (chunks, reads, held) = state.lock().unwrap().take().expect("one attempt");
        Box::pin(async move {
            let body = futures_util::stream::unfold(
                (chunks, reads, held),
                |(mut chunks, reads, held)| async move {
                    reads.send(()).ok();
                    let next = chunks.recv().await??;
                    Some((next, (chunks, reads, held)))
                },
            );
            Ok(HttpResponse {
                status: 200,
                status_text: "OK".into(),
                headers: std::collections::BTreeMap::default(),
                body: Box::pin(body),
            })
        }) as BoxFuture<_>
    });
    (
        fetch,
        requests,
        Gate {
            send,
            reads,
            dropped,
        },
    )
}

/// Options selecting a controlled transport.
#[must_use]
pub fn with_fetch(fetch: Fetch) -> SimpleStreamOptions {
    let mut options = SimpleStreamOptions::default();
    options.common.fetch = Some(fetch);
    options
}
/// Wrap options with the fixed proxy address and credential.
#[must_use]
pub fn proxy(options: SimpleStreamOptions) -> ProxyStreamOptions {
    ProxyStreamOptions {
        options,
        auth_token: TOKEN.into(),
        proxy_url: PROXY.into(),
    }
}
/// An empty conversation.
#[must_use]
pub fn context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    }
}
/// Start the proxy and queue its producer without running it.
#[must_use]
pub fn start(options: SimpleStreamOptions) -> (AssistantMessageEventStream, Arc<Tasks>) {
    start_at(proxy(options))
}
/// Start a proxy with explicit address and credential, queueing its producer without running it.
#[must_use]
pub fn start_at(proxy: ProxyStreamOptions) -> (AssistantMessageEventStream, Arc<Tasks>) {
    let tasks = Arc::new(Tasks::default());
    let stream = stream_proxy(super::model(), context(), proxy, tasks.spawner());
    (stream, tasks)
}

/// Event shape without its live message handle.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
#[must_use]
pub fn shape(event: &AssistantMessageEvent) -> Value {
    let mut value = serde_json::to_value(event).unwrap();
    let object = value.as_object_mut().unwrap();
    for handle in ["partial", "message", "error"] {
        if object.get(handle).is_some_and(Value::is_object) {
            object.shift_remove(handle);
        }
    }
    value
}
/// Write integral floating-point numbers as integers so expectations compare by value.
fn integral(value: Value) -> Value {
    match value {
        Value::Number(number) => match number.as_f64() {
            Some(float) if float.fract() == 0.0 => format!("{float}")
                .parse::<i64>()
                .map_or(Value::Number(number), |whole| json!(whole)),
            _ => Value::Number(number),
        },
        Value::Array(items) => Value::Array(items.into_iter().map(integral).collect()),
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .map(|(key, value)| (key, integral(value)))
                .collect(),
        ),
        other => other,
    }
}
/// The message without its clock reading.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
#[must_use]
pub fn snapshot(message: &SharedAssistantMessage) -> Value {
    let mut value = serde_json::to_value(&*message.read().unwrap()).unwrap();
    value.as_object_mut().unwrap().shift_remove("timestamp");
    integral(value)
}
/// Expected message: fixture identity and the given content, stop reason and usage.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
#[must_use]
pub fn message(content: &Value, stop: &str, error: Option<&str>, usage: &str) -> Value {
    let mut expected = json!({
        "role": "assistant", "content": content, "api": "fixture", "provider": "fixture",
        "model": "fixture", "usage": serde_json::from_str::<Value>(usage).unwrap(),
        "stopReason": stop,
    });
    if let Some(error) = error {
        expected["errorMessage"] = json!(error);
    }
    expected
}
/// Usage with every count and cost zero.
pub const ZERO: &str = r#"{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}"#;

/// A finished run: every event, its handles and the shared final message.
pub struct Run {
    /// Event shapes in delivery order.
    pub events: Vec<Value>,
    /// Event handles in delivery order.
    pub raw: Vec<AssistantMessageEvent>,
    /// Shared final message.
    pub result: SharedAssistantMessage,
}
impl Run {
    /// The final message snapshot.
    #[must_use]
    pub fn message(&self) -> Value {
        snapshot(&self.result)
    }
    /// Event type names in delivery order.
    ///
    /// # Panics
    /// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
    #[must_use]
    pub fn kinds(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|event| event["type"].as_str().unwrap().to_owned())
            .collect()
    }
}
/// Run the producer to completion and collect everything the stream delivered.
pub async fn finish(stream: &AssistantMessageEventStream, tasks: &Tasks) -> Run {
    tasks.take().await;
    collect(stream).await
}
/// Collect everything a stream delivers once its producer has finished.
///
/// # Panics
/// Panics if the finished producer left the stream's result unsettled.
pub async fn collect(stream: &AssistantMessageEventStream) -> Run {
    let mut raw = vec![];
    while let Some(event) = stream.next().await {
        raw.push(event);
    }
    Run {
        events: raw.iter().map(shape).collect(),
        raw,
        result: stream
            .result()
            .now_or_never()
            .expect("the finished producer settled the result"),
    }
}
/// Run one request against `reply` with default options.
pub async fn exchange(reply: Reply) -> (Run, Requests) {
    let (fetch, requests) = replying(reply);
    (run_with(with_fetch(fetch)).await, requests)
}
/// Run one request with prepared options.
pub async fn run_with(options: SimpleStreamOptions) -> Run {
    let (stream, tasks) = start(options);
    finish(&stream, &tasks).await
}
/// Run a successful body made of `chunks`.
pub async fn run_chunks(chunks: Vec<Vec<u8>>) -> Run {
    exchange(Reply::chunks(chunks)).await.0
}
/// Run lines delivered as one chunk.
pub async fn run_lines(lines: &[Vec<u8>]) -> Run {
    run_chunks(vec![join(lines)]).await
}

/// The canonical response: one signed text block ending in a success.
#[must_use]
pub fn canonical_lines() -> Vec<Vec<u8>> {
    vec![
        line(r#"{"type":"start"}"#),
        line(r#"{"type":"text_start","contentIndex":0}"#),
        line(r#"{"type":"text_delta","contentIndex":0,"delta":"hé🙂"}"#),
        line(r#"{"type":"text_delta","contentIndex":0,"delta":"!"}"#),
        line(r#"{"type":"text_end","contentIndex":0,"contentSignature":"signature"}"#),
        done("stop"),
    ]
}
/// Event shapes the canonical response yields.
#[must_use]
pub fn canonical_events() -> Vec<Value> {
    vec![
        json!({"type": "start"}),
        json!({"type": "text_start", "contentIndex": 0}),
        json!({"type": "text_delta", "contentIndex": 0, "delta": "hé🙂"}),
        json!({"type": "text_delta", "contentIndex": 0, "delta": "!"}),
        json!({"type": "text_end", "contentIndex": 0, "content": "hé🙂!"}),
        json!({"type": "done", "reason": "stop"}),
    ]
}
/// Final message the canonical response yields.
#[must_use]
pub fn canonical_message() -> Value {
    let content = json!([{"type": "text", "text": "hé🙂!", "textSignature": "signature"}]);
    message(&content, "stop", None, USAGE)
}

/// Handles of a transport whose request setup never finishes by itself.
pub struct Held {
    /// One message when the setup future is first polled.
    pub started: mpsc::UnboundedReceiver<()>,
    /// Resolves when the setup future is dropped.
    pub dropped: oneshot::Receiver<()>,
}
/// A transport whose attempts wait forever, with witnesses of their start and drop.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
#[must_use]
pub fn held() -> (Fetch, Requests, Held) {
    let requests = Requests::default();
    let seen = Arc::clone(&requests);
    let (started_tx, started) = mpsc::unbounded_channel();
    let (dropped_tx, dropped) = oneshot::channel();
    let witness = Mutex::new(Some(dropped_tx));
    let fetch: Fetch = Arc::new(move |request| {
        record(&seen, request);
        let started = started_tx.clone();
        let guard = Dropped(witness.lock().unwrap().take());
        Box::pin(async move {
            started.send(()).ok();
            let _guard = guard;
            std::future::pending::<()>().await;
            Err(FetchError::Aborted)
        })
    });
    (fetch, requests, Held { started, dropped })
}

/// A transport selected for the proxy stream function.
#[must_use]
pub fn proxy_stream_fn(
    url: String,
    fetch: Option<maestro_models::Fetch>,
) -> maestro_agent::StreamFn {
    let spawner: maestro_agent::AgentSpawner = std::sync::Arc::new(|task| {
        tokio::spawn(task);
    });
    std::sync::Arc::new(move |model, context, mut options, _| {
        options.common.fetch = fetch.clone();
        let proxy = ProxyStreamOptions {
            options,
            auth_token: TOKEN.into(),
            proxy_url: url.clone(),
        };
        let stream = stream_proxy(model, context, proxy, spawner.clone());
        Box::pin(std::future::ready(Ok(stream)))
    })
}

/// An Agent whose model calls go through the proxy stream function.
pub fn proxy_agent(stream_fn: maestro_agent::StreamFn) -> std::sync::Arc<maestro_agent::Agent> {
    let spawn: maestro_agent::AgentSpawner = std::sync::Arc::new(|task| {
        tokio::spawn(task);
    });
    std::sync::Arc::new(maestro_agent::Agent::new(maestro_agent::AgentOptions {
        spawn: Some(spawn),
        stream_fn: Some(stream_fn),
        ..Default::default()
    }))
}
