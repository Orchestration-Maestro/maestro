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

use std::collections::BTreeMap;
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

/// The update labels and final message of a call whose body arrives as `chunks`.
async fn outcome_of_body(chunks: Vec<Vec<u8>>) -> TestResult<(Vec<String>, AssistantMessage)> {
    let body = chunks_then(chunks, || Box::pin(futures_util::stream::empty()));
    let (stream, _) = start(body, &Cancellation::new())?;
    drain(&stream).await
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

/// Assert that a body of tool-call fragments, each given as index, identifier, name and argument
/// text, produces `expected` as identifier, name and decoded arguments.
async fn assert_calls(
    fragments: &[(&str, &str, &str, &str)],
    expected: &[(&str, &str, Value)],
) -> TestResult {
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
    assert_eq!(calls, expected);
    Ok(())
}

/// Fragments whose indices are spelled differently continue one call exactly when the spellings
/// name the same number, even though each carries another identifier.
async fn equal_numbers_are_one_position() -> TestResult {
    assert_calls(
        &[
            ("0", "a", "lookup", r#"{"a":"#),
            ("0.0", "b", "", "1"),
            ("0e0", "c", "", r#","b":"#),
            ("-0", "d", "", "2}"),
            ("1", "e", "other", r#"{"k":"#),
            ("1E0", "f", "", "3}"),
            ("9007199254740992", "g", "far", r#"{"n":"#),
            ("9007199254740993", "h", "", "4}"),
        ],
        &[
            ("a", "lookup", json!({"a": 1, "b": 2})),
            ("e", "other", json!({"k": 3})),
            ("g", "far", json!({"n": 4})),
        ],
    )
    .await
}

/// Numbers beyond the range of a double are the signed infinity or zero they round to, and
/// those are positions like any other.
async fn extreme_numbers_are_positions() -> TestResult {
    assert_calls(
        &[
            ("1e400", "a", "up", r#"{"a":"#),
            ("2e400", "b", "", "1}"),
            ("-1e400", "c", "down", r#"{"c":"#),
            ("-2e400", "d", "", "2}"),
            ("0", "e", "zero", r#"{"e":"#),
            ("1e-400", "f", "", r#"3,"g":"#),
            ("-1e-400", "g", "", "4}"),
            ("1000000000000000100", "h", "large", r#"{"h":"#),
            ("1.0000000000000001e18", "i", "", "5}"),
            ("18446744073709551615", "j", "most", r#"{"j":"#),
            ("18446744073709551616", "k", "", "6}"),
        ],
        &[
            ("a", "up", json!({"a": 1})),
            ("c", "down", json!({"c": 2})),
            ("e", "zero", json!({"e": 3, "g": 4})),
            ("h", "large", json!({"h": 5})),
            ("j", "most", json!({"j": 6})),
        ],
    )
    .await
}

#[test]
fn maestro_chat_matches_tool_indices_by_number() -> TestResult {
    block_on(true, async {
        equal_numbers_are_one_position().await?;
        extreme_numbers_are_positions().await
    })
}

/// One event whose data is `data`, written exactly as given.
fn raw_event(data: &str) -> Vec<u8> {
    format!("data: {data}\n\n").into_bytes()
}

#[test]
fn maestro_chat_keeps_usable_chunks_with_extreme_numbers() -> TestResult {
    block_on(true, async {
        let usable = raw_event(concat!(
            r#"{"unread":1e400,"id":"first","id":"r","model":"first","model":"served","#,
            r#""choices":[{"nested":[-1e400,{"deep":2e400}],"finish_reason":"stop","#,
            r#""delta":{"content":"lost","content":"usable","#,
            r#""tool_calls":[{"index":1,"\u0069ndex":0e0,"id":"a","function":{"name":"f"}}]}}],"#,
            r#""usage":{"prompt_tokens":3,"completion_tokens":4,"extra":2e400}}"#,
        ));
        let continued = raw_event(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"k\":1}"}}]}}]}"#,
        );
        let message = message_of_body(vec![usable, continued]).await?;
        assert_eq!(message.response_id.as_deref(), Some("r"));
        assert_eq!(message.response_model.as_deref(), Some("served"));
        assert!(matches!(message.stop_reason, StopReason::Stop));
        assert_eq!(
            (message.usage.input, message.usage.output),
            (3.0, 4.0),
            "usage read around the unread number"
        );
        assert!(
            matches!(&message.content[..], [AssistantContent::Text(text), AssistantContent::ToolCall(call)]
                if text.text == "usable" && call.id == "a" && call.arguments["k"] == 1),
            "the last repeated member wins and the tool call continues at index 0: {:?}",
            message.content
        );

        let malformed = vec![text_chunk("so far"), raw_event(r#"{"unread":1e400,"#)];
        let (labels, message) = outcome_of_body(malformed).await?;
        assert_eq!(labels.last().map(String::as_str), Some("error"));
        assert!(matches!(message.stop_reason, StopReason::Error));
        assert!(
            message
                .error_message
                .as_deref()
                .is_some_and(|text| !text.is_empty())
        );
        assert_only_text(&message, "so far");
        Ok(())
    })
}

/// The signature a tool call `a` keeps after a reasoning detail whose `data` is `data`.
async fn signature_after(data: &str) -> TestResult<Option<String>> {
    let detail = format!(r#"{{"type":"reasoning.encrypted","id":"a","data":{data}}}"#);
    let chunks = vec![
        tool_fragment("0", "a", "f", "{}"),
        raw_event(&format!(
            r#"{{"choices":[{{"delta":{{"reasoning_details":[{detail}]}}}}]}}"#
        )),
    ];
    let message = message_of_body(chunks).await?;
    let signature = message.content.iter().find_map(|block| match block {
        AssistantContent::ToolCall(call) => Some(call.thought_signature.clone()),
        _ => None,
    });
    Ok(signature.ok_or("the call is missing")?)
}

#[test]
fn maestro_chat_serializes_numeric_signatures() -> TestResult {
    block_on(true, async {
        let nested_in = r#"[1e400,{"10":1,"2":2,"n":9007199254740993,"z":-1e400}]"#;
        let nested_out = r#"[null,{"2":2,"10":1,"n":9007199254740992,"z":null}]"#;
        let mapped_in = r#"{"b":18446744073709551615,"a":1e21}"#;
        let mapped_out = r#"{"b":18446744073709552000,"a":1e+21}"#;
        let cases = [
            ("9007199254740993", Some("9007199254740992")),
            ("1e400", Some("null")),
            ("-1e400", Some("null")),
            ("5e-324", Some("5e-324")),
            ("1e21", Some("1e+21")),
            (nested_in, Some(nested_out)),
            (mapped_in, Some(mapped_out)),
            ("-0", None),
            ("0", None),
            ("0.0", None),
            ("1e-400", None),
            ("2.4703282292062327e-324", None),
        ];
        for (data, expected) in cases {
            let expected = expected
                .map(|data| format!(r#"{{"type":"reasoning.encrypted","id":"a","data":{data}}}"#));
            assert_eq!(signature_after(data).await?, expected, "data {data}");
        }
        Ok(())
    })
}

/// The failure text of a call that receives `events` after some text; the call must fail with
/// that text retained.
async fn failure_after_text(event: &str) -> TestResult<String> {
    let chunks = vec![text_chunk("so far"), raw_event(event)];
    let (labels, message) = outcome_of_body(chunks).await?;
    assert_eq!(labels.last().map(String::as_str), Some("error"), "{event}");
    assert!(matches!(message.stop_reason, StopReason::Error), "{event}");
    assert_only_text(&message, "so far");
    message.error_message.ok_or_else(|| "no error text".into())
}

#[test]
fn maestro_chat_formats_numeric_failures() -> TestResult {
    block_on(true, async {
        let responses = [
            (
                r#"{"error":{"n":9007199254740993}}"#,
                r#"400 {"n":9007199254740992}"#,
            ),
            (r#"{"error":{"n":1e400}}"#, r#"400 {"n":null}"#),
            (r#"{"error":{"message":1e400}}"#, "400 null"),
            (r#"{"error":{"message":{"n":1e400}}}"#, r#"400 {"n":null}"#),
            (r#"{"error":{"message":-0}}"#, r#"400 {"message":0}"#),
            (r#"{"error":1e400}"#, "400 null"),
            (r#"{"error":-1e400}"#, "400 null"),
            ("1e400", "400 status code (no body)"),
            ("[1e400]", "400 status code (no body)"),
            ("-0", "400 -0"),
        ];
        for (body, expected) in responses {
            let failure = failure_of_error_body(vec![body.as_bytes().to_vec()]).await?;
            assert_eq!(failure, expected, "body {body}");
        }
        let events = [
            (
                r#"{"error":{"n":9007199254740993}}"#,
                r#"{"n":9007199254740992}"#,
            ),
            (r#"{"error":{"n":1e400}}"#, r#"{"n":null}"#),
            (r#"{"error":{"message":1e400}}"#, "null"),
            (r#"{"error":1e400}"#, "null"),
            (r#"{"error":{"message":-0}}"#, r#"{"message":0}"#),
        ];
        for (event, expected) in events {
            assert_eq!(failure_after_text(event).await?, expected, "event {event}");
        }
        for absent in ["null", "false", "0", "-0", "0.0", "1e-400", r#""""#] {
            let event =
                format!(r#"{{"error":{absent},"choices":[{{"delta":{{"content":"fine"}}}}]}}"#);
            let text = text_of_body(vec![raw_event(&event)]).await?;
            assert_eq!(text, "fine", "error {absent} is not an error");
        }
        Ok(())
    })
}

/// The usage a call keeps when a chunk reports `usage` as written.
async fn usage_of(usage: &str) -> TestResult<maestro_models::Usage> {
    let event = raw_event(&format!(r#"{{"choices":[],"usage":{usage}}}"#));
    Ok(message_of_body(vec![event]).await?.usage)
}

#[test]
fn maestro_chat_preserves_nonfinite_usage_results() -> TestResult {
    block_on(true, async {
        let infinite =
            usage_of(r#"{"prompt_tokens":1e400,"prompt_tokens_details":{"cached_tokens":1e400}}"#)
                .await?;
        assert!(infinite.input.is_nan() && infinite.total_tokens.is_nan());
        assert_eq!(infinite.cache_read.to_bits(), f64::INFINITY.to_bits());
        assert_eq!(infinite.output.to_bits(), 0.0_f64.to_bits());

        let written = usage_of(r#"{"prompt_tokens":5,"prompt_tokens_details":{"cached_tokens":1e400,"cache_write_tokens":1e400}}"#).await?;
        assert!(written.input.is_nan() && written.cache_read.is_nan());
        assert_eq!(written.cache_write.to_bits(), f64::INFINITY.to_bits());

        let negative = usage_of(r#"{"completion_tokens":-1e400}"#).await?;
        assert_eq!(negative.output.to_bits(), f64::NEG_INFINITY.to_bits());
        assert_eq!(negative.total_tokens.to_bits(), f64::NEG_INFINITY.to_bits());
        assert_eq!(negative.input.to_bits(), 0.0_f64.to_bits());

        let signed_zero = usage_of(r#"{"prompt_tokens":-0,"completion_tokens":-0}"#).await?;
        assert_eq!(signed_zero.output.to_bits(), 0.0_f64.to_bits());
        assert_eq!(signed_zero.input.to_bits(), 0.0_f64.to_bits());

        let exact = usage_of(r#"{"completion_tokens":2.2250738585072011e-308}"#).await?;
        assert_eq!(exact.output.to_bits(), 0x000F_FFFF_FFFF_FFFF);

        let counted = usage_of(r#"{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":30,"cache_write_tokens":10}}"#).await?;
        let figures = [
            counted.input,
            counted.output,
            counted.cache_read,
            counted.cache_write,
            counted.total_tokens,
        ];
        assert_eq!(
            figures.map(f64::to_bits),
            [70.0, 20.0, 20.0, 10.0, 120.0].map(f64::to_bits)
        );
        Ok(())
    })
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

/// The headers `on_response` receives when a loopback server answers with `head_lines` after the
/// status line, then an empty event stream.
async fn response_headers_after(head_lines: &[u8]) -> TestResult<BTreeMap<String, String>> {
    let mut reply = b"HTTP/1.1 200 OK\r\n".to_vec();
    reply.extend(head_lines);
    reply.extend(b"content-length: 14\r\nconnection: close\r\n\r\ndata: [DONE]\n\n");
    let server = loopback::serve("", vec![reply], Duration::ZERO)?;
    let mut model = chat::model(&json!({}))?;
    model.base_url = format!("{}/v1", server.url);
    let reported: Arc<Mutex<Vec<maestro_models::ProviderResponse>>> = Arc::default();
    let sink = Arc::clone(&reported);
    let common = StreamOptions {
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
    observe(model, common).await?;
    server.finish()?;
    let mut reported = reported.lock().map_err(|error| error.to_string())?;
    Ok(reported.pop().ok_or("no response was reported")?.headers)
}

#[test]
fn maestro_chat_observes_repeated_response_headers() -> TestResult {
    if child_process::child_case().is_none() {
        return child_process::rerun("maestro_chat_observes_repeated_response_headers", "*", &[]);
    }
    block_on(false, async {
        let headers = response_headers_after(
            b"X-Gap: \r\nX-Gap: b\r\nX-Tail: a\r\nX-Tail: \r\n\
              Cookie: a=1\r\nCookie: b=2\r\nX-Latin: \xE9\x80\xFF\r\n\
              X-Padded: \t \xE9\xFF \t\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\n",
        )
        .await?;
        let seen = |name: &str| headers.get(name).map(String::as_str);
        assert_eq!(seen("x-gap"), Some(", b"));
        assert_eq!(seen("x-tail"), Some("a, "));
        assert_eq!(seen("cookie"), Some("a=1; b=2"));
        assert_eq!(seen("x-latin"), Some("\u{e9}\u{80}\u{ff}"));
        assert_eq!(
            seen("x-padded"),
            Some("\u{e9}\u{ff}"),
            "no surrounding whitespace"
        );
        assert_eq!(seen("set-cookie"), Some("b=2"), "the last cookie only");

        let emptied =
            response_headers_after(b"Set-Cookie: a=1\r\nSet-Cookie: \r\nCookie: \r\nCookie: b\r\n")
                .await?;
        assert_eq!(emptied.get("set-cookie").map(String::as_str), Some(""));
        assert_eq!(emptied.get("cookie").map(String::as_str), Some("; b"));
        Ok(())
    })
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

/// An error payload whose `error` value holds `containers` containers; its spaces tell the
/// written form from the compact one.
fn deep_error(containers: usize) -> String {
    format!(
        r#"{{"error": {{"x": {}}}}}"#,
        cases::nested_json(containers - 1)
    )
}

/// The compact text of the `error` member of [`deep_error`].
fn compact_error(containers: usize) -> String {
    format!(r#"{{"x":{}}}"#, cases::nested_json(containers - 1))
}

/// Fields the call never reads may nest to any depth: the chunk, error body and streamed error
/// still deliver the fields that are read, and a field read as another type is absent.
async fn deep_unread_fields_are_passed_over() -> TestResult {
    let deep = cases::nested_json(100_000);
    let chunk = format!(
        r#"{{"unread":{deep},"choices":[{{"unread":{deep},"delta":{{"unread":{deep},"content":"kept"}}}}]}}"#
    );
    assert_eq!(text_of_body(vec![raw_event(&chunk)]).await?, "kept");

    let mistyped = format!(
        r#"{{"id":{deep},"usage":{deep},"choices":[{{"finish_reason":{deep},"delta":{{"content":{deep},"tool_calls":{deep},"reasoning_details":{deep},"reasoning":"thought"}}}}]}}"#
    );
    let message = message_of_body(vec![raw_event(&mistyped)]).await?;
    assert!(
        matches!(&message.content[..], [AssistantContent::Thinking(block)] if block.thinking == "thought"),
        "{:?}",
        message.content
    );

    let error = format!(r#"{{"unread":{deep},"message":"rejected"}}"#);
    let body = format!(r#"{{"unread":{deep},"error":{error}}}"#);
    let failure = failure_of_error_body(vec![body.into_bytes()]).await?;
    assert_eq!(failure, "400 rejected");
    let failure = failure_after_text(&format!(r#"{{"error":{error}}}"#)).await?;
    assert_eq!(failure, "rejected");
    Ok(())
}

#[test]
fn maestro_chat_passes_over_deeply_nested_unread_fields() -> TestResult {
    if child_process::child_case().is_some() {
        return block_on(true, deep_unread_fields_are_passed_over());
    }
    child_process::rerun(
        "maestro_chat_passes_over_deeply_nested_unread_fields",
        "nesting",
        &[],
    )
}

/// An error payload is described in full up to 127 nested arrays or objects; a deeper one is
/// malformed, so it adds no description.
async fn error_details_convert_to_the_json_limit() -> TestResult {
    let deepest = cases::DEEPEST_NESTING;
    let failure = failure_of_error_body(vec![deep_error(deepest).into_bytes()]).await?;
    assert_eq!(failure, format!("400 {}", compact_error(deepest)));
    let failure = failure_after_text(&deep_error(deepest)).await?;
    assert_eq!(failure, compact_error(deepest));
    for containers in [100_000, deepest + 1] {
        let body = deep_error(containers).into_bytes();
        let failure = failure_of_error_body(vec![body]).await?;
        assert_eq!(failure, "400 ", "{containers} containers");
        let failure = failure_after_text(&deep_error(containers)).await?;
        assert_eq!(failure, "", "{containers} containers");
    }
    Ok(())
}

#[test]
fn maestro_chat_bounds_error_detail_nesting() -> TestResult {
    if child_process::child_case().is_some() {
        return block_on(true, error_details_convert_to_the_json_limit());
    }
    child_process::rerun("maestro_chat_bounds_error_detail_nesting", "nesting", &[])
}

/// A streamed signature is kept up to 127 nested arrays or objects; a deeper one is dropped
/// and the call still completes.
async fn streamed_signatures_convert_to_the_json_limit() -> TestResult {
    let deepest = cases::DEEPEST_NESTING;
    let data = cases::nested_json(deepest - 1);
    let kept = format!(r#"{{"type":"reasoning.encrypted","id":"a","data":{data}}}"#);
    assert_eq!(signature_after(&data).await?, Some(kept));
    for containers in [100_000, deepest] {
        let data = cases::nested_json(containers);
        assert_eq!(
            signature_after(&data).await?,
            None,
            "{containers} containers"
        );
    }
    Ok(())
}

#[test]
fn maestro_chat_bounds_streamed_signature_nesting() -> TestResult {
    if child_process::child_case().is_some() {
        return block_on(true, streamed_signatures_convert_to_the_json_limit());
    }
    child_process::rerun(
        "maestro_chat_bounds_streamed_signature_nesting",
        "nesting",
        &[],
    )
}
