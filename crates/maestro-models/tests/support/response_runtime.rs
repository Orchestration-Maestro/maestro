//! Controlled runtime witnesses shared by response endpoints.
use crate::chat::{TestResult, context, model};
use futures_util::FutureExt;
use maestro_models::providers::responses::azure_openai_responses::{
    stream_azure_openai_responses, stream_simple_azure_openai_responses,
};
use maestro_models::providers::responses::openai_responses::{
    stream_openai_responses, stream_simple_openai_responses,
};
use maestro_models::{
    AssistantMessageEventStream, AzureOpenAIResponsesOptions, Context, Model,
    OpenAIResponsesOptions, SimpleStreamOptions, StreamOptions,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, PoisonError};

/// Response endpoint under test.
#[allow(dead_code, reason = "Each test executable selects one endpoint.")]
#[derive(Clone, Copy)]
pub enum Endpoint {
    /// Standard response endpoint.
    Standard,
    /// Cloud response endpoint.
    Azure,
}
/// One endpoint and entry-point selection.
#[derive(Clone, Copy)]
struct Invocation {
    /// Endpoint-specific request policy.
    endpoint: Endpoint,
    /// Whether to use the simple entry point.
    simple: bool,
}
/// Descriptor and conversation shared by controlled runtime tests.
fn inputs(endpoint: Endpoint) -> TestResult<(Model, Context)> {
    let (api, provider) = match endpoint {
        Endpoint::Standard => ("openai-responses", "openai"),
        Endpoint::Azure => ("azure-openai-responses", "azure-openai-responses"),
    };
    Ok((
        model(
            &json!({"api":api,"provider":provider,"cost":{"input":2,"output":7,"cacheRead":0.3,"cacheWrite":0.9}}),
        )?,
        context(&json!({"messages":[{"role":"user","content":"hello"}]}))?,
    ))
}
/// Invoke one of the two public entries.
fn start(
    call: Invocation,
    model: Model,
    context: Context,
    common: StreamOptions,
) -> TestResult<AssistantMessageEventStream> {
    Ok(match (call.endpoint, call.simple) {
        (Endpoint::Standard, true) => stream_simple_openai_responses(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
        )?,
        (Endpoint::Standard, false) => stream_openai_responses(
            model,
            context,
            Some(OpenAIResponsesOptions {
                common,
                ..Default::default()
            }),
        ),
        (Endpoint::Azure, true) => stream_simple_azure_openai_responses(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
        )?,
        (Endpoint::Azure, false) => stream_azure_openai_responses(
            model,
            context,
            Some(AzureOpenAIResponsesOptions {
                common,
                ..Default::default()
            }),
        ),
    })
}
/// Both entries use native HTTP with hooks, closed success, incomplete EOF and failures.
pub async fn native(endpoint: Endpoint) -> TestResult {
    let success = format!(
        "{}{}",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[]}}\n\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[{\"type\":\"output_text\",\"text\":\"answer\"}]}}\n\n",
        crate::endpoint::COMPLETE
    );
    for simple in [false, true] {
        let call = Invocation { endpoint, simple };
        for (head, body, reason, error) in [
            (
                crate::loopback::EVENT_STREAM_HEAD,
                success.as_str(),
                "stop",
                None,
            ),
            (
                crate::loopback::EVENT_STREAM_HEAD,
                "data: {\"type\":\"response.created\",\"response\":{\"id\":\"created\"}}\n\n",
                "error",
                Some("Response stream ended before a terminal event"),
            ),
            (
                crate::loopback::EVENT_STREAM_HEAD,
                "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[]}}\n\ndata: {\"type\":\"response.content_part.added\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"draft\"}\n\n",
                "error",
                Some("Response stream ended before a terminal event"),
            ),
            (
                "HTTP/1.1 400 Bad Request\r\nconnection: close\r\n\r\n",
                "{\"error\":{\"message\":\"bad request\"}}",
                "error",
                Some("400 bad request"),
            ),
            (
                crate::loopback::EVENT_STREAM_HEAD,
                "data: {\"type\":\"error\",\"code\":\"bad\",\"message\":\"failure\"}\n\n",
                "error",
                Some("Error Code bad: failure"),
            ),
        ] {
            native_case(call, head, body, reason, error).await?;
        }
    }
    Ok(())
}
/// Exercise one joined native exchange.
async fn native_case(
    call: Invocation,
    head: &'static str,
    body: &str,
    reason: &str,
    error: Option<&str>,
) -> TestResult {
    let server = crate::loopback::serve(
        head,
        vec![body.as_bytes().to_vec()],
        std::time::Duration::ZERO,
    )?;
    let (mut target, history) = inputs(call.endpoint)?;
    target.base_url = match call.endpoint {
        Endpoint::Standard => format!("{}/v1", server.url),
        Endpoint::Azure => format!("{}/v1?token=a/#fragment", server.url),
    };
    let hooks = Arc::new(Mutex::new(Vec::new()));
    let common = native_options(&hooks);
    let stream = start(call, target, history, common)?;
    let (events, result) = crate::response_output::collect(&stream).await?;
    let received = server.finish()?;
    check_native_target(call.endpoint, &received);
    let captured: Value = serde_json::from_slice(&received.body)?;
    assert_eq!(captured["marker"], "sent");
    assert_eq!(
        captured["input"],
        json!([{"role":"user","content":[{"type":"input_text","text":"hello"}]}])
    );
    assert_eq!(result["stopReason"], reason);
    assert_eq!(result["errorMessage"].as_str(), error);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e["type"].as_str(), Some("done" | "error")))
            .count(),
        1
    );
    assert_eq!(
        *hooks.lock().map_err(|e| e.to_string())?,
        if head.contains("400") {
            vec!["payload"]
        } else {
            vec!["payload", "response"]
        }
    );
    check_native_contents(call.endpoint, &result, body, error);
    Ok(())
}
/// Inspect retained text and model-priced cost for the closed native response.
fn check_native_contents(endpoint: Endpoint, result: &Value, body: &str, error: Option<&str>) {
    if body.contains("draft") {
        assert_eq!(result["content"][0]["text"], "draft");
    }
    if error.is_none() {
        let cost = match endpoint {
            Endpoint::Standard => 15.575,
            Endpoint::Azure => 0.002_219_999_999_999_999_8,
        };
        assert_eq!(result["usage"]["cost"]["total"], cost);
        assert_eq!(result["content"][0]["text"], "answer");
    }
}
/// Assert endpoint-specific native path and credential headers.
fn check_native_target(endpoint: Endpoint, received: &crate::loopback::Received) {
    let (path, header, key) = match endpoint {
        Endpoint::Standard => (
            "POST /v1/responses HTTP/1.1",
            "authorization",
            "Bearer fixture-key",
        ),
        Endpoint::Azure => (
            "POST /v1/responses?token=a/&api-version=v1 HTTP/1.1",
            "api-key",
            "fixture-key",
        ),
    };
    assert_eq!(received.request_line, path);
    assert_eq!(received.header(header), Some(key));
}
/// Record native hook effects without holding a lock around serialization or user code.
fn native_options(hooks: &Arc<Mutex<Vec<&'static str>>>) -> StreamOptions {
    let payload = Arc::clone(hooks);
    let response = Arc::clone(hooks);
    StreamOptions {
        api_key: Some("fixture-key".into()),
        max_retries: Some(0.0),
        on_payload: Some(Arc::new(move |mut body, _| {
            payload
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push("payload");
            body["marker"] = json!("sent");
            Box::pin(std::future::ready(Ok(body)))
        })),
        on_response: Some(Arc::new(move |headers, _| {
            assert!((headers.status - 200.0).abs() < f64::EPSILON);
            response
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push("response");
            Box::pin(std::future::ready(Ok(())))
        })),
        ..Default::default()
    }
}

