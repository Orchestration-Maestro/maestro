//! Response-session framing and reduction witnesses.

use super::super::events::Source;
use futures_util::{FutureExt, stream};
use serde::Deserialize;
use serde_json::Value;
use std::fmt::Write as _;

/// Whole event sequences and their normalized retained output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventCase {
    /// Incoming records in order.
    events: Vec<Value>,
    /// Normalized sequence up to the terminal boundary.
    expected: Vec<Value>,
}

#[test]
fn maestro_response_sessions_normalize_terminal_events() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let rows: Vec<EventCase> =
            serde_json::from_str(include_str!("fixtures/events.json")).unwrap();
        for row in rows {
            let bytes: Vec<u8> = event_bytes(&row.events);
            let mut source = Source::new(Box::pin(stream::iter([Ok(bytes)])), None);
            let mut actual = Vec::new();
            while let Some(event) = source.next().await {
                actual.push(serde_json::from_str::<Value>(&event.unwrap()).unwrap());
            }
            assert_eq!(actual, row.expected);
        }
    });
}

/// One selected status and its normalized admission result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusCase {
    /// Original status, null also represents an absent selection.
    status: Value,
    /// Admitted spelling or omission.
    expected: Option<String>,
}

#[test]
fn maestro_response_sessions_normalize_terminal_status() {
    let rows: Vec<StatusCase> =
        serde_json::from_str(include_str!("fixtures/statuses.json")).unwrap();
    assert_eq!(super::super::events::normalize_codex_status(None), None);
    for row in rows {
        let text = row.status.to_string();
        let raw = crate::providers::json_text::raw_json(&text).unwrap();
        assert_eq!(
            super::super::events::normalize_codex_status(Some(raw)),
            row.expected
        );
    }
}

/// Event selection result with a typed source failure.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiCase {
    /// Incoming event records.
    events: Vec<Value>,
    /// Selected records on success.
    expected: Option<Vec<Value>>,
    /// Original API diagnostic on failure.
    error: Option<crate::DiagnosticErrorInfo>,
}

#[test]
fn maestro_response_sessions_preserve_api_error_identity() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let rows: Vec<ApiCase> =
            serde_json::from_str(include_str!("fixtures/api-errors.json")).unwrap();
        for row in rows {
            assert_api_case(row).await;
        }
    });
}

/// Encode complete event frames in their supplied order.
fn event_bytes(events: &[Value]) -> Vec<u8> {
    let mut bytes = String::new();
    for event in events {
        writeln!(bytes, "data: {event}\n").unwrap();
    }
    bytes.into_bytes()
}

/// Check the diagnostic view and the retained category of one selected sequence.
async fn assert_api_case(row: ApiCase) {
    let bytes = event_bytes(&row.events);
    let mut source = Source::new(Box::pin(stream::iter([Ok(bytes)])), None);
    let mut selected = Vec::new();
    while let Some(event) = source.next().await {
        if let Ok(event) = event {
            selected.push(serde_json::from_str::<Value>(&event).unwrap());
        }
    }
    if let Some(expected) = row.error {
        let Some(super::super::CodexError::Api(actual)) = source.failure else {
            panic!("missing API classification");
        };
        assert_eq!(actual, expected);
        assert!(row.expected.is_none());
    } else {
        assert!(source.failure.is_none());
        assert_eq!(selected, row.expected.unwrap());
    }
}

#[test]
fn maestro_response_sessions_run_http_lifecycle() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    runtime.block_on(async {
        let rows: Vec<LifecycleCase> =
            serde_json::from_str(include_str!("fixtures/lifecycle.json")).unwrap();
        for row in rows {
            assert_lifecycle(row).await;
        }
    });
}

#[test]
fn maestro_response_sessions_bound_setup_retries() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    runtime.block_on(async {
        for status in [400, 401, 408, 429, 500, 502, 503, 504, 505] {
            assert_retry_sequence(status).await;
        }
        assert_exhausted_retries().await;
    });
}

