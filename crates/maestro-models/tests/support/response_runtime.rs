//! Native execution witnesses for response invocations.
use crate::chat::{TestResult, context, model};
use maestro_models::providers::responses::openai_responses::{
    stream_openai_responses, stream_simple_openai_responses,
};
use maestro_models::{
    AssistantMessageEventStream, Context, Model, OpenAIResponsesOptions, SimpleStreamOptions,
    StreamOptions,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, PoisonError};

/// Descriptor and conversation shared by controlled runtime tests.
fn inputs() -> TestResult<(Model, Context)> {
    Ok((
        model(
            &json!({"api":"openai-responses","provider":"openai","cost":{"input":2,"output":7,"cacheRead":0.3,"cacheWrite":0.9}}),
        )?,
        context(&json!({"messages":[{"role":"user","content":"hello"}]}))?,
    ))
}
/// Invoke one of the two public entries.
fn start(
    simple: bool,
    model: Model,
    context: Context,
    common: StreamOptions,
) -> TestResult<AssistantMessageEventStream> {
    Ok(if simple {
        stream_simple_openai_responses(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..Default::default()
            }),
        )?
    } else {
        stream_openai_responses(
            model,
            context,
            Some(OpenAIResponsesOptions {
                common,
                ..Default::default()
            }),
        )
    })
}
/// Both entries use native HTTP with hooks, closed success, incomplete EOF and failures.
pub async fn native() -> TestResult {
    let success = format!(
        "{}{}",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[]}}\n\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[{\"type\":\"output_text\",\"text\":\"answer\"}]}}\n\n",
        crate::endpoint::COMPLETE
    );
    for simple in [false, true] {
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
            native_case(simple, head, body, reason, error).await?;
        }
    }
    Ok(())
}
/// Exercise one joined native exchange.
async fn native_case(
    simple: bool,
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
    let (mut target, history) = inputs()?;
    target.base_url = format!("{}/v1", server.url);
    let hooks = Arc::new(Mutex::new(Vec::new()));
    let common = native_options(&hooks);
    let stream = start(simple, target, history, common)?;
    let (events, result) = crate::endpoint::collect(&stream).await?;
    let received = server.finish()?;
    assert_eq!(received.request_line, "POST /v1/responses HTTP/1.1");
    assert_eq!(received.header("authorization"), Some("Bearer fixture-key"));
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
    if error.is_none() {
        assert_eq!(result["usage"]["cost"]["total"], 15.575);
        assert_eq!(result["content"][0]["text"], "answer");
    }
    Ok(())
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
async fn gated_call(simple: bool) -> TestResult {
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
    let (target, history) = inputs()?;
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
    let stream = start(simple, target, history, common)?;
    payload_entered.await?;
    let _ = payload_release.send(());
    response_entered.await?;
    let _ = response_release.send(());
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
    let (events, result) = crate::endpoint::collect(&stream).await?;
    closed.await?;
    assert_eq!(
        events.last(),
        Some(&json!({"type":"error","reason":"aborted"}))
    );
    assert_eq!(result["errorMessage"], "Request was aborted");
    assert_eq!(result["content"], before["content"]);
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
pub async fn gated() -> TestResult {
    for simple in [false, true] {
        gated_call(simple).await?;
    }
    Ok(())
}

/// Shared HTTP retry, timeout and cancellation options reach the existing sender.
pub async fn transport_options() -> TestResult {
    for simple in [false, true] {
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
        let (target, history) = inputs()?;
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
            crate::endpoint::collect(&start(simple, target, history, common)?).await?;
        assert_eq!(result["stopReason"], "stop");
        assert_eq!(setup.attempts(), 2);
        assert!(setup.gaps()[0] > std::time::Duration::from_millis(1));
        assert_eq!(response_hooks.load(std::sync::atomic::Ordering::SeqCst), 1);
        attempt_failure(
            simple,
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
            simple,
            crate::transport::Attempt::Hang,
            Some(1.0),
            "Request timed out.",
        )
        .await?;
        cancel_setup(simple).await?;
    }
    Ok(())
}
/// Zero retries and a caller timeout are forwarded unchanged.
async fn attempt_failure(
    simple: bool,
    attempt: crate::transport::Attempt,
    timeout_ms: Option<f64>,
    expected: &str,
) -> TestResult {
    let setup = crate::transport::transport(vec![attempt]);
    let (target, history) = inputs()?;
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(Arc::clone(&setup.fetch)),
        max_retries: Some(0.0),
        timeout_ms,
        ..Default::default()
    };
    let (_, result) = crate::endpoint::collect(&start(simple, target, history, common)?).await?;
    assert_eq!(result["errorMessage"], expected);
    assert_eq!(setup.attempts(), 1);
    Ok(())
}
/// Abort after the transport begins a pending setup, without advancing a timer.
async fn cancel_setup(simple: bool) -> TestResult {
    let signal = maestro_models::Cancellation::new();
    let cancel = signal.clone();
    let fetch: maestro_models::Fetch = Arc::new(move |_| {
        cancel.abort();
        Box::pin(std::future::pending())
    });
    let (target, history) = inputs()?;
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(fetch),
        signal: Some(signal),
        ..Default::default()
    };
    let (_, result) = crate::endpoint::collect(&start(simple, target, history, common)?).await?;
    assert_eq!(result["stopReason"], "aborted");
    assert_eq!(result["errorMessage"], "Request was aborted.");
    Ok(())
}
/// Failure to spawn without a native runtime still closes both entry streams.
pub fn without_runtime() -> TestResult {
    for simple in [false, true] {
        let (target, history) = inputs()?;
        let common = StreamOptions {
            api_key: Some("fixture-key".into()),
            ..Default::default()
        };
        let stream = start(simple, target, history, common)?;
        crate::chat::block_on(false, async {
            let (events, result) = crate::endpoint::collect(&stream).await?;
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

/// Fixture wrappers reject unread fields; each unique query has one existing test owner.
pub fn fixture_consumers() -> TestResult {
    let rows = crate::response_cases::rows(crate::endpoint::FIXTURE)?;
    let mut queries = std::collections::HashSet::new();
    let mut owners = std::collections::HashSet::new();
    let source = concat!(
        include_str!("../response_endpoint_requests.rs"),
        include_str!("../response_endpoint_streams.rs")
    );
    for row in &rows {
        let owner = row["test"].as_str().ok_or("test owner")?;
        assert!(
            source.contains(&format!("fn {owner}(")),
            "{owner} has a test"
        );
        owners.insert(owner);
        assert!(
            queries.insert(serde_json::to_string(&row["case"])?),
            "unique effective query: {}",
            row["case"]
        );
    }
    assert_eq!(owners.len(), 28);
    assert_eq!(rows.len(), 435);
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
            crate::response_cases::rows(&serde_json::to_string(&vec![invalid])?).is_err(),
            "{location} rejects unread fields"
        );
    }
    Ok(())
}