/// A producer completion witness sent when its pending read is dropped.
struct Closed(Option<tokio::sync::oneshot::Sender<()>>);
impl Drop for Closed {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}
/// A hook controlled by entry and completion acknowledgements.
fn gate_hook() -> (
    maestro_models::OnPayload,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
    Arc<std::sync::atomic::AtomicBool>,
) {
    let (entered_tx, entered) = tokio::sync::oneshot::channel();
    let (release, release_rx) = tokio::sync::oneshot::channel();
    let state = Arc::new(Mutex::new(Some((entered_tx, release_rx))));
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let finished = Arc::clone(&done);
    let hook: maestro_models::OnPayload = Arc::new(move |payload, _| {
        let gates = state.lock().unwrap_or_else(PoisonError::into_inner).take();
        let finished = Arc::clone(&finished);
        Box::pin(async move {
            let (entered, release) = gates.ok_or_else(|| {
                maestro_models::extract_diagnostic_error(maestro_models::DiagnosticInput::Text(
                    "hook invoked twice",
                ))
            })?;
            let _ = entered.send(());
            let _ = release.await;
            finished.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(payload)
        })
    });
    (hook, entered, release, done)
}
/// Body creation and polling check the hook's completed effect, not timing-based absence.
fn pending_body(
    done: Arc<std::sync::atomic::AtomicBool>,
    waiting: tokio::sync::oneshot::Sender<()>,
    closed: tokio::sync::oneshot::Sender<()>,
) -> maestro_models::HttpBody {
    let bytes = b"data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[]}}\n\ndata: {\"type\":\"response.content_part.added\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\n\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"retained\"}\n\n".to_vec();
    Box::pin(futures_util::stream::unfold(
        (Some(bytes), Some(waiting), Closed(Some(closed))),
        move |(bytes, mut waiting, closed)| {
            let done = Arc::clone(&done);
            async move {
                assert!(done.load(std::sync::atomic::Ordering::SeqCst));
                if let Some(bytes) = bytes {
                    return Some((Ok(bytes), (None, waiting, closed)));
                }
                std::future::poll_fn(|_| {
                    let _ = waiting.take().map(|sender| sender.send(()));
                    std::task::Poll::<()>::Pending
                })
                .await;
                None
            }
        },
    ))
}
/// Gate one operation, then abort a witnessed pending read after observing its partial.
async fn gated_call(call: Invocation) -> TestResult {
    let (payload_hook, payload_entered, payload_release, payload_done) = gate_hook();
    let (response_gate, response_entered, response_release, response_done) = gate_hook();
    let (waiting_tx, waiting) = tokio::sync::oneshot::channel();
    let (closed_tx, closed) = tokio::sync::oneshot::channel();
    let body = Arc::new(Mutex::new(Some(pending_body(
        response_done,
        waiting_tx,
        closed_tx,
    ))));
    let fetch = gated_fetch(body, payload_done);
    let signal = maestro_models::Cancellation::new();
    let (target, history) = inputs(call.endpoint)?;
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(fetch),
        signal: Some(signal.clone()),
        on_payload: Some(payload_hook),
        on_response: Some(Arc::new(move |_, model| {
            let gate = Arc::clone(&response_gate);
            Box::pin(async move { gate(Value::Null, model).await.map(|_| ()) })
        })),
        ..Default::default()
    };
    let stream = start(call, target, history, common)?;
    payload_entered.await?;
    let _ = payload_release.send(());
    response_entered.await?;
    assert!(
        stream.next().now_or_never().is_none(),
        "response gate prevents Start"
    );
    let _ = response_release.send(());
    cancel_partial(&stream, signal, waiting, closed).await
}
/// Abort the same operation after its body acknowledges publication of partial text.
async fn cancel_partial(
    stream: &AssistantMessageEventStream,
    signal: maestro_models::Cancellation,
    waiting: tokio::sync::oneshot::Receiver<()>,
    closed: tokio::sync::oneshot::Receiver<()>,
) -> TestResult {
    let partial = loop {
        if let maestro_models::AssistantMessageEvent::TextDelta { partial, .. } =
            stream.next().await.ok_or("text delta before stream end")?
        {
            break partial;
        }
    };
    let snapshot = partial.read().map_err(|e| e.to_string())?.clone();
    let before = serde_json::to_value(&snapshot)?;
    assert_eq!(before["content"][0]["text"], "retained");
    waiting.await?;
    signal.abort();
    let (events, result) = crate::response_output::collect(stream).await?;
    closed.await?;
    assert_eq!(
        events.last(),
        Some(&json!({"type":"error","reason":"aborted"}))
    );
    assert_eq!(result["errorMessage"], "Request was aborted");
    assert_eq!(result["content"], before["content"]);
    assert_eq!(
        crate::json::canonical(result["usage"].clone()),
        json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}})
    );
    let final_alias = stream.result().await;
    assert!(Arc::ptr_eq(&partial, &final_alias));
    assert_eq!(
        partial.read().map_err(|e| e.to_string())?.stop_reason,
        maestro_models::StopReason::Aborted
    );
    Ok(())
}
/// Fetch begins only after the payload hook's completed effect.
fn gated_fetch(
    body: Arc<Mutex<Option<maestro_models::HttpBody>>>,
    payload_done: Arc<std::sync::atomic::AtomicBool>,
) -> maestro_models::Fetch {
    Arc::new(move |_| {
        assert!(payload_done.load(std::sync::atomic::Ordering::SeqCst));
        let body = body.lock().unwrap_or_else(PoisonError::into_inner).take();
        Box::pin(std::future::ready(Ok(maestro_models::HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: std::collections::BTreeMap::default(),
            body: body.unwrap_or_else(|| Box::pin(futures_util::stream::empty())),
        })))
    })
}
/// Both entry points preserve effect order and the pending body's partial output.
pub async fn gated(endpoint: Endpoint) -> TestResult {
    for simple in [false, true] {
        let call = Invocation { endpoint, simple };
        gated_call(call).await?;
    }
    Ok(())
}