/// Close each scripted attempt before checking status-dependent retries and elapsed virtual time.
async fn assert_retry_sequence(status: u16) {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let (model, context, mut options) = super::invocation();
    let times = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::clone(&times);
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&hooks);
    options.common.on_response = Some(Arc::new(move |_, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }));
    options.common.max_retries = Some(0.0);
    options.common.timeout_ms = Some(1.0);
    options.common.max_retry_delay_ms = Some(1.0);
    options.common.fetch = Some(Arc::new(move |_| {
        let now = tokio::time::Instant::now();
        let count = {
            let mut times = calls.lock().unwrap();
            times.push(now);
            times.len()
        };
        Box::pin(async move {
            let bytes = if count == 1 {
                b"plain failure".to_vec()
            } else {
                b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_vec()
            };
            Ok(crate::HttpResponse {
                status: if count == 1 { status } else { 200 },
                status_text: String::new(),
                headers: std::collections::BTreeMap::default(),
                body: Box::pin(stream::iter([Ok(bytes)])),
            })
        })
    }));
    let prepared =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .unwrap();
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    let result =
        super::super::http::invoke_sse(&prepared, &model, &options, &output, &events).await;
    let retry = matches!(status, 429 | 500 | 502 | 503 | 504);
    let times = times.lock().unwrap();
    assert_eq!(times.len(), if retry { 2 } else { 1 }, "status {status}");
    assert_eq!(hooks.load(Ordering::SeqCst), times.len());
    if retry {
        assert_eq!(times[1] - times[0], std::time::Duration::from_millis(1000));
        assert!(result.is_ok());
    } else {
        assert_eq!(result.unwrap_err().diagnostic().message, "plain failure");
    }
}

/// Invoke a finite controlled success body, returning the same supplied output.
async fn finite_body(
    bytes: Vec<u8>,
) -> (
    Result<(), super::super::CodexError>,
    crate::SharedAssistantMessage,
) {
    use std::sync::{Arc, Mutex, RwLock};
    let (model, context, mut options) = super::invocation();
    let body = Arc::new(Mutex::new(Some(bytes)));
    options.common.fetch = Some(Arc::new(move |_| {
        let bytes = body.lock().unwrap().take().unwrap();
        Box::pin(async move {
            Ok(crate::HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::new(),
                body: Box::pin(stream::iter([Ok(bytes)])),
            })
        })
    }));
    let prepared =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .unwrap();
    let output = Arc::new(RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    let result =
        super::super::http::invoke_sse(&prepared, &model, &options, &output, &events).await;
    (result, output)
}

#[test]
fn maestro_response_sessions_require_terminal_events() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let partial = event_bytes(&[
            serde_json::json!({"type":"response.output_item.added","item":{"type":"message","content":[{"type":"output_text"}]}}),
            serde_json::json!({"type":"response.output_text.delta","delta":"retained partial"}),
        ]);
        for (bytes, retained) in [
            (Vec::new(), None),
            (b"data: {\"type\":\"response.created\",\"response\":{\"id\":\"r\",\"status\":\"in_progress\"}}\n\n".to_vec(), None),
            (b"data: [DONE]\n\n".to_vec(), None),
            (partial, Some("retained partial")),
        ] {
            let (result, output) = finite_body(bytes).await;
            assert_eq!(result.unwrap_err().diagnostic().message, "Response stream ended before a terminal event");
            if let Some(retained) = retained {
                assert_eq!(serde_json::to_value(&output.read().unwrap().content).unwrap()[0]["text"], retained);
            }
        }
        for (kind, status, reason) in [("response.completed", "completed", crate::StopReason::Stop),
            ("response.incomplete", "incomplete", crate::StopReason::Length)] {
            let (result, output) = finite_body(event_bytes(&[serde_json::json!({"type":kind,"response":{"status":status}})])).await;
            assert!(result.is_ok());
            assert_eq!(output.read().unwrap().stop_reason, reason);
        }
    });
}

/// Framed bytes and their complete selected records or native protocol failure.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameCase {
    /// Byte chunks, retaining fragmentation.
    chunks: Vec<Vec<u8>>,
    /// All selected records.
    expected: Option<Vec<Value>>,
    /// Native JSON failure, whose suffix is owned by the native parser.
    protocol_error: bool,
}

/// Compare recorded frame selection without reimplementing the provider's text policy.
async fn assert_frames(text: &str) {
    let rows: Vec<FrameCase> = serde_json::from_str(text).unwrap();
    for row in rows {
        let mut source = Source::new(Box::pin(stream::iter(row.chunks.into_iter().map(Ok))), None);
        let mut selected = Vec::new();
        while let Some(event) = source.next().await {
            if let Ok(event) = event {
                selected.push(serde_json::from_str::<Value>(&event).unwrap());
            }
        }
        if row.protocol_error {
            let Some(super::super::CodexError::Protocol(error)) = source.failure else {
                panic!("missing protocol classification");
            };
            assert_eq!(error.name.as_deref(), Some("CodexProtocolError"));
            assert!(error.message.starts_with("Invalid Codex SSE JSON: "));
            assert!(row.expected.is_none());
        } else {
            assert!(source.failure.is_none());
            assert_eq!(selected, row.expected.unwrap());
        }
    }
}

