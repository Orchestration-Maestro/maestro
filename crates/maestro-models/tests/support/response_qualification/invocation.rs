//! Public invocation, controlled cancellation and complete observation comparison.

#[path = "progress.rs"]
mod progress;
#[path = "socket.rs"]
pub(crate) mod socket;
#[path = "../socket_server.rs"]
mod socket_server;

use super::{
    Context, Phase, Route, Row, TestResult, Wire, bundled, canonical, descriptor, messages,
    transport,
};
use crate::json::event_record;
use maestro_models::{
    AssistantMessageEvent, Cancellation, OnPayload, ProviderStreamOptions, SharedAssistantMessage,
    SimpleStreamOptions, StreamOptions, complete, complete_simple, stream, stream_simple,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

/// Native event framing uses the existing protocol helpers.
fn body(wire: &Wire) -> TestResult<Vec<u8>> {
    Ok(framed(wire, wire.events.as_ref().ok_or("event body")?))
}

/// Frame the selected slice without changing protocol event identity.
fn framed(wire: &Wire, events: &[Value]) -> Vec<u8> {
    if wire.model["api"] == "anthropic-messages" {
        events
            .iter()
            .map(messages::sse)
            .collect::<String>()
            .into_bytes()
    } else {
        bundled::sse_frames(events)
    }
}

/// Account fixtures hold a marker; the existing helper assembles the synthetic token.
fn key(wire: &Wire) -> String {
    if wire.options.api_key == "account-fixture" {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let payload = URL_SAFE_NO_PAD
            .encode(r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"fixture-account"}}"#);
        format!("a.{payload}.b")
    } else {
        wire.options.api_key.clone()
    }
}

/// Caller controls retain absence and independent policy values.
fn common(
    wire: &Wire,
    fetch: maestro_models::Fetch,
    payloads: Arc<Mutex<Vec<Value>>>,
    signal: Cancellation,
) -> StreamOptions {
    let callback: OnPayload = Arc::new(
        move |maestro_models::PayloadHookInput {
                  payload,
                  signal: hook_signal,
              },
              _| {
            let observed = payload.clone();
            payloads
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(observed);
            Box::pin(std::future::ready(Ok(maestro_models::PayloadHookInput {
                payload,
                signal: hook_signal,
            })))
        },
    );
    StreamOptions {
        api_key: Some(key(wire)),
        max_tokens: wire.options.max_tokens,
        temperature: wire.options.temperature,
        session_id: wire.options.session_id.clone(),
        cache_retention: wire.options.cache_retention,
        transport: wire.options.transport.clone(),
        headers: wire.options.headers.as_ref().map(|headers| {
            headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }),
        signal: Some(signal),
        fetch: Some(fetch),
        on_payload: Some(callback),
        ..StreamOptions::default()
    }
}

/// Adapter-specific options remain open fields at the existing raw seam.
fn raw(wire: &Wire, common: StreamOptions) -> ProviderStreamOptions {
    let mut extra = maestro_models::JsonObject::new();
    for (name, value) in [
        (
            "effort",
            wire.options.effort.map(|effort| {
                json!(match effort {
                    maestro_models::AnthropicEffort::Low => "low",
                    maestro_models::AnthropicEffort::Medium => "medium",
                    maestro_models::AnthropicEffort::High => "high",
                    maestro_models::AnthropicEffort::Xhigh => "xhigh",
                    maestro_models::AnthropicEffort::Max => "max",
                })
            }),
        ),
        (
            "promptMode",
            wire.options.prompt_mode.as_ref().map(|v| json!(v)),
        ),
        (
            "reasoningEffort",
            wire.options.reasoning_effort.as_ref().map(|v| json!(v)),
        ),
        (
            "thinkingEnabled",
            wire.options.thinking_enabled.map(|v| json!(v)),
        ),
        (
            "thinkingBudgetTokens",
            wire.options.thinking_budget_tokens.map(|v| json!(v)),
        ),
        (
            "transport",
            wire.options.transport.as_ref().map(|v| json!(v)),
        ),
        (
            "azureDeploymentName",
            wire.options
                .azure_deployment_name
                .as_ref()
                .map(|v| json!(v)),
        ),
    ] {
        if let Some(value) = value {
            extra.insert(name.into(), value);
        }
    }
    ProviderStreamOptions {
        common,
        extra,
        ..ProviderStreamOptions::default()
    }
}