/// Shared HTTP retry, timeout and cancellation options reach the existing sender.
pub async fn transport_options(endpoint: Endpoint) -> TestResult {
    for simple in [false, true] {
        let call = Invocation { endpoint, simple };
        let response_hooks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = Arc::clone(&response_hooks);
        let setup = crate::transport::transport(vec![
            crate::transport::Attempt::Fail(maestro_models::FetchError::Connection(
                maestro_models::extract_diagnostic_error(maestro_models::DiagnosticInput::Text(
                    "disconnected",
                )),
            )),
            crate::transport::Attempt::body(
                200,
                &[],
                crate::endpoint::COMPLETE.as_bytes().to_vec(),
            ),
        ]);
        let (target, history) = inputs(call.endpoint)?;
        let common = StreamOptions {
            api_key: Some("fixture-key".into()),
            fetch: Some(Arc::clone(&setup.fetch)),
            max_retries: Some(1.0),
            max_retry_delay_ms: Some(1.0),
            on_response: Some(Arc::new(move |_, _| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Box::pin(std::future::ready(Ok(())))
            })),
            ..Default::default()
        };
        let (_, result) =
            crate::response_output::collect(&start(call, target, history, common)?).await?;
        assert_eq!(result["stopReason"], "stop");
        assert_eq!(setup.attempts(), 2);
        assert!(setup.gaps()[0] > std::time::Duration::from_millis(1));
        assert_eq!(response_hooks.load(std::sync::atomic::Ordering::SeqCst), 1);
        attempt_failure(
            call,
            crate::transport::Attempt::Fail(maestro_models::FetchError::Connection(
                maestro_models::extract_diagnostic_error(maestro_models::DiagnosticInput::Text(
                    "no retry",
                )),
            )),
            None,
            "Connection error.",
        )
        .await?;
        attempt_failure(
            call,
            crate::transport::Attempt::Hang,
            Some(1.0),
            "Request timed out.",
        )
        .await?;
        cancel_setup(call).await?;
        retry_statuses(call).await?;
        inherited_timeout(call).await?;
    }
    Ok(())
}
/// Zero retries and a caller timeout are forwarded unchanged.
async fn attempt_failure(
    call: Invocation,
    attempt: crate::transport::Attempt,
    timeout_ms: Option<f64>,
    expected: &str,
) -> TestResult {
    let setup = crate::transport::transport(vec![attempt]);
    let (target, history) = inputs(call.endpoint)?;
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(Arc::clone(&setup.fetch)),
        max_retries: Some(0.0),
        timeout_ms,
        ..Default::default()
    };
    let (_, result) =
        crate::response_output::collect(&start(call, target, history, common)?).await?;
    assert_eq!(result["errorMessage"], expected);
    assert_eq!(setup.attempts(), 1);
    Ok(())
}
/// Abort after the transport begins a pending setup, without advancing a timer.
async fn cancel_setup(call: Invocation) -> TestResult {
    let signal = maestro_models::Cancellation::new();
    let cancel = signal.clone();
    let fetch: maestro_models::Fetch = Arc::new(move |_| {
        cancel.abort();
        Box::pin(std::future::pending())
    });
    let (target, history) = inputs(call.endpoint)?;
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(fetch),
        signal: Some(signal),
        ..Default::default()
    };
    let (_, result) =
        crate::response_output::collect(&start(call, target, history, common)?).await?;
    assert_eq!(result["stopReason"], "aborted");
    assert_eq!(result["errorMessage"], "Request was aborted.");
    Ok(())
}
/// Failure to spawn without a native runtime still closes both entry streams.
pub fn without_runtime(endpoint: Endpoint) -> TestResult {
    for simple in [false, true] {
        let call = Invocation { endpoint, simple };
        let (target, history) = inputs(call.endpoint)?;
        let common = StreamOptions {
            api_key: Some("fixture-key".into()),
            ..Default::default()
        };
        let stream = start(call, target, history, common)?;
        crate::chat::block_on(false, async {
            let (events, result) = crate::response_output::collect(&stream).await?;
            assert_eq!(events, vec![json!({"type":"error","reason":"error"})]);
            assert_eq!(
                result["errorMessage"],
                "Streaming requires a running Tokio runtime."
            );
            Ok(())
        })?;
    }
    Ok(())
}