#[test]
fn maestro_response_sessions_frame_legal_line_endings() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(assert_frames(include_str!("fixtures/framing.json")));
}

#[test]
fn maestro_response_sessions_filter_sse_data() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(assert_frames(include_str!("fixtures/data.json")));
}

#[test]
fn maestro_response_sessions_keep_hook_and_abort_order() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (model, context, mut options) = super::invocation();
        let signal = crate::Cancellation::new();
        let abort = signal.clone();
        options.common.signal = Some(signal);
        options.common.on_response = Some(std::sync::Arc::new(move |_, _| {
            abort.abort(); Box::pin(async { Ok(()) })
        }));
        options.common.fetch = Some(std::sync::Arc::new(|_| Box::pin(async {
            Ok(crate::HttpResponse { status: 200, status_text: String::new(), headers: std::collections::BTreeMap::new(),
                body: Box::pin(stream::iter([Ok(b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_vec())])) })
        })));
        let prepared = super::super::request::prepare_request(&model, &context, &options, "maestro (browser)").await.unwrap();
        let output = std::sync::Arc::new(std::sync::RwLock::new(crate::providers::assistant_output::initial_message(&model)));
        let events = crate::AssistantMessageEventStream::new();
        let error = super::super::http::invoke_sse(&prepared, &model, &options, &output, &events).await.unwrap_err();
        assert_eq!(error.diagnostic().message, "Request was aborted");
        assert!(matches!(events.next().await, Some(crate::AssistantMessageEvent::Start { .. })));
    });
}

/// Exhaust the authored setup attempts on the controlled clock.
async fn assert_exhausted_retries() {
    use std::sync::{Arc, Mutex};
    let (model, context, mut options) = super::invocation();
    let times = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::clone(&times);
    options.common.fetch = Some(Arc::new(move |_| {
        let now = tokio::time::Instant::now();
        calls.lock().unwrap().push(now);
        Box::pin(async {
            Err(crate::FetchError::Connection(
                super::super::request::diagnostic("original final cause"),
            ))
        })
    }));
    let prepared =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .unwrap();
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    let error = super::super::http::invoke_sse(&prepared, &model, &options, &output, &events)
        .await
        .unwrap_err();
    assert_eq!(error.diagnostic().message, "original final cause");
    let times = times.lock().unwrap();
    assert_eq!(times.len(), 4);
    let elapsed: Vec<_> = times
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).as_millis())
        .collect();
    assert_eq!(elapsed, [1000, 2000, 4000]);
}

#[test]
fn maestro_response_sessions_reject_bodyless_success() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for status in [204, 205, 200] {
            assert_empty_success(status).await;
        }
    });
}

/// Distinguish absent success bodies from empty usable streams after the response hook.
async fn assert_empty_success(status: u16) {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (model, context, mut options) = super::invocation();
    let hooks = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&hooks);
    options.common.on_response = Some(Arc::new(move |response, _| {
        assert_eq!(response.status.to_bits(), f64::from(status).to_bits());
        observed.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }));
    options.common.fetch = Some(Arc::new(move |_| {
        Box::pin(async move {
            Ok(crate::HttpResponse {
                status,
                status_text: String::new(),
                headers: std::collections::BTreeMap::new(),
                body: Box::pin(stream::empty()),
            })
        })
    }));
    let prepared =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .unwrap();
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    let error = super::super::http::invoke_sse(&prepared, &model, &options, &output, &events)
        .await
        .unwrap_err();
    assert_eq!(hooks.load(Ordering::SeqCst), 1);
    if status == 200 {
        assert_eq!(
            error.diagnostic().message,
            "Response stream ended before a terminal event"
        );
        assert!(matches!(
            events.next().await,
            Some(crate::AssistantMessageEvent::Start { .. })
        ));
    } else {
        assert_eq!(error.diagnostic().message, "No response body");
    }
    assert!(
        events.next().now_or_never().is_none(),
        "settled invocation published an unexpected event"
    );
}

