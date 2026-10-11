//! Public invocation, controlled cancellation and complete observation comparison.

use super::{
    Context, Phase, Route, Row, TestResult, Wire, bundled, canonical, descriptor, messages,
    transport,
};
use maestro_models::{
    AssistantMessageEvent, Cancellation, OnPayload, ProviderStreamOptions, SharedAssistantMessage,
    SimpleStreamOptions, StreamOptions, complete, complete_simple, stream,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

/// Native event framing uses the existing protocol helpers.
fn body(wire: &Wire) -> Vec<u8> {
    if wire.model["api"] == "anthropic-messages" {
        wire.events
            .iter()
            .map(messages::sse)
            .collect::<String>()
            .into_bytes()
    } else {
        bundled::sse_frames(&wire.events)
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
    let callback: OnPayload = Arc::new(move |payload, _| {
        let observed = payload.clone();
        payloads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(observed);
        Box::pin(std::future::ready(Ok(payload)))
    });
    StreamOptions {
        api_key: Some(key(wire)),
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
fn controlled(wire: &Wire) -> TestResult<(transport::Transport, Option<messages::Feed>)> {
    if wire.phase == Phase::Mid {
        let (feed, body_stream) = messages::feed();
        feed.chunks.send(body(wire))?;
        let body = Mutex::new(Some(body_stream));
        let attempt = transport::Attempt::streaming(move || {
            body.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| {
                    Box::pin(futures_util::stream::iter([Err(
                        maestro_models::FetchError::Aborted,
                    )]))
                })
        });
        Ok((transport::transport(vec![attempt]), Some(feed)))
    } else {
        let chunks = body(wire);
        let attempt = transport::Attempt::streaming(move || {
            Box::pin(futures_util::stream::iter([Ok(chunks.clone())]))
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
    let (transport, feed) = controlled(wire)?;
    let payloads = Arc::new(Mutex::new(Vec::new()));
    let signal = Cancellation::new();
    if wire.phase == Phase::Pre {
        signal.abort();
    }
    let inner = Arc::clone(&transport.fetch);
    let fetch: maestro_models::Fetch = Arc::new(move |request| {
        let response = inner(request);
        Box::pin(async move {
            let mut response = response.await?;
            response
                .headers
                .insert("content-type".into(), "text/event-stream".into());
            Ok(response)
        })
    });
    let common = common(wire, fetch, Arc::clone(&payloads), signal.clone());
    let (result, events) = dispatch(wire, context, common, feed, &signal).await?;
    let diagnostic = json!({
        "model": {"id": wire.model["id"], "provider": wire.model["provider"],
            "api": wire.model["api"]},
        "inputHistory": history,
        "capturedPayloads": *payloads.lock().map_err(|_| "payloads poisoned")?,
        "actualResult": &*result.read().map_err(|_| "result poisoned")?,
    });
    compare_stream(row, &result, events, &diagnostic)?;
    compare_requests(row, wire, &transport, &payloads, &diagnostic)?;
    Ok(result)
}

/// Dispatch preserves the raw versus simple public entry point.
async fn dispatch(
    wire: &Wire,
    context: Context,
    common: StreamOptions,
    feed: Option<messages::Feed>,
    signal: &Cancellation,
) -> TestResult<(SharedAssistantMessage, Option<Vec<Value>>)> {
    let model = descriptor(wire)?;
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
        Route::Stream => {
            let started = stream(model, context, Some(raw(wire, common)))?;
            let events = collect(&started, feed, signal).await?;
            let result = started.result().await;
            assert!(
                events
                    .iter()
                    .all(|event| Arc::ptr_eq(handle(event), &result)),
                "{} shared result handle",
                "stream"
            );
            (
                result,
                Some(
                    events
                        .iter()
                        .map(event_record)
                        .collect::<TestResult<Vec<_>>>()?,
                ),
            )
        }
    };
    Ok(output)
}

/// Observe a public text delta before cancellation, then drain terminal and await body release.
async fn collect(
    started: &maestro_models::AssistantMessageEventStream,
    mut feed: Option<messages::Feed>,
    signal: &Cancellation,
) -> TestResult<Vec<AssistantMessageEvent>> {
    let mut events = Vec::new();
    while let Some(event) = started.next().await {
        let text =
            matches!(&event, AssistantMessageEvent::TextDelta { delta, .. } if delta.len() > 50);
        events.push(event);
        if text && let Some(feed) = feed.as_mut() {
            feed.read_pending().await?;
            signal.abort();
            break;
        }
    }
    events.extend(bundled::drain(started).await);
    if let Some(feed) = feed {
        feed.released.await?;
    }
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
        .remove("timestamp");
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
    Ok(
        json!({"url":request.url,"headers":headers,"body":serde_json::from_slice::<Value>(&request.body)?}),
    )
}

/// Ordered event fields exclude only shared message objects.
fn event_record(event: &AssistantMessageEvent) -> TestResult<Value> {
    let mut record = serde_json::to_value(event)?;
    for field in ["partial", "message", "error"] {
        record.as_object_mut().ok_or("event object")?.remove(field);
    }
    Ok(record)
}

/// Each public event carries the same invocation object.
fn handle(event: &AssistantMessageEvent) -> &SharedAssistantMessage {
    match event {
        AssistantMessageEvent::Start { partial }
        | AssistantMessageEvent::TextStart { partial, .. }
        | AssistantMessageEvent::TextDelta { partial, .. }
        | AssistantMessageEvent::TextEnd { partial, .. }
        | AssistantMessageEvent::ThinkingStart { partial, .. }
        | AssistantMessageEvent::ThinkingDelta { partial, .. }
        | AssistantMessageEvent::ThinkingEnd { partial, .. }
        | AssistantMessageEvent::ToolcallStart { partial, .. }
        | AssistantMessageEvent::ToolcallDelta { partial, .. }
        | AssistantMessageEvent::ToolcallEnd { partial, .. } => partial,
        AssistantMessageEvent::Done { message, .. } => message,
        AssistantMessageEvent::Error { error, .. } => error,
    }
}