/// HTTP status retries retain the retry hint while ignoring the delay-cap preference.
async fn retry_statuses(call: Invocation) -> TestResult {
    for retries in [Some(0_u8), Some(1), None] {
        let setup = crate::transport::transport(vec![
            crate::transport::Attempt::status(503, &[("retry-after-ms", "2")]),
            crate::transport::Attempt::body(
                200,
                &[],
                crate::endpoint::COMPLETE.as_bytes().to_vec(),
            ),
        ]);
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&count);
        let (target, history) = inputs(call.endpoint)?;
        let common = StreamOptions {
            api_key: Some("fixture-key".into()),
            fetch: Some(Arc::clone(&setup.fetch)),
            max_retries: retries.map(f64::from),
            max_retry_delay_ms: Some(1.0),
            on_response: Some(Arc::new(move |_, _| {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Box::pin(std::future::ready(Ok(())))
            })),
            ..Default::default()
        };
        let (_, result) =
            crate::response_output::collect(&start(call, target, history, common)?).await?;
        let retried = retries != Some(0);
        assert_eq!(setup.attempts(), if retried { 2 } else { 1 });
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            usize::from(retried)
        );
        assert_eq!(result["stopReason"], if retried { "stop" } else { "error" });
        if retried {
            assert_eq!(setup.gaps(), vec![std::time::Duration::from_millis(2)]);
        }
    }
    let setup = crate::transport::transport(vec![
        crate::transport::Attempt::status(503, &[("retry-after-ms", "2")]),
        crate::transport::Attempt::status(503, &[("retry-after-ms", "2")]),
        crate::transport::Attempt::body(200, &[], crate::endpoint::COMPLETE.as_bytes().to_vec()),
    ]);
    let (target, history) = inputs(call.endpoint)?;
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(Arc::clone(&setup.fetch)),
        ..Default::default()
    };
    let (_, result) =
        crate::response_output::collect(&start(call, target, history, common)?).await?;
    assert_eq!(setup.attempts(), 3);
    assert_eq!(result["stopReason"], "stop");
    Ok(())
}
/// The inherited timeout remains ten minutes on a controlled paused clock.
async fn inherited_timeout(call: Invocation) -> TestResult {
    let setup = crate::transport::transport(vec![crate::transport::Attempt::Hang]);
    let (target, history) = inputs(call.endpoint)?;
    let began = tokio::time::Instant::now();
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(Arc::clone(&setup.fetch)),
        max_retries: Some(0.0),
        ..Default::default()
    };
    let (_, result) =
        crate::response_output::collect(&start(call, target, history, common)?).await?;
    assert_eq!(result["errorMessage"], "Request timed out.");
    assert_eq!(began.elapsed(), std::time::Duration::from_secs(600));
    Ok(())
}

