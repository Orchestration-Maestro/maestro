//! Controlled Bedrock conversations: event-stream frames and the scripted transport that serves them.

#![cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use maestro_models::{Context, HttpBody, Model};
use serde_json::{Value, json};

use crate::chat::TestResult;
use crate::transport::{self, Attempt, Transport, transport};

/// The model every controlled conversation asks.
pub fn model(base_url: &str) -> TestResult<Model> {
    Ok(serde_json::from_value(json!({
        "id": "meta.llama-controlled", "name": "Controlled text", "api": "bedrock-converse-stream",
        "provider": "amazon-bedrock", "baseUrl": base_url, "reasoning": false, "input": ["text"],
        "cost": {"input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4},
        "contextWindow": 8192, "maxTokens": 1024
    }))?)
}

/// A conversation holding one user message.
pub fn context(text: &str) -> TestResult<Context> {
    Ok(serde_json::from_value(json!({
        "messages": [{"role": "user", "content": text, "timestamp": 1}]
    }))?)
}

/// The CRC-32 the event-stream framing uses.
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0_u32, |crc, byte| {
        (0..8).fold(crc ^ u32::from(*byte), |crc, _| {
            (crc >> 1) ^ (0xEDB8_8320 & 0_u32.wrapping_sub(crc & 1))
        })
    })
}

/// One event-stream frame with the given string headers and JSON payload.
fn encode(headers: &[(&str, &str)], payload: &Value) -> TestResult<Vec<u8>> {
    let mut block = Vec::new();
    for (name, value) in headers {
        block.push(u8::try_from(name.len())?);
        block.extend_from_slice(name.as_bytes());
        block.push(7);
        block.extend_from_slice(&u16::try_from(value.len())?.to_be_bytes());
        block.extend_from_slice(value.as_bytes());
    }
    let payload = serde_json::to_vec(payload)?;
    let total = u32::try_from(16 + block.len() + payload.len())?;
    let mut frame = Vec::new();
    frame.extend_from_slice(&total.to_be_bytes());
    frame.extend_from_slice(&u32::try_from(block.len())?.to_be_bytes());
    frame.extend_from_slice(&crc32(&frame).to_be_bytes());
    frame.extend_from_slice(&block);
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&crc32(&frame).to_be_bytes());
    Ok(frame)
}

/// A stream event frame.
pub fn event(kind: &str, payload: &Value) -> TestResult<Vec<u8>> {
    encode(
        &[
            (":message-type", "event"),
            (":event-type", kind),
            (":content-type", "application/json"),
        ],
        payload,
    )
}

/// A stream exception frame.
pub fn exception(kind: &str, message: &str) -> TestResult<Vec<u8>> {
    encode(
        &[
            (":message-type", "exception"),
            (":exception-type", kind),
            (":content-type", "application/json"),
        ],
        &json!({"message": message}),
    )
}

/// The frames of an assistant message that answers `text` in one block.
pub fn answer(text: &str) -> TestResult<Vec<Vec<u8>>> {
    Ok(vec![
        event("messageStart", &json!({"role": "assistant"}))?,
        event(
            "contentBlockDelta",
            &json!({"contentBlockIndex": 0, "delta": {"text": text}}),
        )?,
        event("contentBlockStop", &json!({"contentBlockIndex": 0}))?,
        event("messageStop", &json!({"stopReason": "end_turn"}))?,
    ])
}

/// A transport that answers every request with a successful stream of `chunks`.
pub fn serve(chunks: Vec<Vec<u8>>) -> Transport {
    transport(vec![stream_of(
        200,
        &[("x-amzn-requestid", "fixture-id")],
        chunks,
    )])
}

/// An attempt that answers with the given status and headers and the chunks as body reads.
pub fn stream_of(status: u16, headers: &[(&str, &str)], chunks: Vec<Vec<u8>>) -> Attempt {
    Attempt::streaming_with_headers(status, headers, move || {
        Box::pin(futures_util::stream::iter(
            chunks.clone().into_iter().map(Ok),
        ))
    })
}