/// Existing one-shot transport owns body capture and release.
fn controlled(
    wire: &Wire,
    gated: bool,
) -> TestResult<(transport::Transport, Option<messages::Feed>)> {
    if wire.phase == Phase::Mid || gated {
        let (feed, body_stream) = messages::feed();
        if !gated {
            feed.chunks.send(body(wire)?)?;
        }
        let attempt = transport::Attempt::streaming_once(body_stream);
        Ok((transport::transport(vec![attempt]), Some(feed)))
    } else {
        if let Some(http) = &wire.http {
            let headers: Vec<_> = http
                .headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            let attempt =
                transport::Attempt::body(http.status, &headers, http.body.clone().into_bytes());
            return Ok((transport::transport(vec![attempt]), None));
        }
        assert_ne!(
            wire.events.is_some(),
            wire.chunks.is_some(),
            "one body source"
        );
        let chunks = if let Some(chunks) = &wire.chunks {
            chunks.clone()
        } else {
            vec![body(wire)?]
        };
        let attempt = transport::Attempt::streaming(move || {
            Box::pin(futures_util::stream::iter(
                chunks.clone().into_iter().map(Ok),
            ))
        });
        Ok((transport::transport(vec![attempt]), None))
    }
}

/// Start one invocation, compare its observations, and retain its actual result for continuation.
pub(super) async fn invoke(
    row: &Row,
    wire: &Wire,
    context: Context,
) -> TestResult<SharedAssistantMessage> {
    let history = serde_json::to_value(&context)?;
    let mut model = descriptor(wire)?;
    let peer = if wire.options.transport == Some(maestro_models::Transport::Websocket) {
        let (base, peer) = socket::serve(wire.events.clone().ok_or("socket events")?).await?;
        model.base_url = base;
        Some(peer)
    } else {
        None
    };
    let (transport, feed) = controlled(wire, row.expected.progress.is_some())?;
    let gate = progress::Gate::new(wire, row.expected.progress.as_deref(), feed)?;
    let payloads = Arc::new(Mutex::new(Vec::new()));
    let signal = Cancellation::new();
    if wire.phase == Phase::Pre {
        signal.abort();
    }
    let fetch = fetch_for(&transport, peer.is_some());
    let common = common(wire, fetch, Arc::clone(&payloads), signal.clone());
    let (result, events) = dispatch(wire, context, common, gate, model).await?;
    let diagnostic = json!({
        "model": {"id": wire.model["id"], "provider": wire.model["provider"],
            "api": wire.model["api"]},
        "inputHistory": history,
        "capturedPayloads": *payloads.lock().map_err(|_| "payloads poisoned")?,
        "actualResult": &*result.read().map_err(|_| "result poisoned")?,
    });
    compare_stream(row, &result, events, &diagnostic)?;
    compare_overflow(row, wire, &result)?;
    if let Some(peer) = peer {
        let request = peer
            .await?
            .map_err(|error| error as Box<dyn std::error::Error>)?;
        transport
            .requests
            .lock()
            .map_err(|_| "requests poisoned")?
            .push(request);
    }
    compare_requests(row, wire, &transport, &payloads, &diagnostic)?;
    Ok(result)
}

