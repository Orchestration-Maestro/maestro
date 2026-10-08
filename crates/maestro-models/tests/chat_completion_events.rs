//! Stream reduction for direct chat-completion invocations.

#[path = "support/cases.rs"]
mod cases;
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the loopback server."
)]
#[path = "support/loopback.rs"]
mod loopback;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the scripted transport."
)]
#[path = "support/transport.rs"]
mod transport;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use cases::assert_rows;
use chat::{TestResult, block_on};
use futures_util::StreamExt;
use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    Cancellation, DiagnosticErrorInfo, FetchError, HttpBody, OpenAICompletionsOptions,
    SharedAssistantMessage, StopReason, StreamOptions, stream_openai_completions,
};
use serde_json::{Value, json};
use transport::{Attempt, call_inputs, drain, transport};

const FIXTURE: &str = include_str!("fixtures/chat_completions/events.json");

/// Run the rows a test owns.
fn rows_of(test: &str) -> TestResult {
    block_on(false, assert_rows(FIXTURE, test))
}

#[test]
fn maestro_chat_frames_fragmented_events() -> TestResult {
    rows_of("maestro_chat_frames_fragmented_events")
}

/// Chunks whose fields have the wrong type, followed by one valid text chunk.
async fn wrong_typed_fields_read_as_absent() -> TestResult {
    let valid = json!({"id": "r", "model": "model",
        "choices": [{"index": 0, "delta": {"content": "ok"}, "finish_reason": "stop"}]});
    let case = json!({"chunks": [
        {"id": 5, "model": 7, "usage": "x", "choices": "x"},
        {"choices": [7]},
        {"choices": [{"delta": 5, "finish_reason": 5, "usage": []}]},
        {"choices": [{"delta": {"content": 5, "reasoning": [], "tool_calls": {},
            "reasoning_details": "x"}}]},
        valid
    ]});
    let observed = cases::run_case(&case).await?;
    assert_eq!(
        observed.result["content"],
        json!([{"type": "text", "text": "ok"}])
    );
    assert_eq!(observed.result["responseId"], "r");
    assert_eq!(observed.result["stopReason"], "stop");
    assert_eq!(
        cases::canonical(observed.result["usage"].clone())["totalTokens"],
        0
    );
    Ok(())
}

#[test]
fn maestro_chat_ignores_noncontent_chunks() -> TestResult {
    block_on(false, async {
        assert_rows(FIXTURE, "maestro_chat_ignores_noncontent_chunks").await?;
        wrong_typed_fields_read_as_absent().await
    })
}

#[test]
fn maestro_chat_requires_a_running_runtime() -> TestResult {
    let options = OpenAICompletionsOptions {
        common: StreamOptions {
            api_key: Some("fixture-key".into()),
            ..StreamOptions::default()
        },
        ..OpenAICompletionsOptions::default()
    };
    let history = chat::context(&json!({"messages": [{"role": "user", "content": "hello"}]}))?;
    let stream = stream_openai_completions(chat::model(&json!({}))?, history, Some(options));
    let (labels, message) = block_on(false, async { drain(&stream).await })?;
    assert_eq!(labels, ["error"]);
    assert_eq!(
        message.error_message.as_deref(),
        Some("Streaming requires a running Tokio runtime.")
    );
    assert!(matches!(message.stop_reason, StopReason::Error));
    Ok(())
}

#[test]
fn maestro_chat_retains_invocation_identity() -> TestResult {
    rows_of("maestro_chat_retains_invocation_identity")
}

#[test]
fn maestro_chat_keeps_parallel_tool_deltas_independent() -> TestResult {
    rows_of("maestro_chat_keeps_parallel_tool_deltas_independent")
}

#[test]
fn maestro_chat_accounts_for_usage_locations() -> TestResult {
    rows_of("maestro_chat_accounts_for_usage_locations")
}

#[test]
fn maestro_chat_preserves_stream_outcomes() -> TestResult {
    rows_of("maestro_chat_preserves_stream_outcomes")
}

#[test]
fn maestro_chat_resolves_late_tool_fields() -> TestResult {
    rows_of("maestro_chat_resolves_late_tool_fields")
}

#[test]
fn maestro_chat_retains_thought_signatures() -> TestResult {
    rows_of("maestro_chat_retains_thought_signatures")
}

#[test]
fn maestro_chat_finalizes_partial_arguments() -> TestResult {
    rows_of("maestro_chat_finalizes_partial_arguments")
}