/// A body that records its first read and then yields `chunks`.
pub fn watched(polled: Arc<std::sync::atomic::AtomicBool>, chunks: Vec<Vec<u8>>) -> HttpBody {
    use futures_util::StreamExt;
    Box::pin(
        futures_util::stream::once(async move {
            polled.store(true, std::sync::atomic::Ordering::SeqCst);
            futures_util::stream::iter(chunks.into_iter().map(Ok))
        })
        .flatten(),
    )
}

/// Sets its flag when dropped.
pub struct DropFlag(pub Arc<std::sync::atomic::AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A body that yields `before`, then waits for `gate`, then yields `after` and reports on `end`
/// that a reader reached its end. The guard is dropped when the body is dropped, or once the gate has fired.
pub fn gated(
    before: Vec<Vec<u8>>,
    gate: tokio::sync::oneshot::Receiver<()>,
    after: Vec<Vec<u8>>,
    guard: DropFlag,
    end: tokio::sync::oneshot::Sender<()>,
) -> HttpBody {
    use futures_util::StreamExt;
    let wait = futures_util::stream::once(async move {
        let _guard = guard;
        gate.await.ok();
        futures_util::stream::iter(after.into_iter().map(Ok))
    })
    .flatten();
    let ended = futures_util::stream::once(async move {
        end.send(()).ok();
        futures_util::stream::empty()
    })
    .flatten();
    Box::pin(
        futures_util::stream::iter(before.into_iter().map(Ok))
            .chain(wait)
            .chain(ended),
    )
}

/// The region and access key a `SigV4` Authorization header names.
pub fn credential_scope(authorization: &str) -> Option<(&str, &str)> {
    let credential = authorization
        .split("Credential=")
        .nth(1)?
        .split(',')
        .next()?;
    let mut parts = credential.split('/');
    let key = parts.next()?;
    let region = parts.nth(1)?;
    Some((key, region))
}

use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
use maestro_models::{AssistantMessage, StreamOptions};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    kind: String,
    body: Value,
    #[serde(default)]
    exception: bool,
    #[serde(default)]
    corrupt: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    frames: Vec<Frame>,
    events: Vec<Value>,
    message: Value,
    #[serde(default)]
    hook_error: Option<String>,
    response_hooks: usize,
    #[serde(default)]
    route: Option<Route>,
    #[serde(default)]
    options: serde_json::Map<String, Value>,
    #[serde(default)]
    model: Option<Model>,
    #[serde(default)]
    context: Option<Context>,
    #[serde(default)]
    payload: Option<Value>,
    #[serde(default)]
    wire: Option<Value>,
    #[serde(default)]
    destination: Option<String>,
    #[serde(default)]
    replay_from: Option<String>,
    #[serde(default)]
    signing_identity: Option<String>,
    #[serde(default)]
    abort_on: Option<String>,
    #[serde(default)]
    pre_abort: bool,
}

/// Generic invocation route selected by a fixture.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Route {
    Raw,
    Simple,
}

/// Read a selected common option from a recorded invocation.
fn selected<T: serde::de::DeserializeOwned>(
    extra: &serde_json::Map<String, Value>,
    key: &str,
) -> TestResult<Option<T>> {
    Ok(extra
        .get(key)
        .cloned()
        .map(serde_json::from_value)
        .transpose()?)
}

/// Construct the recorded common options, retaining the controlled effects.
fn common(
    extra: &serde_json::Map<String, Value>,
    base: StreamOptions,
) -> TestResult<StreamOptions> {
    Ok(StreamOptions {
        cache_retention: selected(extra, "cacheRetention")?
            .or(Some(maestro_models::CacheRetention::None)),
        temperature: selected(extra, "temperature")?,
        max_tokens: selected(extra, "maxTokens")?,
        api_key: selected(extra, "apiKey")?,
        headers: selected(extra, "headers")?,
        timeout_ms: selected(extra, "timeoutMs")?,
        max_retries: selected(extra, "maxRetries")?,
        max_retry_delay_ms: selected(extra, "maxRetryDelayMs")?,
        transport: selected(extra, "transport")?,
        session_id: selected(extra, "sessionId")?,
        metadata: selected(extra, "metadata")?,
        ..base
    })
}

