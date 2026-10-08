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

use std::future::poll_fn;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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
use tokio::sync::oneshot;
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

/// The JSON of a response chunk carrying text.
fn chunk_json(text: &str) -> Value {
    json!({"id": "r", "model": "model", "choices": [{"index": 0, "delta": {"content": text}}]})
}

/// An event stream chunk carrying text.
fn text_chunk(text: &str) -> Vec<u8> {
    format!("data: {}\n\n", chunk_json(text)).into_bytes()
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

/// The test's side of a body that waits to be released.
struct Gate {
    /// Resolves once the body has delivered its first chunks and waits.
    reached: oneshot::Receiver<()>,
    /// Lets the body deliver its last chunk.
    release: oneshot::Sender<()>,
}

/// An attempt that delivers `head`, tells the gate it was reached, waits for the gate to
/// release it and then delivers `rest`.
fn gated(head: Vec<Vec<u8>>, rest: Vec<u8>) -> (Attempt, Gate) {
    let (reached_by_body, reached) = oneshot::channel();
    let (release, released_to_body) = oneshot::channel::<()>();
    let halves = Arc::new(Mutex::new(Some((reached_by_body, released_to_body))));
    let attempt = chunks_then(head, move || {
        let halves = halves.lock().ok().and_then(|mut halves| halves.take());
        let rest = rest.clone();
        Box::pin(futures_util::stream::once(async move {
            if let Some((reached, released)) = halves {
                reached.send(()).ok();
                released.await.ok();
            }
            Ok(rest)
        }))
    });
    (attempt, Gate { reached, release })
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
    let (stalled, Gate { reached, release }) = gated(vec![text_chunk("partial")], Vec::new());
    let (stream, _) = start(stalled, &signal)?;
    let trigger = signal.clone();
    tokio::spawn(async move {
        reached.await.ok();
        trigger.abort();
    });
    let (labels, message) = drain(&stream).await?;
    drop(release);
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
    let (held, Gate { reached, release }) = gated(Vec::new(), b"data: [DONE]\n\n".to_vec());
    let (stream, target) = start(held, &Cancellation::new())?;
    let result = stream.result();
    reached.await?;
    drop(stream);
    release.send(()).map_err(|()| "the body stopped waiting")?;
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
    assert_eq!(labels, ["start", "text_start", "text_delta", "error"]);
    assert!(matches!(message.stop_reason, StopReason::Error));
    assert_eq!(
        message.error_message.as_deref(),
        Some("Request was aborted.")
    );
    assert_only_text(&message, "so far");
    Ok(())
}

/// A body that aborts the caller's signal and reports its own abort in the same step.
async fn body_aborts_with_the_signal() -> TestResult {
    let signal = Cancellation::new();
    let trigger = signal.clone();
    let aborting = chunks_then(vec![text_chunk("whole")], move || {
        let trigger = trigger.clone();
        Box::pin(futures_util::stream::poll_fn(move |_| {
            trigger.abort();
            Poll::Ready(Some(Err(FetchError::Aborted)))
        }))
    });
    let (stream, _) = start(aborting, &signal)?;
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

#[test]
fn maestro_chat_cancels_owned_request_work() -> TestResult {
    block_on(true, async {
        assert_rows(FIXTURE, "maestro_chat_cancels_owned_request_work").await?;
        abort_while_the_body_is_pending().await?;
        abort_as_the_body_ends().await?;
        body_aborts_by_itself().await?;
        body_aborts_with_the_signal().await?;
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

#[test]
fn maestro_chat_retains_failure_context() -> TestResult {
    block_on(true, async {
        assert_rows(FIXTURE, "maestro_chat_retains_failure_context").await?;
        body_fails_midstream().await
    })
}

/// Longest a call may take to settle before the test calls it stuck; time is paused, so the
/// limit passes only when every task is waiting.
const SETTLE: Duration = Duration::from_secs(60);

/// The update labels and final message of a call whose body arrives as `chunks`; the call
/// must settle.
async fn outcome_of_body(chunks: Vec<Vec<u8>>) -> TestResult<(Vec<String>, AssistantMessage)> {
    let body = chunks_then(chunks, || Box::pin(futures_util::stream::empty()));
    let (stream, _) = start(body, &Cancellation::new())?;
    tokio::time::timeout(SETTLE, drain(&stream))
        .await
        .map_err(|_| "the call never settled")?
}

/// The message a call keeps when its body arrives as `chunks`; the call must complete.
async fn message_of_body(chunks: Vec<Vec<u8>>) -> TestResult<AssistantMessage> {
    let (labels, message) = outcome_of_body(chunks).await?;
    assert_eq!(
        labels.last().map(String::as_str),
        Some("done"),
        "{:?}",
        message.error_message
    );
    Ok(message)
}

/// The text a call keeps when its body arrives as `chunks`; the call must complete.
async fn text_of_body(chunks: Vec<Vec<u8>>) -> TestResult<String> {
    match &message_of_body(chunks).await?.content[..] {
        [AssistantContent::Text(text)] => Ok(text.text.clone()),
        other => Err(format!("{other:?}").into()),
    }
}

/// Check that `body` keeps `expected` as text when it arrives whole and when it is cut in two at
/// every offset.
async fn assert_text_at_every_cut(body: &[u8], expected: &str) -> TestResult {
    assert_eq!(text_of_body(vec![body.to_vec()]).await?, expected, "whole");
    for cut in 0..=body.len() {
        let (head, tail) = body.split_at(cut);
        let chunks = vec![head.to_vec(), tail.to_vec()];
        assert_eq!(text_of_body(chunks).await?, expected, "cut at {cut}");
    }
    Ok(())
}

/// One event carrying `content` bytes inside the text delta, after a byte-order mark.
fn marked_event(content: &[u8]) -> Vec<u8> {
    let mut body = vec![0xEF, 0xBB, 0xBF];
    body.extend(br#"data: {"id":"r","model":"model","choices":[{"index":0,"delta":{"content":""#);
    body.extend(content);
    body.extend(br#""}}]}"#);
    body.extend(b"\n\n");
    body
}

/// A leading byte-order mark is dropped, invalid bytes become replacement characters and a
/// character cut by a chunk boundary is completed, wherever the chunks are cut.
async fn text_is_decoded_whatever_the_chunk_boundaries() -> TestResult {
    let mut content = "雪a".as_bytes().to_vec();
    content.extend([0xFF]);
    content.extend(b"b");
    content.extend([0xE3, 0x81]);
    content.extend(b"A");
    assert_text_at_every_cut(&marked_event(&content), "雪a\u{FFFD}b\u{FFFD}A").await
}

/// Bytes that are not text and form a blank line are an unknown field, not a failure.
async fn undecodable_trailer_is_an_ignored_field() -> TestResult {
    let chunks = vec![text_chunk("so far"), vec![0xFF, 0xFE, b'\n', b'\n']];
    assert_eq!(text_of_body(chunks).await?, "so far");
    Ok(())
}

/// The failure text of a call whose response has status 400 and a body made of `chunks`.
async fn failure_of_error_body(chunks: Vec<Vec<u8>>) -> TestResult<String> {
    let rejected = Attempt::streaming_with_status(400, move || {
        Box::pin(futures_util::stream::iter(
            chunks.clone().into_iter().map(Ok),
        ))
    });
    let (stream, _) = start(rejected, &Cancellation::new())?;
    let (_, message) = drain(&stream).await?;
    message.error_message.ok_or_else(|| "no error text".into())
}

/// An error body is decoded as one text: the byte-order mark is dropped, so the JSON error
/// object is read, and bytes that are not text become replacement characters.
async fn error_body_is_decoded_as_text() -> TestResult {
    let mut json = vec![0xEF, 0xBB, 0xBF];
    json.extend(br#"{"error":{"message":"rejected"}}"#);
    for cut in 0..=json.len() {
        let (head, tail) = json.split_at(cut);
        let failure = failure_of_error_body(vec![head.to_vec(), tail.to_vec()]).await?;
        assert_eq!(failure, "400 rejected", "cut at {cut}");
    }
    let plain = vec![
        b"no ".to_vec(),
        vec![0xFF],
        b" good".to_vec(),
        vec![0xE3, 0x81],
    ];
    let failure = failure_of_error_body(plain).await?;
    assert_eq!(failure, "400 no \u{FFFD} good\u{FFFD}");
    Ok(())
}

#[test]
fn maestro_chat_decodes_body_text_leniently() -> TestResult {
    block_on(true, async {
        text_is_decoded_whatever_the_chunk_boundaries().await?;
        undecodable_trailer_is_an_ignored_field().await?;
        error_body_is_decoded_as_text().await
    })
}

/// The bytes of a byte-order mark.
const MARK: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// A line that starts with several marks loses one of them and the rest make it an unknown
/// field; the call still reads the event after it, with the line ending at once or followed by
/// a blank line.
async fn repeated_marks_open_an_ignored_field() -> TestResult {
    for marks in [2, 3] {
        for tail in [&b"\n"[..], b"\n\n"] {
            let body = [&MARK.repeat(marks)[..], tail, &text_chunk("after")].concat();
            assert_text_at_every_cut(&body, "after").await?;
        }
    }
    Ok(())
}

/// Each line is decoded on its own, so one mark opening a later line is dropped from it.
async fn a_mark_opens_a_later_line() -> TestResult {
    let body = [text_chunk("one"), MARK.to_vec(), text_chunk("two")].concat();
    assert_text_at_every_cut(&body, "onetwo").await
}

#[test]
fn maestro_chat_drops_one_byte_order_mark_per_line() -> TestResult {
    block_on(true, async {
        repeated_marks_open_an_ignored_field().await?;
        a_mark_opens_a_later_line().await
    })
}

/// Events ended by each kind of line ending, and by all of them in one body.
async fn every_line_ending_ends_a_line() -> TestResult {
    let event = |text: &str, ending: &str| format!("data: {}{ending}{ending}", chunk_json(text));
    for ending in ["\n", "\r\n", "\r"] {
        let body = format!("{}{}", event("one", ending), event("two", ending));
        assert_text_at_every_cut(body.as_bytes(), "onetwo").await?;
    }
    let mixed = [
        event("one", "\r"),
        event("two", "\r\n"),
        event("three", "\n"),
    ]
    .concat();
    assert_text_at_every_cut(mixed.as_bytes(), "onetwothree").await
}

#[test]
fn maestro_chat_ends_lines_at_every_line_ending() -> TestResult {
    block_on(true, every_line_ending_ends_a_line())
}

/// Fields other than `data` and `event` do not change the event they appear in.
async fn unknown_fields_are_ignored() -> TestResult {
    let body = [
        &b"retry: 3000\nid: 7\nbare\nfoo:bar\n: comment\nevent: update\n"[..],
        &text_chunk("kept"),
    ]
    .concat();
    assert_text_at_every_cut(&body, "kept").await
}

/// One space after the colon belongs to the field syntax; any further space is part of the data.
async fn one_space_after_the_colon_is_dropped() -> TestResult {
    for field in ["data:", "data: ", "data:  "] {
        let body = format!("{field}{}\n\n", chunk_json("x"));
        assert_text_at_every_cut(body.as_bytes(), "x").await?;
    }
    Ok(())
}

/// A last event the body ends before a blank line completes is not delivered, even when its
/// final line is whole.
async fn an_open_event_is_not_delivered() -> TestResult {
    for ending in ["", "\n"] {
        let body = [
            &text_chunk("one")[..],
            format!("data: {}{ending}", chunk_json("two")).as_bytes(),
        ]
        .concat();
        assert_text_at_every_cut(&body, "one").await?;
    }
    Ok(())
}

#[test]
fn maestro_chat_reads_event_fields() -> TestResult {
    block_on(true, async {
        unknown_fields_are_ignored().await?;
        one_space_after_the_colon_is_dropped().await?;
        an_open_event_is_not_delivered().await
    })
}

/// One tool-call fragment as event bytes; `index` is spliced in as written.
fn tool_fragment(index: &str, id: &str, name: &str, arguments: &str) -> Vec<u8> {
    let (id, name, arguments) = (json!(id), json!(name), json!(arguments));
    format!(
        r#"data: {{"choices":[{{"delta":{{"tool_calls":[{{"index":{index},"id":{id},"function":{{"name":{name},"arguments":{arguments}}}}}]}}}}]}}{}"#,
        "\n\n"
    )
    .into_bytes()
}

/// Fragments whose indices are spelled differently continue one call exactly when the spellings
/// name the same number, even though each carries another identifier.
async fn equal_numbers_are_one_position() -> TestResult {
    let fragments = [
        ("0", "a", "lookup", r#"{"a":"#),
        ("0.0", "b", "", "1"),
        ("0e0", "c", "", r#","b":"#),
        ("-0", "d", "", "2}"),
        ("1", "e", "other", r#"{"k":"#),
        ("1E0", "f", "", "3}"),
        ("9007199254740992", "g", "far", r#"{"n":"#),
        ("9007199254740993", "h", "", "4}"),
    ];
    let chunks = fragments
        .iter()
        .map(|(index, id, name, arguments)| tool_fragment(index, id, name, arguments))
        .collect();
    let message = message_of_body(chunks).await?;
    let calls: Vec<(&str, &str, Value)> = message
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContent::ToolCall(call) => {
                let arguments = serde_json::to_value(&call.arguments).ok()?;
                Some((call.id.as_str(), call.name.as_str(), arguments))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        calls,
        [
            ("a", "lookup", json!({"a": 1, "b": 2})),
            ("e", "other", json!({"k": 3})),
            ("g", "far", json!({"n": 4})),
        ]
    );
    Ok(())
}

#[test]
fn maestro_chat_matches_tool_indices_by_number() -> TestResult {
    block_on(true, equal_numbers_are_one_position())
}

/// A chunk that carries only a finish reason.
fn finish_chunk(reason: &str) -> Vec<u8> {
    let chunk = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": reason}]});
    format!("data: {chunk}\n\n").into_bytes()
}

/// The last label, stop reason and error text of a call that receives `reasons` in order.
async fn ending_after(reasons: &[&str]) -> TestResult<(String, StopReason, Option<String>)> {
    let chunks = reasons.iter().map(|reason| finish_chunk(reason)).collect();
    let (labels, message) = outcome_of_body(chunks).await?;
    let last = labels.last().cloned().ok_or("no updates")?;
    Ok((last, message.stop_reason, message.error_message))
}

#[test]
fn maestro_chat_replaces_earlier_finish_reasons() -> TestResult {
    block_on(true, async {
        let (last, stop, error) = ending_after(&["network_error", "stop"]).await?;
        assert_eq!((last.as_str(), error), ("done", None));
        assert!(matches!(stop, StopReason::Stop));

        let (last, stop, error) = ending_after(&["stop", "network_error"]).await?;
        assert_eq!(last, "error");
        assert!(matches!(stop, StopReason::Error));
        assert_eq!(
            error.as_deref(),
            Some("Provider finish_reason: network_error")
        );

        let (last, stop, error) = ending_after(&["length", ""]).await?;
        assert_eq!((last.as_str(), error), ("done", None));
        assert!(matches!(stop, StopReason::Length));
        Ok(())
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

/// Wakes the reading task after the probe has checked the message lock.
struct ProbedWaker {
    probe: Arc<WriteProbe>,
    task: Waker,
}

impl Wake for ProbedWaker {
    fn wake(self: Arc<Self>) {
        if let Some(message) = self.probe.message.get() {
            let counter = if message.try_write().is_ok() {
                &self.probe.writable
            } else {
                &self.probe.locked
            };
            counter.fetch_add(1, Ordering::Relaxed);
        }
        self.task.wake_by_ref();
    }
}

/// An attempt that delivers text and holds the end of the stream back until released.
fn held_back_attempt() -> (Attempt, oneshot::Sender<()>) {
    let (attempt, Gate { release, .. }) =
        gated(vec![text_chunk("shared")], b"data: [DONE]\n\n".to_vec());
    (attempt, release)
}

/// Wait for the next update, waking the awaiting task through the probe.
async fn next_probed(
    stream: &AssistantMessageEventStream,
    probe: &Arc<WriteProbe>,
) -> Option<AssistantMessageEvent> {
    let mut next = Box::pin(stream.next());
    poll_fn(|task| {
        let waker = Waker::from(Arc::new(ProbedWaker {
            probe: Arc::clone(probe),
            task: task.waker().clone(),
        }));
        next.as_mut().poll(&mut Context::from_waker(&waker))
    })
    .await
}

/// Read every update through a waker that probes the message lock, setting the stop reason
/// to an error as soon as text arrives and then releasing the end of the stream. Returns the
/// handles the updates carried.
async fn read_with_probe(
    stream: &AssistantMessageEventStream,
    probe: &Arc<WriteProbe>,
    release: oneshot::Sender<()>,
) -> TestResult<Vec<SharedAssistantMessage>> {
    let mut release = Some(release);
    let mut handles = Vec::new();
    while let Some(event) = next_probed(stream, probe).await {
        let shared = handle(&event).clone();
        probe.message.get_or_init(|| shared.clone());
        if matches!(event, AssistantMessageEvent::TextDelta { .. }) {
            shared
                .write()
                .map_err(|error| error.to_string())?
                .stop_reason = StopReason::Error;
            if let Some(release) = release.take() {
                release.send(()).map_err(|()| "the body stopped waiting")?;
            }
        }
        handles.push(shared);
    }
    Ok(handles)
}

#[test]
fn maestro_chat_shares_partial_observations() -> TestResult {
    block_on(true, async {
        let (attempt, release) = held_back_attempt();
        let (stream, _) = start(attempt, &Cancellation::new())?;
        let probe = Arc::new(WriteProbe::default());
        let handles = read_with_probe(&stream, &probe, release).await?;
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

/// Run a call whose reader sets the shared message's stop reason and error text when the first
/// text arrives and only then lets the stream end, returning the update labels and the final
/// message.
async fn finish_after_edit(
    stop_reason: StopReason,
    error_message: &str,
) -> TestResult<(Vec<String>, AssistantMessage)> {
    let (attempt, release) = held_back_attempt();
    let (stream, _) = start(attempt, &Cancellation::new())?;
    let mut release = Some(release);
    let mut labels = Vec::new();
    while let Some(event) = stream.next().await {
        let label = serde_json::to_value(&event)?["type"]
            .as_str()
            .map(str::to_owned);
        labels.push(label.ok_or("update type")?);
        if matches!(event, AssistantMessageEvent::TextDelta { .. }) {
            let mut message = handle(&event).write().map_err(|error| error.to_string())?;
            message.stop_reason = stop_reason.clone();
            message.error_message = Some(error_message.to_owned());
            drop(message);
            if let Some(release) = release.take() {
                release.send(()).map_err(|()| "the body stopped waiting")?;
            }
        }
    }
    let message = stream.result().await;
    let message = message.read().map_err(|error| error.to_string())?.clone();
    Ok((labels, message))
}

#[test]
fn maestro_chat_reports_supplied_stop_reasons() -> TestResult {
    block_on(true, async {
        for (stop_reason, supplied, expected) in [
            (StopReason::Aborted, "ignored", "Request was aborted"),
            (
                StopReason::Error,
                "",
                "Provider returned an error stop reason",
            ),
        ] {
            let (labels, message) = finish_after_edit(stop_reason, supplied).await?;
            assert_eq!(
                labels,
                ["start", "text_start", "text_delta", "text_end", "error"]
            );
            assert!(matches!(message.stop_reason, StopReason::Error));
            assert_eq!(message.error_message.as_deref(), Some(expected));
            assert_only_text(&message, "shared");
        }
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