/// An event stream chunk carrying text.
fn text_chunk(text: &str) -> Vec<u8> {
    let chunk = json!({"id": "r", "model": "model",
        "choices": [{"index": 0, "delta": {"content": text}}]});
    format!("data: {chunk}\n\n").into_bytes()
}

/// An attempt that answers with `chunks` and then the given tail of the body.
fn chunks_then(
    chunks: Vec<Vec<u8>>,
    tail: impl Fn() -> HttpBody + Send + Sync + 'static,
) -> Attempt {
    Attempt::streaming(move || {
        let head = futures_util::stream::iter(chunks.clone().into_iter().map(Ok));
        Box::pin(head.chain(tail()))
    })
}

/// Start a call over `attempt`, cancellable through `signal`.
fn start(
    attempt: Attempt,
    signal: &Cancellation,
) -> TestResult<(AssistantMessageEventStream, transport::Transport)> {
    let target = transport(vec![attempt]);
    let (model, context, mut common) = call_inputs(&target)?;
    common.signal = Some(signal.clone());
    common.max_retries = Some(0.0);
    let options = OpenAICompletionsOptions {
        common,
        ..OpenAICompletionsOptions::default()
    };
    Ok((
        stream_openai_completions(model, context, Some(options)),
        target,
    ))
}

/// Check the text of a message that kept exactly one text block.
fn assert_only_text(message: &AssistantMessage, expected: &str) {
    assert!(
        matches!(&message.content[..], [AssistantContent::Text(text)] if text.text == expected),
        "{:?}",
        message.content
    );
}

/// A body that delivers `text` and then stalls until the signal aborts.
async fn abort_while_the_body_is_pending() -> TestResult {
    let signal = Cancellation::new();
    let stalled = chunks_then(vec![text_chunk("partial")], || {
        Box::pin(futures_util::stream::pending())
    });
    let (stream, _) = start(stalled, &signal)?;
    let trigger = signal.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        trigger.abort();
    });
    let (labels, message) = drain(&stream).await?;
    assert_eq!(
        labels,
        ["start", "text_start", "text_delta", "text_end", "error"]
    );
    assert!(matches!(message.stop_reason, StopReason::Aborted));
    assert_eq!(
        message.error_message.as_deref(),
        Some("Request was aborted")
    );
    assert_only_text(&message, "partial");
    Ok(())
}

/// A body that aborts the signal at the moment it ends.
async fn abort_as_the_body_ends() -> TestResult {
    let signal = Cancellation::new();
    let trigger = signal.clone();
    let ends_aborted = chunks_then(vec![text_chunk("whole")], move || {
        let trigger = trigger.clone();
        Box::pin(futures_util::stream::poll_fn(move |_| {
            trigger.abort();
            Poll::Ready(None)
        }))
    });
    let (stream, _) = start(ends_aborted, &signal)?;
    let (labels, message) = drain(&stream).await?;
    assert_eq!(
        labels,
        ["start", "text_start", "text_delta", "text_end", "error"]
    );
    assert!(matches!(message.stop_reason, StopReason::Aborted));
    assert_eq!(
        message.error_message.as_deref(),
        Some("Request was aborted")
    );
    assert_only_text(&message, "whole");
    Ok(())
}

/// Dropping the reader leaves the request running; its result still arrives.
async fn dropped_reader_keeps_the_request_running() -> TestResult {
    let (stream, target) = start(
        Attempt::slow_body(Duration::from_secs(30)),
        &Cancellation::new(),
    )?;
    let result = stream.result();
    drop(stream);
    let message = result.await;
    let message = message.read().map_err(|error| error.to_string())?;
    assert!(matches!(message.stop_reason, StopReason::Stop));
    assert_eq!(target.attempts(), 1);
    Ok(())
}

/// A body that reports an abort of its own while the caller's signal stays unset.
async fn body_aborts_by_itself() -> TestResult {
    let aborted = Attempt::streaming(|| {
        Box::pin(futures_util::stream::iter([
            Ok(text_chunk("so far")),
            Err(FetchError::Aborted),
        ]))
    });
    let (stream, _) = start(aborted, &Cancellation::new())?;
    let (labels, message) = drain(&stream).await?;
    assert_eq!(
        labels,
        ["start", "text_start", "text_delta", "text_end", "done"]
    );
    assert!(matches!(message.stop_reason, StopReason::Stop));
    assert_only_text(&message, "so far");
    Ok(())
}