/// Options with the supplied transport, fixture bearer token and remaining defaults.
pub fn options(scripted: &transport::Transport) -> BedrockOptions {
    BedrockOptions {
        common: StreamOptions {
            fetch: Some(scripted.fetch.clone()),
            ..StreamOptions::default()
        },
        bearer_token: Some("fixture-token".into()),
        ..BedrockOptions::default()
    }
}

/// Diagnostic with the supplied message and absent name, stack and code.
pub fn diagnostic(text: &str) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo {
        name: None,
        message: text.into(),
        stack: None,
        code: None,
    }
}

fn encode_record(frame: &Frame) -> TestResult<Vec<u8>> {
    let mut bytes = if frame.exception {
        exception(
            &frame.kind,
            frame.body["message"].as_str().ok_or("exception message")?,
        )?
    } else {
        event(&frame.kind, &frame.body)?
    };
    if frame.corrupt {
        *bytes.last_mut().ok_or("empty frame")? ^= 0xFF;
    }
    Ok(bytes)
}

/// Check the consumed recorded cases through the native adapter.
pub async fn corpus(text: &str) -> TestResult {
    let cases: Vec<Case> = serde_json::from_str(text)?;
    assert!(!cases.is_empty());
    for case in cases {
        check_case(case).await?;
    }
    Ok(())
}

/// Counters and payloads recorded by the supplied hooks.
struct Observations {
    /// Response hook invocations, observed after settlement.
    responses: Arc<std::sync::atomic::AtomicUsize>,
    /// Payloads passed to the request hook.
    payloads: Arc<std::sync::Mutex<Vec<Value>>>,
}

/// Install the case's hooks on the controlled transport.
fn observed_options(
    case: &Case,
    scripted: &Transport,
) -> TestResult<(BedrockOptions, Observations)> {
    let responses = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&responses);
    let payloads = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&payloads);
    let mut options = options(scripted);
    let fetch = scripted.fetch.clone();
    options.common.fetch = Some(Arc::new(move |request| {
        if request
            .signal
            .as_ref()
            .is_some_and(maestro_models::Cancellation::is_aborted)
        {
            Box::pin(std::future::ready(Err(maestro_models::FetchError::Aborted)))
        } else {
            fetch(request)
        }
    }));
    options.common = common(&case.options, options.common)?;
    options.common.on_payload = Some(Arc::new(
        move |maestro_models::PayloadHookInput {
                  payload,
                  signal: hook_signal,
              },
              _| {
            seen.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(payload.clone());
            Box::pin(std::future::ready(Ok(maestro_models::PayloadHookInput {
                payload,
                signal: hook_signal,
            })))
        },
    ));
    let error = case.hook_error.clone();
    options.common.on_response = Some(Arc::new(move |_, _| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let result = error.as_ref().map_or(Ok(()), |text| Err(diagnostic(text)));
        Box::pin(std::future::ready(result))
    }));
    Ok((
        options,
        Observations {
            responses,
            payloads,
        },
    ))
}