/// Producer that supplies one chunk and rejects any subsequent read.
struct TerminalBody {
    /// Complete terminal frame, followed by invalid queued data.
    chunk: Option<Vec<u8>>,
    /// Witness that the same source was released by invocation.
    dropped: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl futures_core::Stream for TerminalBody {
    type Item = Result<Vec<u8>, crate::FetchError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(Some(Ok(self
            .chunk
            .take()
            .expect("terminal must finish before a second body poll"))))
    }
}

impl Drop for TerminalBody {
    fn drop(&mut self) {
        self.dropped
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn maestro_response_sessions_finish_before_body_eof() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        for kind in ["response.done", "response.completed", "response.incomplete"] {
            for status in [
                serde_json::json!("completed"),
                serde_json::json!("incomplete"),
                serde_json::json!("failed"),
                serde_json::json!("cancelled"),
                serde_json::json!("queued"),
                serde_json::json!("in_progress"),
                serde_json::json!(null),
                serde_json::json!("unknown"),
                serde_json::json!(42),
            ] {
                assert_open_terminal(kind, status).await;
            }
        }
    });
}

/// Invoke each terminal through reduction without allowing the producer to return EOF.
async fn assert_open_terminal(kind: &str, status: Value) {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let (model, context, mut options) = super::invocation();
    let dropped = Arc::new(AtomicBool::new(false));
    let mut bytes = event_bytes(&[serde_json::json!({"type":kind,"response":{"status":status}})]);
    bytes.extend_from_slice(b"data: invalid JSON after terminal\n\n");
    let body = Arc::new(Mutex::new(Some(TerminalBody {
        chunk: Some(bytes),
        dropped: Arc::clone(&dropped),
    })));
    options.common.fetch = Some(Arc::new(move |_| {
        let body = body.lock().unwrap().take().unwrap();
        Box::pin(async move {
            Ok(crate::HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::new(),
                body: Box::pin(body),
            })
        })
    }));
    let prepared =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .unwrap();
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    super::super::http::invoke_sse(&prepared, &model, &options, &output, &events)
        .await
        .unwrap();
    assert!(
        dropped.load(Ordering::SeqCst),
        "invocation retained the same source after terminal reduction"
    );
    assert_eq!(
        output.read().unwrap().stop_reason,
        match status.as_str() {
            Some("incomplete") => crate::StopReason::Length,
            Some("failed" | "cancelled") => crate::StopReason::Error,
            _ => crate::StopReason::Stop,
        }
    );
}

/// Deliver one chunk, then acknowledge the next read before waiting for cancellation.
struct PausedBody {
    /// Initial partial text events.
    chunk: Option<Vec<u8>>,
    /// Producer guarantee that the first chunk has already been reduced.
    reading: Option<tokio::sync::oneshot::Sender<()>>,
    /// Same-body release witness.
    dropped: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl futures_core::Stream for PausedBody {
    type Item = Result<Vec<u8>, crate::FetchError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(bytes) = self.chunk.take() {
            return std::task::Poll::Ready(Some(Ok(bytes)));
        }
        if let Some(sender) = self.reading.take() {
            sender.send(()).unwrap();
        }
        std::task::Poll::Pending
    }
}

impl Drop for PausedBody {
    fn drop(&mut self) {
        self.dropped
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn maestro_response_sessions_cancel_body_reads() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        assert_cancelled_read().await;
        for error in [
            crate::FetchError::Connection(super::super::request::diagnostic(
                "body connection cause",
            )),
            crate::FetchError::Connection(crate::DiagnosticErrorInfo {
                name: Some("AbortError".to_owned()),
                ..super::super::request::diagnostic("source abort cause")
            }),
        ] {
            let expected = match &error {
                crate::FetchError::Connection(info) => info.clone(),
                _ => unreachable!(),
            };
            let (result, output, _) = invoke_body(Box::pin(stream::iter([Err(error)])), None).await;
            assert_eq!(result.unwrap_err().diagnostic(), &expected);
            assert_eq!(output.read().unwrap().stop_reason, crate::StopReason::Stop);
        }
    });
}