/// Reject HTTP fallback for native sockets; label controlled HTTP responses as SSE.
fn fetch_for(transport: &transport::Transport, socket: bool) -> maestro_models::Fetch {
    if socket {
        return Arc::new(|_| Box::pin(async { Err(maestro_models::FetchError::Aborted) }));
    }
    let inner = Arc::clone(&transport.fetch);
    Arc::new(move |request| {
        let response = inner(request);
        Box::pin(async move {
            let mut response = response.await?;
            response
                .headers
                .entry("content-type".into())
                .or_insert_with(|| "text/event-stream".into());
            Ok(response)
        })
    })
}

/// Dispatch preserves the raw versus simple public entry point.
async fn dispatch(
    wire: &Wire,
    context: Context,
    common: StreamOptions,
    gate: progress::Gate<'_>,
    model: maestro_models::Model,
) -> TestResult<(SharedAssistantMessage, Option<Vec<Value>>)> {
    let signal = common.signal.clone().ok_or("signal")?;
    let output = match wire.route {
        Route::Complete => (
            complete(model, context, Some(raw(wire, common))).await?,
            None,
        ),
        Route::CompleteSimple => {
            let simple = SimpleStreamOptions {
                common,
                reasoning: wire.options.reasoning,
                ..SimpleStreamOptions::default()
            };
            (complete_simple(model, context, Some(simple)).await?, None)
        }
        Route::Stream | Route::StreamSimple => {
            let started = if matches!(wire.route, Route::StreamSimple) {
                stream_simple(
                    model,
                    context,
                    Some(SimpleStreamOptions {
                        common,
                        reasoning: wire.options.reasoning,
                        ..SimpleStreamOptions::default()
                    }),
                )?
            } else {
                stream(model, context, Some(raw(wire, common)))?
            };
            streamed(&started, gate, &signal).await?
        }
    };
    Ok(output)
}

/// Drain one streaming operation and observe its shared result object.
async fn streamed(
    started: &maestro_models::AssistantMessageEventStream,
    gate: progress::Gate<'_>,
    signal: &Cancellation,
) -> TestResult<(SharedAssistantMessage, Option<Vec<Value>>)> {
    let events = collect(started, gate, signal).await?;
    let result = started.result().await;
    assert!(
        events
            .iter()
            .all(|event| Arc::ptr_eq(crate::response_output::handle(event), &result)),
        "{} shared result handle",
        "stream"
    );
    Ok((
        result,
        Some(
            events
                .iter()
                .map(event_record)
                .collect::<TestResult<Vec<_>>>()?,
        ),
    ))
}

/// Collect public events through the gate and await producer release.
/// Cancellation gates observe a text delta before cancelling; argument gates release fragments.
async fn collect(
    started: &maestro_models::AssistantMessageEventStream,
    mut gate: progress::Gate<'_>,
    signal: &Cancellation,
) -> TestResult<Vec<AssistantMessageEvent>> {
    let mut events = Vec::new();
    while let Some(event) = started.next().await {
        gate.observe(&event, signal).await?;
        events.push(event);
    }
    gate.finish().await?;
    Ok(events)
}

/// Every retained subtree has one comparison, without printing credentials.
fn compare_stream(
    row: &Row,
    result: &SharedAssistantMessage,
    events: Option<Vec<Value>>,
    diagnostic: &Value,
) -> TestResult {
    let finished = result.read().map_err(|_| "result poisoned")?.clone();
    let mut message = serde_json::to_value(&finished)?;
    assert!(
        message["timestamp"].is_number(),
        "{} result clock: {diagnostic}",
        row.id
    );
    message
        .as_object_mut()
        .ok_or("message object")?
        .shift_remove("timestamp");
    assert!(
        Some(canonical(message)) == row.expected.result.clone().map(canonical),
        "{} terminal result: {diagnostic}",
        row.id
    );
    assert!(
        events.map(|events| canonical(json!(events)))
            == row.expected.events.clone().map(|v| canonical(json!(v))),
        "{} ordered events: {diagnostic}",
        row.id
    );
    Ok(())
}