/// Invoke the direct or registered route selected by this case.
fn start_case(
    case: &Case,
    options: BedrockOptions,
) -> TestResult<maestro_models::AssistantMessageEventStream> {
    let model = case
        .model
        .clone()
        .unwrap_or(serde_json::from_str(include_str!(
            "../fixtures/cloud_conversation/model.json"
        ))?);
    let context = case.context.clone().unwrap_or(context("Hello")?);
    Ok(match case.route {
        None => stream_bedrock(model, context, Some(options)),
        Some(Route::Raw) => maestro_models::stream(
            model,
            context,
            Some(maestro_models::ProviderStreamOptions {
                common: options.common,
                extra: case.options.clone(),
                ..maestro_models::ProviderStreamOptions::default()
            }),
        )?,
        Some(Route::Simple) => maestro_models::stream_simple(
            model,
            context,
            Some(maestro_models::SimpleStreamOptions {
                reasoning: selected(&case.options, "reasoning")?,
                thinking_budgets: selected(&case.options, "thinkingBudgets")?,
                common: options.common,
                ..maestro_models::SimpleStreamOptions::default()
            }),
        )?,
    })
}

/// Consume ordered updates, verify their shared identity and compare the final message.
async fn check_stream(
    case: &Case,
    stream: &maestro_models::AssistantMessageEventStream,
    signal: &maestro_models::Cancellation,
) -> TestResult<AssistantMessage> {
    let mut events = Vec::new();
    let mut handles = Vec::new();
    while let Some(event) = stream.next().await {
        handles.push(Arc::clone(crate::response_output::handle(&event)));
        let record = json::event_record(&event)?;
        if record["type"].as_str() == case.abort_on.as_deref() {
            signal.abort();
        }
        events.push(record);
    }
    assert_eq!(events, case.events, "{}", case.name);
    let result = stream.result().await;
    assert!(
        handles.iter().all(|seen| Arc::ptr_eq(seen, &result)),
        "{}",
        case.name
    );
    let mut message = result.read().map_err(|e| e.to_string())?.clone();
    message.timestamp = 0.0;
    message.error_message = None;
    let mut expected = case.message.clone();
    expected["timestamp"] = Value::from(0);
    let expected: AssistantMessage = serde_json::from_value(expected)?;
    assert_eq!(message, expected, "{}", case.name);
    for (actual, expected) in message.content.iter().zip(&expected.content) {
        if let (
            maestro_models::AssistantContent::ToolCall(actual),
            maestro_models::AssistantContent::ToolCall(expected),
        ) = (actual, expected)
        {
            assert_eq!(
                actual
                    .arguments
                    .as_object()
                    .map(|members| members.keys().collect::<Vec<_>>()),
                expected
                    .arguments
                    .as_object()
                    .map(|members| members.keys().collect::<Vec<_>>()),
                "{}",
                case.name
            );
        }
    }
    Ok(message)
}

/// Check each recorded request observation after the stream has settled.
fn check_requests(case: &Case, scripted: &Transport, seen: &Observations) -> TestResult {
    assert_eq!(
        seen.responses.load(std::sync::atomic::Ordering::SeqCst),
        case.response_hooks,
        "{}",
        case.name
    );
    if let Some(payload) = &case.payload {
        let seen = seen.payloads.lock().map_err(|e| e.to_string())?;
        assert_eq!(seen.len(), 1, "{}", case.name);
        check_metadata_keys(&seen[0], payload)?;
        assert_eq!(&json::canonical(seen[0].clone()), payload, "{}", case.name);
    }
    let requests = scripted.seen()?;
    if let Some(identity) = &case.signing_identity {
        let authorization = requests[0]
            .headers
            .get("authorization")
            .ok_or("authorization")?;
        assert!(
            credential_scope(authorization)
                .is_some_and(|(key, region)| key == identity && region == "eu-west-1"),
            "{}",
            case.name
        );
    }
    if case
        .options
        .get("bearerToken")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty())
    {
        assert!(
            requests[0]
                .headers
                .get("authorization")
                .is_some_and(|value| value == "Bearer fixture-token"),
            "{}",
            case.name
        );
    }
    if let Some(wire) = &case.wire {
        assert_eq!(requests.len(), 1, "{}", case.name);
        let body = serde_json::from_slice(&requests[0].body)?;
        check_metadata_keys(&body, wire)?;
        assert_eq!(&json::canonical(body), wire, "{}", case.name);
    }
    if let Some(destination) = &case.destination {
        assert_eq!(
            url::Url::parse(&requests[0].url)?.host_str(),
            Some(destination.as_str()),
            "{}",
            case.name
        );
    }
    Ok(())
}