/// Abort only after the producer acknowledges a read after the retained text delta.
async fn assert_cancelled_read() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let signal = crate::Cancellation::new();
    let dropped = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let bytes = event_bytes(&[
        serde_json::json!({"type":"response.output_item.added","item":{"type":"message","content":[{"type":"output_text"}]}}),
        serde_json::json!({"type":"response.output_text.delta","delta":"before cancellation"}),
    ]);
    let body = PausedBody {
        chunk: Some(bytes),
        reading: Some(sender),
        dropped: Arc::clone(&dropped),
    };
    let abort = async {
        receiver.await.unwrap();
        signal.abort();
    };
    let (observed, ()) =
        futures_util::future::join(invoke_body(Box::pin(body), Some(signal.clone())), abort).await;
    let (result, output, _) = observed;
    assert_eq!(
        result.unwrap_err().diagnostic().message,
        "Request was aborted"
    );
    assert_eq!(
        serde_json::to_value(&output.read().unwrap().content).unwrap()[0]["text"],
        "before cancellation"
    );
    assert!(dropped.load(Ordering::SeqCst));
}

/// Invoke one controlled body with no setup replay permitted.
async fn invoke_body(
    body: crate::HttpBody,
    signal: Option<crate::Cancellation>,
) -> (
    Result<(), super::super::CodexError>,
    crate::SharedAssistantMessage,
    crate::AssistantMessageEventStream,
) {
    use std::sync::{Arc, Mutex, RwLock};
    let (model, context, mut options) = super::invocation();
    options.common.signal = signal;
    let slot = Arc::new(Mutex::new(Some(body)));
    options.common.fetch = Some(Arc::new(move |_| {
        let body = slot
            .lock()
            .unwrap()
            .take()
            .expect("body failure must not replay setup");
        Box::pin(async move {
            Ok(crate::HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::new(),
                body,
            })
        })
    }));
    let prepared =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .unwrap();
    let output = Arc::new(RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    let result =
        super::super::http::invoke_sse(&prepared, &model, &options, &output, &events).await;
    (result, output, events)
}

#[test]
fn maestro_response_sessions_release_retry_waits() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap();
    runtime.block_on(async {
        assert_cancelled_retry_wait().await;
        let signal = crate::Cancellation::new();
        let bytes = event_bytes(&[
            serde_json::json!({"type":"response.completed","response":{"status":"completed"}}),
        ]);
        let (result, output, events) =
            invoke_body(Box::pin(stream::iter([Ok(bytes)])), Some(signal.clone())).await;
        result.unwrap();
        signal.abort();
        assert_eq!(output.read().unwrap().stop_reason, crate::StopReason::Stop);
        assert!(matches!(
            events.next().await,
            Some(crate::AssistantMessageEvent::Start { .. })
        ));
        assert!(events.next().now_or_never().is_none());
    });
}

/// Explicit polling closes each setup decision before cancelling the second scoped wait.
async fn assert_cancelled_retry_wait() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (model, context, mut options) = super::invocation();
    let signal = crate::Cancellation::new();
    options.common.signal = Some(signal.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    options.common.fetch = Some(Arc::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Err(crate::FetchError::Connection(
                super::super::request::diagnostic("retry setup"),
            ))
        })
    }));
    let prepared =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .unwrap();
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    let future = super::super::http::invoke_sse(&prepared, &model, &options, &output, &events);
    futures_util::pin_mut!(future);
    assert!(future.as_mut().now_or_never().is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::advance(std::time::Duration::from_millis(1000)).await;
    assert!(future.as_mut().now_or_never().is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    signal.abort();
    let error = future.await.unwrap_err();
    assert_eq!(error.diagnostic().message, "Request was aborted");
    tokio::time::advance(std::time::Duration::from_secs(10)).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "settled producer must not create another attempt"
    );
    assert!(events.next().now_or_never().is_none());
}

