//! Event framing, repair and reduction for direct message-protocol invocations.

#[allow(dead_code, reason = "Each test binary uses part of the chat fixtures.")]
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/json.rs"]
mod json;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the message fixtures."
)]
#[path = "support/messages.rs"]
mod messages;

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use chat::{TestResult, block_on};
use child_process::child_case;
use maestro_models::{
    AnthropicOptions, AssistantContent, AssistantMessage, AssistantMessageEvent,
    AssistantMessageEventStream, Cancellation, FetchError, HttpResponse, SharedAssistantMessage,
    StopReason, stream_anthropic,
};
use messages::{Case, Chunk, Feed, sse};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("fixtures/message_protocol/streams.json");

/// Check the rows a test owns.
async fn rows_of(test: &str) -> TestResult {
    messages::assert_rows(FIXTURE, test).await
}

/// The message start every fed stream opens with.
fn message_start() -> Value {
    json!({"type": "message_start", "message": {"id": "m", "usage": {"input_tokens": 12, "output_tokens": 1}}})
}

/// The stop that ends a stream.
fn message_stop() -> Value {
    json!({"type": "message_stop"})
}

/// A text block opening at wire position 0.
fn text_start() -> Value {
    json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text"}})
}

/// A text fragment for wire position 0.
fn text_delta(text: &str) -> Value {
    json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": text}})
}

/// Start a call whose injected client answers with a body the test feeds.
fn start_fed(signal: Option<Cancellation>) -> TestResult<(AssistantMessageEventStream, Feed)> {
    let (feed, body) = messages::feed();
    let mut options = AnthropicOptions {
        client: Some(messages::client_for(body)),
        ..AnthropicOptions::default()
    };
    options.common.signal = signal;
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    Ok((stream, feed))
}

/// Send one chunk holding the given events to a fed body.
fn feed_events(feed: &Feed, events: &[Value]) -> TestResult {
    let text: String = events.iter().map(sse).collect();
    feed.chunks.send(text.into_bytes())?;
    Ok(())
}

/// Wait for a future that must finish without further input: a future that never would
/// fails the test, at once when the clock is paused.
async fn within<T>(future: impl Future<Output = T>) -> TestResult<T> {
    Ok(tokio::time::timeout(Duration::from_secs(1), future).await?)
}

/// Read updates until one matches; return it.
async fn next_matching(
    stream: &AssistantMessageEventStream,
    matches: impl Fn(&AssistantMessageEvent) -> bool,
) -> TestResult<AssistantMessageEvent> {
    while let Some(event) = within(stream.next()).await? {
        if matches(&event) {
            return Ok(event);
        }
    }
    Err("the stream ended before the update".into())
}

/// A copy of a shared message.
fn snapshot(message: &SharedAssistantMessage) -> TestResult<AssistantMessage> {
    Ok(message.read().map_err(|error| error.to_string())?.clone())
}

/// The arguments of the only tool call of a shared message.
fn tool_arguments(message: &SharedAssistantMessage) -> TestResult<Value> {
    match snapshot(message)?.content.first() {
        Some(AssistantContent::ToolCall(call)) => Ok(Value::Object(call.arguments.clone())),
        other => Err(format!("expected a tool call, found {other:?}").into()),
    }
}

#[test]
fn messages_reduce_sparse_wire_indices() -> TestResult {
    block_on(false, rows_of("messages_reduce_sparse_wire_indices"))
}

#[test]
fn messages_accumulate_thinking_signatures() -> TestResult {
    block_on(false, rows_of("messages_accumulate_thinking_signatures"))
}

#[test]
fn messages_reduce_redacted_thinking() -> TestResult {
    block_on(false, rows_of("messages_reduce_redacted_thinking"))
}

/// A tool call opening at wire position 9 with no arguments yet.
fn tool_start() -> Value {
    json!({"type": "content_block_start", "index": 9,
        "content_block": {"type": "tool_use", "id": "call", "name": "lookup", "input": {}}})
}