#[test]
fn maestro_chat_cancels_owned_request_work() -> TestResult {
    block_on(true, async {
        assert_rows(FIXTURE, "maestro_chat_cancels_owned_request_work").await?;
        abort_while_the_body_is_pending().await?;
        abort_as_the_body_ends().await?;
        body_aborts_by_itself().await?;
        dropped_reader_keeps_the_request_running().await
    })
}

/// A body whose read fails after `text` was delivered.
async fn body_fails_midstream() -> TestResult {
    let cut = Attempt::streaming(|| {
        let failure = DiagnosticErrorInfo {
            name: None,
            message: "socket closed".into(),
            stack: None,
            code: None,
        };
        Box::pin(futures_util::stream::iter([
            Ok(text_chunk("so far")),
            Err(FetchError::Connection(failure)),
        ]))
    });
    let (stream, _) = start(cut, &Cancellation::new())?;
    let (labels, message) = drain(&stream).await?;
    assert_eq!(labels, ["start", "text_start", "text_delta", "error"]);
    assert_eq!(message.error_message.as_deref(), Some("socket closed"));
    assert!(matches!(message.stop_reason, StopReason::Error));
    assert_only_text(&message, "so far");
    Ok(())
}

/// A body that is not valid text ends the stream with a decoding failure.
async fn body_is_not_text() -> TestResult {
    let garbled = Attempt::streaming(|| {
        Box::pin(futures_util::stream::iter([
            Ok(text_chunk("so far")),
            Ok(vec![0xff, 0xfe, b'\n', b'\n']),
        ]))
    });
    let (stream, _) = start(garbled, &Cancellation::new())?;
    let (labels, message) = drain(&stream).await?;
    assert_eq!(labels, ["start", "text_start", "text_delta", "error"]);
    assert!(matches!(message.stop_reason, StopReason::Error));
    assert!(
        message
            .error_message
            .as_ref()
            .is_some_and(|text| !text.is_empty())
    );
    assert_only_text(&message, "so far");
    Ok(())
}

#[test]
fn maestro_chat_retains_failure_context() -> TestResult {
    block_on(true, async {
        assert_rows(FIXTURE, "maestro_chat_retains_failure_context").await?;
        body_fails_midstream().await?;
        body_is_not_text().await
    })
}

/// The shared message handle an update carries.
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

/// Counts whether the message could be written at the moment a reader is woken.
#[derive(Default)]
struct WriteProbe {
    message: std::sync::OnceLock<SharedAssistantMessage>,
    writable: AtomicUsize,
    locked: AtomicUsize,
}