#[test]
fn maestro_response_sessions_reduce_shared_content() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let bytes = event_bytes(&[
            serde_json::json!({"type":"response.output_item.added","item":{"type":"reasoning","summary":[]}}),
            serde_json::json!({"type":"response.reasoning_text.delta","delta":"provisional thinking"}),
            serde_json::json!({"type":"response.output_item.done","item":{"type":"reasoning","summary":[{"type":"summary_text","text":"final thinking"}]}}),
            serde_json::json!({"type":"response.output_item.added","item":{"type":"message","content":[{"type":"output_text"}]}}),
            serde_json::json!({"type":"response.output_text.delta","delta":"provisional text"}),
            serde_json::json!({"type":"response.output_item.done","item":{"type":"message","id":"msg_final","content":[{"type":"output_text","text":"final text"}]}}),
            serde_json::json!({"type":"response.output_item.added","item":{"type":"function_call","id":"fc_item","call_id":"call_one","name":"lookup","arguments":""}}),
            serde_json::json!({"type":"response.function_call_arguments.delta","delta":"{\"lookup\":17}"}),
            serde_json::json!({"type":"response.output_item.done","item":{"type":"function_call","arguments":"{\"lookup\":99}"}}),
            serde_json::json!({"type":"response.completed","response":{"id":"r_final","status":"completed","usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120,"input_tokens_details":{"cached_tokens":40}}}}),
        ]);
        let (result, output, events) = invoke_body(Box::pin(stream::iter([Ok(bytes)])), None).await;
        result.unwrap();
        assert_reduced_content(&output);
        let mut kinds = Vec::new();
        while let Some(Some(event)) = events.next().now_or_never() {
            let (kind, partial) = partial_event(&event);
            assert!(std::sync::Arc::ptr_eq(partial, &output));
            kinds.push(kind);
        }
        assert_eq!(kinds, ["start", "thinking_start", "thinking_delta", "thinking_end", "text_start", "text_delta", "text_end", "toolcall_start", "toolcall_delta", "toolcall_end"]);
    });
}

/// Inspect canonical output after provisional content was replaced and scratch discarded.
fn assert_reduced_content(output: &crate::SharedAssistantMessage) {
    let output = output.read().unwrap();
    let content = serde_json::to_value(&output.content).unwrap();
    assert_eq!(content[0]["thinking"], "final thinking");
    assert_eq!(content[1]["text"], "final text");
    assert_eq!(
        content[2],
        serde_json::json!({"type":"toolCall","id":"call_one|fc_item","name":"lookup","arguments":{"lookup":17}})
    );
    assert_eq!(output.response_id.as_deref(), Some("r_final"));
    assert_eq!(output.stop_reason, crate::StopReason::ToolUse);
    let usage = serde_json::to_value(&output.usage).unwrap();
    assert_eq!(
        usage,
        serde_json::json!({"input":60.0,"output":20.0,"cacheRead":40.0,"cacheWrite":0.0,"totalTokens":120.0,
        "cost":{"input":0.000_059_999_999_999_999_995,"output":0.000_039_999_999_999_999_996,"cacheRead":0.00012,"cacheWrite":0.0,"total":0.000_219_999_999_999_999_98}})
    );
}

/// Borrow each published content event's supplied partial handle.
fn partial_event(
    event: &crate::AssistantMessageEvent,
) -> (&'static str, &crate::SharedAssistantMessage) {
    use crate::AssistantMessageEvent as Event;
    match event {
        Event::Start { partial } => ("start", partial),
        Event::ThinkingStart { partial, .. } => ("thinking_start", partial),
        Event::ThinkingDelta { partial, .. } => ("thinking_delta", partial),
        Event::ThinkingEnd { partial, .. } => ("thinking_end", partial),
        Event::TextStart { partial, .. } => ("text_start", partial),
        Event::TextDelta { partial, .. } => ("text_delta", partial),
        Event::TextEnd { partial, .. } => ("text_end", partial),
        Event::ToolcallStart { partial, .. } => ("toolcall_start", partial),
        Event::ToolcallDelta { partial, .. } => ("toolcall_delta", partial),
        Event::ToolcallEnd { partial, .. } => ("toolcall_end", partial),
        Event::Done { .. } | Event::Error { .. } => {
            panic!("internal invocation published a final outcome")
        }
    }
}

#[test]
fn maestro_response_sessions_use_native_http() {
    use std::sync::Arc;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_sender, request_receiver) = tokio::sync::oneshot::channel();
    let (closed_sender, closed_receiver) = tokio::sync::oneshot::channel();
    let server = std::thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        let request = read_native_request(&mut peer);
        request_sender.send(request).unwrap();
        send_native_terminal(&mut peer);
        assert_native_body_closed(&mut peer);
        closed_sender.send(()).unwrap();
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (mut model, context, options) = super::invocation();
        Arc::make_mut(&mut model).base_url = format!("http://{address}/codex");
        let prepared =
            super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
                .await
                .unwrap();
        let output = Arc::new(std::sync::RwLock::new(
            crate::providers::assistant_output::initial_message(&model),
        ));
        let events = crate::AssistantMessageEventStream::new();
        super::super::http::invoke_sse(&prepared, &model, &options, &output, &events)
            .await
            .unwrap();
        let (headers, body) = request_receiver.await.unwrap();
        assert!(headers.starts_with("POST /codex/responses HTTP/1.1\r\n"));
        assert_eq!(
            body,
            crate::providers::json_text::compact_json(&prepared.body)
                .unwrap()
                .into_bytes()
        );
        for (name, value) in &prepared.headers {
            assert!(
                headers.contains(&format!("\r\n{name}: {value}\r\n")),
                "missing {name}"
            );
        }
        assert_eq!(
            serde_json::to_value(&output.read().unwrap().content).unwrap()[0]["text"],
            "native Ω🧭"
        );
        assert_eq!(output.read().unwrap().stop_reason, crate::StopReason::Stop);
        closed_receiver.await.unwrap();
    });
    server.join().unwrap();
}