/// An argument fragment for wire position 9.
fn argument_fragment(partial: &str) -> Value {
    json!({"type": "content_block_delta", "index": 9,
        "delta": {"type": "input_json_delta", "partial_json": partial}})
}

/// The close of wire position 9.
fn tool_stop() -> Value {
    json!({"type": "content_block_stop", "index": 9})
}

/// Feed one argument fragment and return the arguments its update shows.
async fn preview_after(
    stream: &AssistantMessageEventStream,
    feed: &Feed,
    fragment: &str,
) -> TestResult<Value> {
    feed_events(feed, &[argument_fragment(fragment)])?;
    let AssistantMessageEvent::ToolcallDelta { partial, .. } = next_matching(stream, |event| {
        matches!(event, AssistantMessageEvent::ToolcallDelta { .. })
    })
    .await?
    else {
        return Err("expected an argument update".into());
    };
    tool_arguments(&partial)
}

/// Close the call and return the arguments it ends with.
async fn arguments_at_end(stream: &AssistantMessageEventStream, feed: &Feed) -> TestResult<Value> {
    feed_events(feed, &[tool_stop(), message_stop()])?;
    let AssistantMessageEvent::ToolcallEnd { tool_call, .. } = next_matching(stream, |event| {
        matches!(event, AssistantMessageEvent::ToolcallEnd { .. })
    })
    .await?
    else {
        return Err("expected the end of the tool call".into());
    };
    Ok(Value::Object(tool_call.arguments))
}