/// Compare metadata insertion order independently of JSON object equality.
fn check_metadata_keys(actual: &Value, expected: &Value) -> TestResult {
    if let Some(metadata) = expected.get("requestMetadata") {
        let expected = metadata.as_object().ok_or("expected metadata object")?;
        let actual = actual["requestMetadata"]
            .as_object()
            .ok_or("metadata object")?;
        assert_eq!(
            actual.keys().collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>()
        );
    }
    Ok(())
}

/// Run and verify one complete case, retaining its actual output for replay.
async fn check_case(case: Case) -> TestResult<AssistantMessage> {
    let frames = case
        .frames
        .iter()
        .map(encode_record)
        .collect::<TestResult<Vec<_>>>()?;
    let (scripted, pending) = if case.abort_on.is_some() {
        let pending = held(frames, vec![]);
        (pending.scripted.clone(), Some(pending))
    } else {
        (serve(frames), None)
    };
    let signal = maestro_models::Cancellation::new();
    if case.pre_abort {
        signal.abort();
    }
    let (mut options, seen) = observed_options(&case, &scripted)?;
    options.common.signal = Some(signal.clone());
    let stream = start_case(&case, options)?;
    let message = check_stream(&case, &stream, &signal).await?;
    check_requests(&case, &scripted, &seen)?;
    if let Some(mut pending) = pending {
        assert!(pending.dropped.load(std::sync::atomic::Ordering::SeqCst));
        pending.received.recv().await.ok_or("no producer")?;
        pending.release.send(()).ok();
        drop(pending.ended);
    }
    Ok(message)
}

/// Feed each first response into its recorded second request, using the actual signed content.
pub async fn replay_corpus(text: &str) -> TestResult {
    let cases: Vec<Case> = serde_json::from_str(text)?;
    let mut responses: std::collections::HashMap<String, AssistantMessage> =
        std::collections::HashMap::new();
    for mut case in cases {
        if let Some(model) = &case.replay_from {
            let actual = responses.get(model).ok_or("no first response")?;
            case.context
                .as_mut()
                .ok_or("replay context")?
                .messages
                .insert(1, maestro_models::Message::Assistant(actual.clone()));
        }
        let response = check_case(case).await?;
        responses.insert(response.model.clone(), response);
    }
    Ok(())
}

/// A sender whose body yields `before`, then waits for the returned gate.
pub struct Held {
    pub scripted: transport::Transport,
    pub release: tokio::sync::oneshot::Sender<()>,
    pub dropped: Arc<std::sync::atomic::AtomicBool>,
    pub ended: tokio::sync::oneshot::Receiver<()>,
    pub received: tokio::sync::mpsc::UnboundedReceiver<()>,
}

pub fn held(before: Vec<Vec<u8>>, after: Vec<Vec<u8>>) -> Held {
    let (release, gate) = tokio::sync::oneshot::channel();
    let (end, ended) = tokio::sync::oneshot::channel();
    let (sender, received) = tokio::sync::mpsc::unbounded_channel();
    let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let guard = std::sync::Mutex::new(Some((gate, end, DropFlag(Arc::clone(&dropped)))));
    let scripted = transport::transport(vec![transport::Attempt::streaming(move || {
        sender.send(()).ok();
        match guard.lock().ok().and_then(|mut guard| guard.take()) {
            Some((gate, end, flag)) => gated(before.clone(), gate, after.clone(), flag, end),
            None => Box::pin(futures_util::stream::empty()),
        }
    })]);
    Held {
        scripted,
        release,
        dropped,
        ended,
        received,
    }
}

pub(crate) use crate::json;
