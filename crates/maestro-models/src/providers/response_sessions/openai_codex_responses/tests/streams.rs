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
        .build()
        .unwrap();
    runtime.block_on(async {
        let (model, context, mut options) = super::invocation();
        options.common.fetch = Some(std::sync::Arc::new(|request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["marker"], "retained");
            Box::pin(async {
                Ok(crate::HttpResponse { status: 200, status_text: String::new(), headers: std::collections::BTreeMap::default(),
                    body: Box::pin(stream::iter([Ok(b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_vec())])) })
            })
        }));
        options.common.on_payload = Some(std::sync::Arc::new(|mut payload, _| Box::pin(async move {
            payload["marker"] = Value::from("retained"); Ok(payload)
        })));
        let prepared = super::super::request::prepare_request(&model, &context, &options, "maestro (browser)").await.unwrap();
        let output = std::sync::Arc::new(std::sync::RwLock::new(crate::providers::assistant_output::initial_message(&model)));
        let events = crate::AssistantMessageEventStream::new();
        super::super::http::invoke_sse(&prepared, &model, &options, &output, &events).await.unwrap();
        assert!(matches!(events.next().await, Some(crate::AssistantMessageEvent::Start { partial }) if std::sync::Arc::ptr_eq(&partial, &output)));
        assert_eq!(output.read().unwrap().stop_reason, crate::StopReason::Stop);
        assert!(events.next().now_or_never().is_none(), "internal invocation must leave final publication to its caller");
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