/// Read one complete native HTTP request, preserving authored body bytes.
fn read_native_request(peer: &mut std::net::TcpStream) -> (String, Vec<u8>) {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    let boundary = loop {
        let mut chunk = [0; 1024];
        let count = peer.read(&mut chunk).unwrap();
        assert_ne!(count, 0, "request closed before headers");
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(position) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let headers = String::from_utf8(bytes[..boundary].to_vec()).unwrap();
    let length: usize = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap();
    while bytes.len() < boundary + length {
        let mut chunk = [0; 1024];
        let count = peer.read(&mut chunk).unwrap();
        assert_ne!(count, 0, "request closed before body");
        bytes.extend_from_slice(&chunk[..count]);
    }
    assert_eq!(bytes.len(), boundary + length);
    (headers, bytes[boundary..].to_vec())
}

/// Send split UTF-8 and CRLF event bytes without terminating the chunked HTTP body.
fn send_native_terminal(peer: &mut std::net::TcpStream) {
    use std::io::Write as _;
    peer.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
    let text = "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\"}]}}\r\n\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"native Ω🧭\"}\r\n\r\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\r\n\r\n";
    for &byte in text.as_bytes() {
        peer.write_all(&[b'1', b'\r', b'\n', byte, b'\r', b'\n'])
            .unwrap();
    }
    peer.flush().unwrap();
}

/// Observe client body cancellation while the server still has no HTTP EOF to send.
fn assert_native_body_closed(peer: &mut std::net::TcpStream) {
    use std::io::Read as _;
    let mut byte = [0];
    match peer.read(&mut byte) {
        Ok(0) => {}
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
        outcome => panic!("client did not close terminal body: {outcome:?}"),
    }
}

/// Selected full invocation and its observed internal outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleCase {
    /// Controlled setup/body branch.
    scenario: String,
    /// Settled producer attempt count.
    attempts: usize,
    /// Awaited response observations.
    hooks: usize,
    /// Native error, or success.
    error: Option<String>,
    /// Reducer stop reason on the supplied output.
    reason: String,
}

/// Owned observations shared with the controlled producers, not the invocation owner.
#[derive(Default)]
struct LifecycleObservations {
    /// Body bytes actually sent by each attempt.
    bodies: std::sync::Mutex<Vec<Vec<u8>>>,
    /// Finished response callbacks.
    hooks: std::sync::atomic::AtomicUsize,
    /// Finished payload callbacks.
    payloads: std::sync::atomic::AtomicUsize,
}

/// Exercise the reference setup/callback/body boundary rather than deferred outer outcomes.
async fn assert_lifecycle(row: LifecycleCase) {
    use std::sync::{Arc, atomic::Ordering};
    let (model, context, mut options) = super::invocation();
    let observed = Arc::new(LifecycleObservations::default());
    configure_lifecycle(&mut options, &row.scenario, &observed);
    let prepared =
        super::super::request::prepare_request(&model, &context, &options, "maestro (browser)")
            .await
            .unwrap();
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    let result =
        super::super::http::invoke_sse(&prepared, &model, &options, &output, &events).await;
    assert_eq!(
        result.err().map(|error| error.diagnostic().message.clone()),
        row.error,
        "{}",
        row.scenario
    );
    assert_eq!(observed.payloads.load(Ordering::SeqCst), 1);
    assert_eq!(observed.hooks.load(Ordering::SeqCst), row.hooks);
    assert_sent_bodies(&observed, &prepared.body, row.attempts);
    assert_eq!(
        serde_json::to_value(&output.read().unwrap().stop_reason).unwrap(),
        row.reason
    );
    let started = matches!(
        row.scenario.as_str(),
        "terminal" | "incomplete" | "no-terminal" | "malformed"
    );
    if started {
        assert!(matches!(
            events.next().await,
            Some(crate::AssistantMessageEvent::Start { .. })
        ));
    }
    assert!(
        events.next().now_or_never().is_none(),
        "internal operation published final outcome"
    );
}