/// Fixture wrappers reject unread fields; each unique query has one existing test owner.
pub fn fixture_consumers(
    fixture: &str,
    decode: fn(&str) -> TestResult<Vec<Value>>,
    source: &str,
    expected: (usize, usize),
    query_key: fn(&Value) -> TestResult<String>,
) -> TestResult {
    let rows = decode(fixture)?;
    let mut queries = std::collections::HashSet::new();
    let mut owners = std::collections::HashSet::new();
    for row in &rows {
        let owner = row["test"].as_str().ok_or("test owner")?;
        assert!(
            source.contains(&format!("fn {owner}(")),
            "{owner} has a test"
        );
        owners.insert(owner);
        assert!(
            queries.insert(query_key(&row["case"])?),
            "unique effective query: {}",
            row["case"]
        );
    }
    assert_eq!(owners.len(), expected.0);
    assert_eq!(rows.len(), expected.1);
    for location in ["wrapper", "input", "option", "expected"] {
        let mut invalid = rows[0].clone();
        let object = match location {
            "wrapper" => &mut invalid,
            "input" => &mut invalid["case"],
            "option" => {
                invalid["case"]["options"] = json!({});
                &mut invalid["case"]["options"]
            }
            _ => &mut invalid["expected"],
        };
        object
            .as_object_mut()
            .ok_or("fixture object")?
            .insert("unread".into(), json!(1));
        assert!(
            decode(&serde_json::to_string(&vec![invalid])?).is_err(),
            "{location} rejects unread fields"
        );
    }
    Ok(())
}