/// Tool arguments are parsed as they stream: a fragment that stops inside a string already
/// shows the text so far, and the closing fragment completes the object.
async fn arguments_stream_in_partial_and_final_form() -> TestResult {
    let (stream, feed) = start_fed(None)?;
    feed_events(&feed, &[message_start(), tool_start()])?;
    let preview = preview_after(&stream, &feed, r#"{"q":"par"#).await?;
    assert_eq!(preview, json!({"q": "par"}));
    let preview = preview_after(&stream, &feed, r#"t","n":1}"#).await?;
    assert_eq!(preview, json!({"q": "part", "n": 1}));
    assert_eq!(
        arguments_at_end(&stream, &feed).await?,
        json!({"q": "part", "n": 1})
    );
    Ok(())
}

#[test]
fn messages_repair_event_and_argument_json() -> TestResult {
    block_on(false, async {
        rows_of("messages_repair_event_and_argument_json").await?;
        if child_case().as_deref() == Some("*") {
            arguments_stream_in_partial_and_final_form().await?;
        }
        Ok(())
    })
}

/// The arguments a call ends with when its argument text arrives as one fragment, closed or not.
async fn arguments_of_one_fragment(fragment: &str) -> TestResult<Value> {
    let (stream, feed) = start_fed(None)?;
    feed_events(&feed, &[message_start(), tool_start()])?;
    preview_after(&stream, &feed, fragment).await?;
    arguments_at_end(&stream, &feed).await
}

/// Argument fragments pass through the shared reader: numbers round to doubles with an overflow
/// as `null`, in the preview and at the end, and a value past the 127-container conversion bound
/// becomes an empty object, whether the fragments close it or not.
async fn streamed_arguments_follow_the_shared_reader() -> TestResult {
    let numbers = json!({"overflow": null, "rounded": 9_007_199_254_740_992_u64});
    let (stream, feed) = start_fed(None)?;
    feed_events(&feed, &[message_start(), tool_start()])?;
    let open = preview_after(
        &stream,
        &feed,
        r#"{"overflow":1e400,"rounded":9007199254740993"#,
    )
    .await?;
    assert_eq!(open, numbers);
    assert_eq!(preview_after(&stream, &feed, "}").await?, numbers);
    assert_eq!(arguments_at_end(&stream, &feed).await?, numbers);
    for (containers, closed) in [(127, true), (127, false), (128, true), (128, false)] {
        let arrays = containers - 1;
        let closing = if closed {
            format!("{}}}", "]".repeat(arrays))
        } else {
            String::new()
        };
        let fragment = format!(r#"{{"x":{}1e400{closing}"#, "[".repeat(arrays));
        let arguments = arguments_of_one_fragment(&fragment).await?;
        if containers == 127 {
            let mut leaf = &arguments["x"];
            for _ in 0..arrays {
                leaf = &leaf[0];
            }
            assert!(leaf.is_null(), "{containers} {closed}");
        } else {
            assert_eq!(arguments, json!({}), "{containers} {closed}");
        }
    }
    Ok(())
}

#[test]
fn messages_keep_initial_tool_arguments() -> TestResult {
    block_on(false, async {
        rows_of("messages_keep_initial_tool_arguments").await?;
        if child_case().as_deref() == Some("*") {
            streamed_arguments_follow_the_shared_reader().await?;
        }
        Ok(())
    })
}

#[test]
fn messages_strip_scratch_on_terminal() -> TestResult {
    block_on(false, rows_of("messages_strip_scratch_on_terminal"))
}

#[test]
fn messages_reject_unterminated_stream() -> TestResult {
    block_on(false, rows_of("messages_reject_unterminated_stream"))
}

/// A recognized stop completes the stream and releases the body, so a later event can never be
/// admitted.
async fn stop_releases_the_body() -> TestResult {
    let (stream, feed) = start_fed(None)?;
    feed_events(&feed, &[message_start(), message_stop()])?;
    let message = within(stream.result()).await?;
    assert!(matches!(snapshot(&message)?.stop_reason, StopReason::Stop));
    within(feed.released).await??;
    let tail = b"event: error\ndata: trailing failure\n\n".to_vec();
    assert!(
        feed.chunks.send(tail).is_err(),
        "nothing reads the body any more"
    );
    Ok(())
}

#[test]
fn messages_complete_at_terminal() -> TestResult {
    block_on(true, async {
        rows_of("messages_complete_at_terminal").await?;
        if child_case().as_deref() == Some("*") {
            stop_releases_the_body().await?;
        }
        Ok(())
    })
}

#[test]
fn messages_ignore_unknown_event_names() -> TestResult {
    block_on(false, rows_of("messages_ignore_unknown_event_names"))
}

#[test]
fn messages_ignore_unmatched_block_events() -> TestResult {
    block_on(false, rows_of("messages_ignore_unmatched_block_events"))
}

#[test]
fn messages_merge_usage_categories() -> TestResult {
    block_on(false, rows_of("messages_merge_usage_categories"))
}

#[test]
fn messages_map_stop_outcomes() -> TestResult {
    block_on(false, rows_of("messages_map_stop_outcomes"))
}

/// Deliver a body in the given chunks and compare the call with a row's expectation.
async fn deliver(row: &messages::Row, chunks: Vec<Chunk>) -> TestResult {
    let case = Case {
        chunks: Some(chunks),
        ..Case::default()
    };
    messages::assert_run(row, &messages::run_case(&case).await?);
    Ok(())
}

/// A body ends its lines with `\n`, `\r` or `\r\n`, and gives the same result whole, cut at any
/// position or delivered a byte at a time.
async fn framing_survives_every_cut(rows: &[messages::Row]) -> TestResult {
    let lf = rows
        .iter()
        .find(|row| row.id == "lf-body")
        .ok_or("lf-body")?;
    let Some(Chunk::Text(body)) = lf.case.chunks.as_ref().and_then(|c| c.first().cloned()) else {
        return Err("the lf body is one text chunk".into());
    };
    for ending in ["\n", "\r", "\r\n"] {
        let bytes = body.replace('\n', ending).into_bytes();
        deliver(lf, vec![Chunk::Bytes(bytes.clone())]).await?;
        for cut in 0..=bytes.len() {
            let (head, tail) = bytes.split_at(cut);
            deliver(
                lf,
                vec![Chunk::Bytes(head.to_vec()), Chunk::Bytes(tail.to_vec())],
            )
            .await?;
        }
        deliver(
            lf,
            bytes.iter().map(|byte| Chunk::Bytes(vec![*byte])).collect(),
        )
        .await?;
    }
    Ok(())
}

/// Special bodies give the same result delivered whole as a byte at a time.
async fn special_bodies_survive_bytewise_delivery(rows: &[messages::Row]) -> TestResult {
    for row in rows
        .iter()
        .filter(|row| row.id != "lf-body" && row.id != "split-crlf")
    {
        let whole: Vec<u8> = row
            .case
            .chunks
            .iter()
            .flatten()
            .flat_map(|chunk| match chunk {
                Chunk::Text(text) => text.clone().into_bytes(),
                Chunk::Bytes(bytes) => bytes.clone(),
            })
            .collect();
        deliver(
            row,
            whole.iter().map(|byte| Chunk::Bytes(vec![*byte])).collect(),
        )
        .await?;
    }
    Ok(())
}

#[test]
fn messages_decode_fragmented_sse() -> TestResult {
    block_on(false, async {
        rows_of("messages_decode_fragmented_sse").await?;
        if child_case().as_deref() == Some("*") {
            let rows = messages::rows_of(FIXTURE, "messages_decode_fragmented_sse")?;
            framing_survives_every_cut(&rows).await?;
            special_bodies_survive_bytewise_delivery(&rows).await?;
        }
        Ok(())
    })
}

#[test]
fn messages_flush_trailing_event() -> TestResult {
    block_on(false, rows_of("messages_flush_trailing_event"))
}

#[test]
fn messages_preserve_sse_fields() -> TestResult {
    block_on(false, rows_of("messages_preserve_sse_fields"))
}

#[test]
fn messages_report_exact_error_context() -> TestResult {
    block_on(false, rows_of("messages_report_exact_error_context"))
}

/// A usage count beyond the range of a double is infinite, and the sums and costs that follow
/// it are infinite too; a rounded count is the double it names.
async fn counts_are_doubles() -> TestResult {
    let wire = concat!(
        r#"event: message_delta"#,
        "\n",
        r#"data: {"type":"message_delta","delta":{},"usage":{"input_tokens":1e400,"output_tokens":-0,"cache_read_input_tokens":9007199254740993}}"#,
        "\n\n",
    );
    let body = format!("{}{wire}{}", sse(&message_start()), sse(&message_stop()));
    let case = Case {
        chunks: Some(vec![Chunk::Text(body)]),
        ..Case::default()
    };
    let requests = messages::Requests::default();
    let mut options = messages::options(&case)?;
    options.common.fetch = Some(messages::scripted_fetch(&case, &requests));
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    stream.result().await;
    let usage = snapshot(&stream.result().await)?.usage;
    assert_eq!(usage.input.to_bits(), f64::INFINITY.to_bits());
    assert_eq!(usage.output.to_bits(), (-0.0_f64).to_bits());
    assert_eq!(
        usage.cache_read.to_bits(),
        9_007_199_254_740_992.0_f64.to_bits()
    );
    assert_eq!(usage.total_tokens.to_bits(), f64::INFINITY.to_bits());
    assert_eq!(usage.cost.input.to_bits(), f64::INFINITY.to_bits());
    assert_eq!(usage.cost.total.to_bits(), f64::INFINITY.to_bits());
    Ok(())
}

#[test]
fn messages_keep_number_and_member_semantics() -> TestResult {
    block_on(false, async {
        rows_of("messages_keep_number_and_member_semantics").await?;
        if child_case().as_deref() == Some("*") {
            counts_are_doubles().await?;
        }
        Ok(())
    })
}

/// Run a body of events whose message update carries an unread member nested `depth` deep.
async fn usage_survives_unread_nesting(depth: usize) -> TestResult {
    let ignored = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
    let update = format!(
        "event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{}},\"usage\":{{\"output_tokens\":9}},\"ignored\":{ignored}}}\n\n"
    );
    let body = format!("{}{update}{}", sse(&message_start()), sse(&message_stop()));
    let case = Case {
        chunks: Some(vec![Chunk::Text(body)]),
        ..Case::default()
    };
    let run = messages::run_case(&case).await?;
    assert_eq!(run.result["stopReason"], "stop", "{depth}");
    assert_eq!(run.result["usage"]["output"], json!(9.0), "{depth}");
    Ok(())
}

/// A tool call opens with the arguments it carries when they fit the conversion bound, and with
/// none otherwise.
async fn opening_arguments_follow_the_conversion_bound() -> TestResult {
    for (containers, kept) in [(127, true), (128, false), (10_000, false)] {
        let nested = format!(
            "{}0{}",
            "[".repeat(containers - 1),
            "]".repeat(containers - 1)
        );
        let tool = format!(
            "event: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"tool_use\",\"id\":\"c\",\"name\":\"f\",\"input\":{{\"deep\":{nested}}}}}}}\n\n"
        );
        let stop = sse(&json!({"type": "content_block_stop", "index": 0}));
        let case = Case {
            chunks: Some(vec![Chunk::Text(format!(
                "{}{tool}{stop}{}",
                sse(&message_start()),
                sse(&message_stop())
            ))]),
            ..Case::default()
        };
        let run = messages::run_case(&case).await?;
        let arguments = &run.result["content"][0]["arguments"];
        assert_eq!(
            arguments
                .as_object()
                .is_some_and(|members| !members.is_empty()),
            kept,
            "{containers}"
        );
    }
    Ok(())
}

#[test]
fn messages_skip_unread_deep_values() -> TestResult {
    block_on(false, async {
        if child_case().is_none() {
            return child_process::rerun("messages_skip_unread_deep_values", "*", &[]);
        }
        for depth in [127, 128, 10_000] {
            usage_survives_unread_nesting(depth).await?;
        }
        opening_arguments_follow_the_conversion_bound().await
    })
}

/// The first update of a kind, with its message handle.
fn partial_of(event: &AssistantMessageEvent) -> Option<&SharedAssistantMessage> {
    match event {
        AssistantMessageEvent::Start { partial }
        | AssistantMessageEvent::TextDelta { partial, .. } => Some(partial),
        _ => None,
    }
}

/// Handles from earlier updates of one invocation show later changes, and dropping the observer
/// does not stop the invocation.
async fn handles_stay_live() -> TestResult {
    let (stream, feed) = start_fed(None)?;
    feed_events(&feed, &[message_start()])?;
    let started = next_matching(&stream, |event| {
        matches!(event, AssistantMessageEvent::Start { .. })
    })
    .await?;
    let early = partial_of(&started).ok_or("start handle")?.clone();
    assert!(snapshot(&early)?.content.is_empty());
    feed_events(&feed, &[text_start(), text_delta("hello")])?;
    let delta = next_matching(&stream, |event| {
        matches!(event, AssistantMessageEvent::TextDelta { .. })
    })
    .await?;
    assert!(Arc::ptr_eq(
        &early,
        partial_of(&delta).ok_or("delta handle")?
    ));
    assert!(
        matches!(snapshot(&early)?.content.first(), Some(AssistantContent::Text(text)) if text.text == "hello")
    );
    feed_events(&feed, &[text_delta(" there"), message_stop()])?;
    let finished = within(stream.result()).await?;
    assert!(Arc::ptr_eq(&early, &finished));
    assert!(
        matches!(snapshot(&early)?.content.first(), Some(AssistantContent::Text(text)) if text.text == "hello there")
    );
    Ok(())
}

/// An observer that is dropped before any answer arrives leaves the invocation running to its
/// end, which the released body acknowledges.
async fn dropped_observer_keeps_the_invocation_running() -> TestResult {
    let (stream, feed) = start_fed(None)?;
    let result = stream.result();
    drop(stream);
    feed_events(
        &feed,
        &[
            message_start(),
            text_start(),
            text_delta("kept"),
            message_stop(),
        ],
    )?;
    let message = snapshot(&within(result).await?)?;
    within(feed.released).await??;
    assert!(
        matches!(message.content.first(), Some(AssistantContent::Text(text)) if text.text == "kept")
    );
    assert!(matches!(message.stop_reason, StopReason::Stop));
    Ok(())
}

#[test]
fn messages_retain_live_message_handles() -> TestResult {
    block_on(true, async {
        handles_stay_live().await?;
        dropped_observer_keeps_the_invocation_running().await
    })
}

/// Kinds of update of a drained stream, with its final message.
async fn drain(
    stream: &AssistantMessageEventStream,
) -> TestResult<(Vec<String>, AssistantMessage)> {
    let (events, result) = within(messages::collect(stream)).await??;
    let kinds = events
        .iter()
        .map(|event| event["type"].as_str().unwrap_or_default().to_owned())
        .collect();
    Ok((kinds, serde_json::from_value(add_timestamp(result))?))
}

/// The final message with the timestamp the collector removed.
fn add_timestamp(mut result: Value) -> Value {
    if let Some(members) = result.as_object_mut() {
        members.insert("timestamp".to_owned(), json!(0));
    }
    result
}

/// A signal aborted before the call still reaches the injected client's body; the first check
/// before reading ends the call as aborted.
async fn preaborted_signal_ends_before_reading() -> TestResult {
    let signal = Cancellation::new();
    signal.abort();
    let (stream, feed) = start_fed(Some(signal))?;
    feed_events(
        &feed,
        &[
            message_start(),
            text_start(),
            text_delta("never read"),
            message_stop(),
        ],
    )?;
    let (kinds, message) = drain(&stream).await?;
    assert_eq!(kinds, ["start", "error"]);
    assert!(message.content.is_empty());
    assert_eq!(message.usage.input.to_bits(), 0.0_f64.to_bits());
    assert!(matches!(message.stop_reason, StopReason::Aborted));
    assert_eq!(
        message.error_message.as_deref(),
        Some("Request was aborted")
    );
    Ok(())
}

/// An abort while an injected body read is pending does not interrupt that read: the data it
/// yields counts, and the check before the next read, or at completion when that data holds the
/// stop, ends the call with usage and content kept.
async fn pending_injected_read_still_contributes(rest: &[Value]) -> TestResult {
    let signal = Cancellation::new();
    let (stream, mut feed) = start_fed(Some(signal.clone()))?;
    feed_events(&feed, &[message_start(), text_start()])?;
    next_matching(&stream, |event| {
        matches!(event, AssistantMessageEvent::TextStart { .. })
    })
    .await?;
    within(feed.read_pending()).await??;
    signal.abort();
    feed_events(&feed, rest)?;
    let (kinds, message) = drain(&stream).await?;
    assert_eq!(kinds, ["text_delta", "error"]);
    assert!(
        matches!(message.content.first(), Some(AssistantContent::Text(text)) if text.text == "read completed")
    );
    assert_eq!(message.usage.input.to_bits(), 12.0_f64.to_bits());
    assert!(matches!(message.stop_reason, StopReason::Aborted));
    assert_eq!(
        message.error_message.as_deref(),
        Some("Request was aborted")
    );
    Ok(())
}

/// Options for a call over the replaceable transport, whose reads race the signal.
fn transport_options(
    signal: Cancellation,
    body: Option<maestro_models::HttpBody>,
) -> TestResult<AnthropicOptions> {
    let body = std::sync::Mutex::new(body);
    let mut options = messages::options(&Case::default())?;
    options.common.signal = Some(signal);
    options.common.fetch = Some(Arc::new(move |_| {
        let taken = body.lock().ok().and_then(|mut body| body.take());
        Box::pin(std::future::ready(match taken {
            Some(body) => Ok(HttpResponse {
                status: 200,
                headers: std::collections::BTreeMap::new(),
                body,
            }),
            None => Err(FetchError::Aborted),
        }))
    }));
    Ok(options)
}

/// An abort ends a pending read of the transport's body at once, and a signal aborted before the
/// call ends it during setup with the sender's own text.
async fn transport_cancellation_covers_setup_and_body() -> TestResult {
    let signal = Cancellation::new();
    let (mut feed, body) = messages::feed();
    let options = transport_options(signal.clone(), Some(body))?;
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    next_matching(&stream, |event| {
        matches!(event, AssistantMessageEvent::Start { .. })
    })
    .await?;
    within(feed.read_pending()).await??;
    signal.abort();
    let (kinds, message) = drain(&stream).await?;
    assert_eq!(kinds, ["error"]);
    assert!(matches!(message.stop_reason, StopReason::Aborted));
    assert_eq!(
        message.error_message.as_deref(),
        Some("Request was aborted")
    );
    within(feed.released).await??;

    let signal = Cancellation::new();
    signal.abort();
    let options = transport_options(signal, None)?;
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    let (kinds, message) = drain(&stream).await?;
    assert_eq!(kinds, ["error"]);
    assert!(matches!(message.stop_reason, StopReason::Aborted));
    assert_eq!(
        message.error_message.as_deref(),
        Some("Request was aborted.")
    );
    Ok(())
}

/// A body that reports the transport's own cancellation once the signal is aborted ends the call
/// with the body's cancellation text, which has no period unlike the setup text.
async fn aborted_body_error_reads_as_body_cancellation() -> TestResult {
    let signal = Cancellation::new();
    let aborting = signal.clone();
    let body: maestro_models::HttpBody = Box::pin(futures_util::stream::once(async move {
        aborting.abort();
        Result::<Vec<u8>, FetchError>::Err(FetchError::Aborted)
    }));
    let options = transport_options(signal, Some(body))?;
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    let (kinds, message) = drain(&stream).await?;
    assert_eq!(kinds, ["start", "error"]);
    assert!(matches!(message.stop_reason, StopReason::Aborted));
    assert_eq!(
        message.error_message.as_deref(),
        Some("Request was aborted")
    );
    Ok(())
}

/// A body that fails after some data ends the call with that failure and keeps what arrived.
async fn failing_body_keeps_partial_usage() -> TestResult {
    let start = sse(&message_start()).into_bytes();
    let failure = FetchError::Connection(messages::diagnostic("socket closed"));
    let body: maestro_models::HttpBody =
        Box::pin(futures_util::stream::iter([Ok(start), Err(failure)]));
    let mut options = AnthropicOptions {
        client: Some(messages::client_for(body)),
        ..AnthropicOptions::default()
    };
    options.common.signal = None;
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    let (kinds, message) = drain(&stream).await?;
    assert_eq!(kinds, ["start", "error"]);
    assert_eq!(message.usage.input.to_bits(), 12.0_f64.to_bits());
    assert!(matches!(message.stop_reason, StopReason::Error));
    assert_eq!(message.error_message.as_deref(), Some("socket closed"));
    Ok(())
}

#[test]
fn messages_cancel_without_losing_partial_usage() -> TestResult {
    block_on(true, async {
        preaborted_signal_ends_before_reading().await?;
        pending_injected_read_still_contributes(&[text_delta("read completed")]).await?;
        pending_injected_read_still_contributes(&[text_delta("read completed"), message_stop()])
            .await?;
        transport_cancellation_covers_setup_and_body().await?;
        aborted_body_error_reads_as_body_cancellation().await?;
        failing_body_keeps_partial_usage().await
    })
}