/// Produce the selected response or setup failure, with body reads gated on the response hook.
fn lifecycle_response(
    scenario: &str,
    observed: std::sync::Arc<LifecycleObservations>,
) -> Result<crate::HttpResponse, crate::FetchError> {
    let message = match scenario {
        "network" => Some("connection lost"),
        "usage-network" => Some("usage limit exceeded"),
        "upper-usage-network" => Some("Usage limit exceeded"),
        "abort-network" => Some("Request was aborted"),
        _ => None,
    };
    if let Some(message) = message {
        return Err(crate::FetchError::Connection(
            super::super::request::diagnostic(message),
        ));
    }
    let status = match scenario {
        "400" | "usage400" | "body-read-fails" => 400,
        "429" | "usage429" => 429,
        _ => 200,
    };
    let bytes = match scenario {
        "400" => b"bad input".to_vec(),
        "429" => b"overloaded".to_vec(),
        "usage400" | "usage429" => {
            br#"{"error":{"code":"usage_limit_reached","message":"server message"}}"#.to_vec()
        }
        "no-terminal" => event_bytes(&[
            serde_json::json!({"type":"response.created","response":{"id":"created"}}),
        ]),
        "malformed" => b"data: {invalid\n\n".to_vec(),
        "incomplete" => event_bytes(&[
            serde_json::json!({"type":"response.incomplete","response":{"status":"incomplete"}}),
        ]),
        _ => event_bytes(&[
            serde_json::json!({"type":"response.completed","response":{"status":"completed"}}),
        ]),
    };
    let fail_read = scenario == "body-read-fails";
    let count = observed.bodies.lock().unwrap().len();
    let body = stream::once(async move {
        assert_eq!(
            observed.hooks.load(std::sync::atomic::Ordering::SeqCst),
            count,
            "body read preceded awaited response hook"
        );
        if fail_read {
            Err(crate::FetchError::Connection(
                super::super::request::diagnostic("body read failed"),
            ))
        } else {
            Ok(bytes)
        }
    });
    Ok(crate::HttpResponse {
        status,
        status_text: String::new(),
        headers: std::collections::BTreeMap::new(),
        body: Box::pin(body),
    })
}

/// Configure finite callbacks over owned observations without capturing their owner.
fn configure_lifecycle(
    options: &mut super::super::OpenAICodexResponsesOptions,
    scenario: &str,
    observed: &std::sync::Arc<LifecycleObservations>,
) {
    use std::sync::{Arc, atomic::Ordering};
    let payloads = Arc::clone(observed);
    options.common.on_payload = Some(Arc::new(move |mut body, _| {
        payloads.payloads.fetch_add(1, Ordering::SeqCst);
        body["marker"] = Value::from("replaced");
        Box::pin(async { Ok(body) })
    }));
    let hooks = Arc::clone(observed);
    let hook_scenario = scenario.to_owned();
    options.common.on_response = Some(Arc::new(move |_, _| {
        hooks.hooks.fetch_add(1, Ordering::SeqCst);
        let error = match hook_scenario.as_str() {
            "hook" => Some("hook failed"),
            "hook-usage" => Some("usage limit from hook"),
            _ => None,
        };
        Box::pin(async move {
            error.map_or(Ok(()), |message| {
                Err(super::super::request::diagnostic(message))
            })
        })
    }));
    let calls = Arc::clone(observed);
    let scenario = scenario.to_owned();
    options.common.fetch = Some(Arc::new(move |request| {
        calls.bodies.lock().unwrap().push(request.body);
        let scenario = scenario.clone();
        let calls = Arc::clone(&calls);
        Box::pin(async move { lifecycle_response(&scenario, calls) })
    }));
}

/// Compare each settled attempt's exact body against the one retained payload result.
fn assert_sent_bodies(observed: &LifecycleObservations, body: &Value, attempts: usize) {
    let expected = crate::providers::json_text::compact_json(body)
        .unwrap()
        .into_bytes();
    let bodies = observed.bodies.lock().unwrap();
    assert_eq!(bodies.len(), attempts);
    for body in &*bodies {
        assert_eq!(body, &expected);
    }
}