impl Wake for WriteProbe {
    fn wake(self: Arc<Self>) {
        if let Some(message) = self.message.get() {
            let counter = if message.try_write().is_ok() {
                &self.writable
            } else {
                &self.locked
            };
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// An attempt that delivers text, pauses for a second, then ends the stream.
fn paced_attempt() -> Attempt {
    Attempt::streaming(|| {
        let pause = futures_util::stream::once(async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(b"data: [DONE]\n\n".to_vec())
        });
        Box::pin(futures_util::stream::iter([Ok(text_chunk("shared"))]).chain(pause))
    })
}

/// Read every update through a waker that probes the message lock, setting the stop reason
/// to an error as soon as text arrives. Returns the handles the updates carried.
async fn read_with_probe(
    stream: &AssistantMessageEventStream,
    probe: &Arc<WriteProbe>,
) -> TestResult<Vec<SharedAssistantMessage>> {
    let waker = Waker::from(Arc::clone(probe));
    let mut context = Context::from_waker(&waker);
    let mut handles = Vec::new();
    let mut next = Box::pin(stream.next());
    loop {
        match next.as_mut().poll(&mut context) {
            Poll::Pending => tokio::time::sleep(Duration::from_millis(1)).await,
            Poll::Ready(None) => return Ok(handles),
            Poll::Ready(Some(event)) => {
                let shared = handle(&event).clone();
                probe.message.get_or_init(|| shared.clone());
                if matches!(event, AssistantMessageEvent::TextDelta { .. }) {
                    shared
                        .write()
                        .map_err(|error| error.to_string())?
                        .stop_reason = StopReason::Error;
                }
                handles.push(shared);
                next = Box::pin(stream.next());
            }
        }
    }
}

#[test]
fn maestro_chat_shares_partial_observations() -> TestResult {
    block_on(true, async {
        let (stream, _) = start(paced_attempt(), &Cancellation::new())?;
        let probe = Arc::new(WriteProbe::default());
        let handles = read_with_probe(&stream, &probe).await?;
        let result = stream.result().await;
        assert_eq!(handles.len(), 5);
        assert!(handles.iter().all(|shared| Arc::ptr_eq(shared, &result)));
        assert_eq!(
            probe.locked.load(Ordering::Relaxed),
            0,
            "no lock is held while readers wake"
        );
        assert!(
            probe.writable.load(Ordering::Relaxed) > 0,
            "readers were woken"
        );
        let message = result.read().map_err(|error| error.to_string())?;
        assert!(matches!(message.stop_reason, StopReason::Error));
        assert_eq!(
            message.error_message.as_deref(),
            Some("Provider returned an error stop reason"),
            "a stop reason the reader set is honored when the stream is finalized"
        );
        Ok(())
    })
}

/// What a caller sees of a finished call: event labels and the final message as JSON.
async fn observe(
    model: maestro_models::Model,
    common: StreamOptions,
) -> TestResult<(Vec<String>, Value)> {
    let history = chat::context(&json!({"messages": [{"role": "user", "content": "hello"}]}))?;
    let options = OpenAICompletionsOptions {
        common,
        ..OpenAICompletionsOptions::default()
    };
    let stream = stream_openai_completions(model, history, Some(options));
    let (labels, message) = drain(&stream).await?;
    let mut message = serde_json::to_value(message)?;
    if let Some(object) = message.as_object_mut() {
        object.remove("timestamp");
    }
    Ok((labels, message))
}

/// A response with reasoning, non-Latin text, usage and a finish reason, as CRLF events.
fn framed_response() -> Vec<u8> {
    let events = [
        json!({"id": "r1", "model": "model", "choices": [{"index": 0,
            "delta": {"reasoning_content": "雪を考える"}}]}),
        json!({"id": "r1", "model": "model", "choices": [{"index": 0,
            "delta": {"content": "雪 is snow"}, "finish_reason": "stop"}]}),
        json!({"id": "r1", "model": "model", "choices": [],
            "usage": {"prompt_tokens": 10, "completion_tokens": 4}}),
    ];
    let mut body: Vec<u8> = events
        .iter()
        .flat_map(|event| format!("data: {event}\r\n\r\n").into_bytes())
        .collect();
    body.extend_from_slice(b"data: [DONE]\r\n\r\n");
    body
}

#[test]
fn maestro_chat_transport_is_replaceable() -> TestResult {
    if child_process::child_case().is_none() {
        return child_process::rerun("maestro_chat_transport_is_replaceable", "*", &[]);
    }
    block_on(false, async {
        let body = framed_response();
        // Split inside multi-byte characters so framing must reassemble them.
        let pieces: Vec<Vec<u8>> = body.chunks(7).map(<[u8]>::to_vec).collect();
        let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nx-multi: a\r\nx-multi: b\r\nconnection: close\r\n\r\n";
        let server = loopback::serve(head, pieces, Duration::from_millis(2))?;
        let mut model = chat::model(&json!({}))?;
        model.base_url = format!("{}/v1", server.url);
        let reported: Arc<std::sync::Mutex<Vec<maestro_models::ProviderResponse>>> = Arc::default();
        let sink = Arc::clone(&reported);
        let production = StreamOptions {
            api_key: Some("fixture-key".into()),
            max_retries: Some(0.0),
            on_response: Some(Arc::new(move |response, _| {
                if let Ok(mut sink) = sink.lock() {
                    sink.push(response);
                }
                Box::pin(std::future::ready(Ok(())))
            })),
            ..StreamOptions::default()
        };
        let over_http = observe(model, production).await?;
        let received = server.finish()?;
        let reported = reported.lock().map_err(|error| error.to_string())?.clone();
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].status.to_bits(), 200.0_f64.to_bits());
        assert_eq!(
            reported[0].headers.get("x-multi").map(String::as_str),
            Some("a, b")
        );

        let target = transport(vec![Attempt::body(200, &[], body)]);
        let (model, _, options) = call_inputs(&target)?;
        let controlled = observe(model, options).await?;

        assert_eq!(
            over_http, controlled,
            "the same caller sees the same call over either transport"
        );
        assert!(over_http.0.contains(&"thinking_end".to_owned()));
        assert_eq!(over_http.1["content"][1]["text"], "雪 is snow");
        assert_eq!(received.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(received.header("authorization"), Some("Bearer fixture-key"));
        assert_eq!(received.header("content-type"), Some("application/json"));
        assert_eq!(received.header("accept"), Some("application/json"));
        assert_eq!(received.body, target.first_request()?.1);
        Ok(())
    })
}