/// The returned message classifies as the retained observation says against the supplied window.
fn compare_overflow(row: &Row, wire: &Wire, result: &SharedAssistantMessage) -> TestResult {
    assert_eq!(
        row.expected.overflow.is_some(),
        wire.overflow_window.is_some(),
        "{} overflow observation and window",
        row.id
    );
    if let Some(expected) = row.expected.overflow {
        let message = result.read().map_err(|_| "result poisoned")?;
        assert_eq!(
            maestro_models::is_context_overflow(&message, wire.overflow_window),
            expected,
            "{} context overflow classification",
            row.id
        );
    }
    Ok(())
}

/// Compare the callback payload and the transport's closed request set.
fn compare_requests(
    row: &Row,
    wire: &Wire,
    transport: &transport::Transport,
    payloads: &Mutex<Vec<Value>>,
    diagnostic: &Value,
) -> TestResult {
    let payloads = payloads.lock().map_err(|_| "payloads poisoned")?.clone();
    assert!(
        canonical(json!(payloads))
            == canonical(json!(row.expected.payloads.as_ref().ok_or("payloads")?)),
        "{} payload evidence: {diagnostic}",
        row.id
    );
    let requests = transport
        .requests
        .lock()
        .map_err(|_| "requests poisoned")?
        .clone();
    let actual: Vec<_> = requests
        .iter()
        .map(|request| request_record(request, wire))
        .collect::<TestResult<_>>()?;
    let mut expected = row.expected.requests.clone().ok_or("requests")?;
    for request in &mut expected {
        if let Some(beta) = request["headers"].get_mut("anthropic-beta") {
            let names = beta
                .as_array()
                .ok_or("beta names")?
                .iter()
                .map(|name| name.as_str().ok_or("beta name"))
                .collect::<Result<Vec<_>, _>>()?;
            *beta = json!(names.join(","));
        }
    }
    assert!(
        canonical(json!(actual)) == canonical(json!(expected)),
        "{} wire request: {diagnostic}",
        row.id
    );
    Ok(())
}

/// Retain application headers while excluding only runtime-specific transport identity.
fn request_record(request: &maestro_models::HttpRequest, wire: &Wire) -> TestResult<Value> {
    let mut headers: BTreeMap<_, _> = request
        .headers
        .iter()
        .filter(|(name, _)| {
            matches!(
                name.as_str(),
                "authorization"
                    | "api-key"
                    | "anthropic-beta"
                    | "anthropic-version"
                    | "chatgpt-account-id"
                    | "openai-beta"
                    | "session_id"
                    | "x-client-request-id"
                    | "x-session-affinity"
                    | "x-affinity"
                    | "x-api-key"
                    | "x-goog-api-key"
                    | "x-initiator"
                    | "copilot-integration-id"
                    | "editor-version"
                    | "editor-plugin-version"
                    | "originator"
                    | "cf-aig-authorization"
                    | "content-type"
                    | "accept"
            ) || (name.as_str() == "user-agent"
                && (wire.model["provider"] == "github-copilot"
                    || wire.options.api_key == "sk-ant-oat-fixture"
                    || wire.model["provider"] == "kimi-coding"))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if wire.options.api_key == "account-fixture"
        && let Some(auth) = headers.get_mut("authorization")
    {
        assert!(
            *auth == format!("Bearer {}", key(wire)),
            "account authentication"
        );
        *auth = "Bearer account-fixture".into();
    }
    if wire.options.transport == Some(maestro_models::Transport::Websocket) {
        let session = headers.get("session_id").ok_or("socket session")?;
        let parsed = uuid::Uuid::parse_str(session)?;
        assert_eq!(parsed.to_string(), *session, "canonical socket identifier");
        assert_eq!(
            headers.get("x-client-request-id"),
            Some(session),
            "paired socket identifiers"
        );
        headers.insert("session_id".into(), "generated-request-id".into());
        headers.insert("x-client-request-id".into(), "generated-request-id".into());
    }
    Ok(
        json!({"url":request.url,"headers":headers,"body":serde_json::from_slice::<Value>(&request.body)?}),
    )
}
