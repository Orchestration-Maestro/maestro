//! Public response-session stream boundaries.

use super::super::{stream_openai_codex_responses, stream_simple_openai_codex_responses};
use crate::tests::runtime::run;
use crate::{AssistantMessageEvent, Cancellation, ErrorReason, SimpleStreamOptions, StopReason};

#[test]
fn maestro_response_sessions_raw_and_simple_key_boundaries() {
    run(maestro_response_sessions_raw_and_simple_key_boundaries_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_raw_and_simple_key_boundaries_scenario() {
    for key in [None, Some(String::new())] {
        for aborted in [false, true] {
            key_case(key.clone(), aborted).await;
        }
    }
}

/// A finite controlled HTTP response containing one text reply.
pub(super) fn response() -> crate::HttpResponse {
    let frames = super::loopback::response_events(Some("r_public"), "Visible Ω😀");
    let bytes = frames_bytes(&frames);
    crate::HttpResponse {
        status: 200,
        status_text: "OK".to_owned(),
        headers: std::collections::BTreeMap::default(),
        body: Box::pin(futures_util::stream::iter([Ok(bytes)])),
    }
}

#[test]
fn maestro_response_sessions_stream_public_sse() {
    run(maestro_response_sessions_stream_public_sse_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_stream_public_sse_scenario() {
    let (model, context, mut options) = super::invocation();
    options.common.transport = Some(crate::Transport::Sse);
    let token = options.common.api_key.clone().unwrap();
    options.common.fetch = Some(std::sync::Arc::new(move |request| {
        let bearer = format!("Bearer {token}");
        Box::pin(async move {
            assert_eq!(request.method, "POST");
            for (name, value) in [
                ("authorization", bearer.as_str()),
                ("chatgpt-account-id", "acc_test"),
                ("openai-beta", "responses=experimental"),
                ("originator", "maestro"),
                ("accept", "text/event-stream"),
            ] {
                assert!(
                    request.headers.get(name).map(String::as_str) == Some(value),
                    "header {name} differs"
                );
            }
            assert!(!request.headers.contains_key("x-api-key"));
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["model"], "gpt-5.1-codex");
            assert_eq!(body["store"], false);
            Ok(usage_response())
        })
    }));
    let stream = stream_openai_codex_responses((*model).clone(), context, Some(options));
    let mut kinds = Vec::new();
    while let Some(event) = stream.next().await {
        kinds.push(
            serde_json::to_value(event).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    assert_eq!(
        kinds,
        ["start", "text_start", "text_delta", "text_end", "done"]
    );
    let output = stream.result().await;
    let output = serde_json::to_value(&*output.read().unwrap()).unwrap();
    assert_eq!(output["content"][0]["text"], "Visible Ω😀");
    assert_eq!(output["usage"]["input"], 10.0);
    assert_eq!(output["usage"]["output"], 5.0);
    assert_eq!(output["usage"]["totalTokens"], 15.0);
}

/// A finite controlled HTTP response whose completion reports token usage.
fn usage_response() -> crate::HttpResponse {
    let mut frames = super::loopback::response_events(Some("r_public"), "Visible Ω😀");
    let mut completed: serde_json::Value = serde_json::from_str(frames.last().unwrap()).unwrap();
    completed["response"]["usage"] = serde_json::json!({
        "input_tokens": 10,
        "output_tokens": 5,
        "total_tokens": 15,
        "input_tokens_details": {"cached_tokens": 0}
    });
    *frames.last_mut().unwrap() = completed.to_string();
    crate::HttpResponse {
        body: Box::pin(futures_util::stream::iter([Ok(frames_bytes(&frames))])),
        ..response()
    }
}

#[test]
fn maestro_response_sessions_require_runtime() {
    let (model, context, options) = super::invocation();
    let events = stream_openai_codex_responses((*model).clone(), context, Some(options));
    let result = futures_util::FutureExt::now_or_never(events.result()).unwrap();
    assert_eq!(
        result.read().unwrap().error_message.as_deref(),
        Some("Streaming requires a running Tokio runtime.")
    );
}

/// Requested price branch.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PriceInput {
    /// Descriptor identity.
    id: String,
    /// Requested service tier.
    service_tier: crate::providers::responses::openai_responses::OpenAIResponsesServiceTier,
}
/// Whole usage outcome.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PriceExpected {
    /// Token categories and costs.
    usage: serde_json::Value,
}
/// One pricing fixture.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PriceCase {
    /// Query.
    input: PriceInput,
    /// Expected usage.
    expected: PriceExpected,
}

#[test]
fn maestro_response_sessions_price_public_tiers() {
    run(maestro_response_sessions_price_public_tiers_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_price_public_tiers_scenario() {
    let rows: Vec<PriceCase> =
        super::fixture_rows(include_str!("fixtures/public-price.json"), &["input"]).unwrap();
    assert_eq!(rows.len(), 4);
    for row in rows {
        price_case(row).await;
    }
}

/// Body that never reports EOF; teardown can cancel the invocation.
struct OpenBody {
    /// One terminal-bearing chunk.
    chunk: Option<Vec<u8>>,
    /// Teardown observation.
    dropped: Option<tokio::sync::oneshot::Sender<()>>,
    /// Optional cancellation applied at teardown.
    signal: Option<Cancellation>,
}
impl futures_core::Stream for OpenBody {
    type Item = Result<Vec<u8>, crate::FetchError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.chunk.take().map_or(std::task::Poll::Pending, |chunk| {
            std::task::Poll::Ready(Some(Ok(chunk)))
        })
    }
}
impl Drop for OpenBody {
    fn drop(&mut self) {
        if let Some(signal) = &self.signal {
            signal.abort();
        }
        if let Some(dropped) = self.dropped.take() {
            let _ = dropped.send(());
        }
    }
}

/// Install one body and await both its teardown and the public result.
async fn open_body_result(bytes: Vec<u8>, abort: bool) -> crate::SharedAssistantMessage {
    let (model, context, mut options) = super::invocation();
    options.common.transport = Some(crate::Transport::Sse);
    let signal = Cancellation::new();
    options.common.signal = Some(signal.clone());
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let body = std::sync::Mutex::new(Some(OpenBody {
        chunk: Some(bytes),
        dropped: Some(sender),
        signal: abort.then_some(signal),
    }));
    options.common.fetch = Some(std::sync::Arc::new(move |_| {
        let body = body.lock().unwrap().take().unwrap();
        Box::pin(async move {
            Ok(crate::HttpResponse {
                body: Box::pin(body),
                ..response()
            })
        })
    }));
    let stream = stream_openai_codex_responses((*model).clone(), context, Some(options));
    let result = stream.result().await;
    receiver.await.unwrap();
    let mut terminals = 0;
    while let Some(event) = stream.next().await {
        terminals += usize::from(matches!(
            event,
            AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
        ));
    }
    assert_eq!(terminals, 1);
    result
}

#[test]
fn maestro_response_sessions_finish_open_bodies() {
    run(maestro_response_sessions_finish_open_bodies_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_finish_open_bodies_scenario() {
    for (kind, status, done, expected) in [
        ("response.completed", "completed", true, StopReason::Stop),
        ("response.completed", "completed", false, StopReason::Stop),
        (
            "response.incomplete",
            "incomplete",
            false,
            StopReason::Length,
        ),
    ] {
        let mut frames = super::loopback::response_events(None, "open body retained Ω😀");
        *frames.last_mut().unwrap() =
            format!("{{\"type\":\"{kind}\",\"response\":{{\"status\":\"{status}\"}}}}");
        let mut bytes = frames_bytes(&frames);
        if done {
            bytes.extend_from_slice(b"data: [DONE]\n\n");
        }
        let result = open_body_result(bytes, false).await;
        let result = result.read().unwrap();
        assert_eq!(result.stop_reason, expected);
        assert_eq!(
            serde_json::to_value(&*result).unwrap()["content"][0]["text"],
            "open body retained Ω😀"
        );
    }
}

#[test]
fn maestro_response_sessions_terminal_abort_wins() {
    run(maestro_response_sessions_terminal_abort_wins_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_terminal_abort_wins_scenario() {
    let mut frames = super::loopback::response_events(Some("r_abort"), "retained Ω😀");
    *frames.last_mut().unwrap() = "{\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":5,\"output_tokens\":3,\"total_tokens\":8}}}".into();
    let bytes = frames_bytes(&frames);
    let result = open_body_result(bytes, true).await;
    let output = result.read().unwrap();
    assert_eq!(output.stop_reason, StopReason::Aborted);
    assert_eq!(output.error_message.as_deref(), Some("Request was aborted"));
    assert_eq!(
        output.usage.total_tokens.present().unwrap().to_bits(),
        8.0_f64.to_bits()
    );
    assert_eq!(
        serde_json::to_value(&*output).unwrap()["content"][0]["text"],
        "retained Ω😀"
    );
}

/// Install a source whose terminal cannot arrive until its caller releases it.
fn gated_options(
    first: Vec<u8>,
    last: Vec<u8>,
) -> (
    super::super::OpenAICodexResponsesOptions,
    tokio::sync::oneshot::Sender<()>,
) {
    use futures_util::StreamExt as _;
    let (_, _, mut options) = super::invocation();
    options.common.transport = Some(crate::Transport::Sse);
    let (release, released) = tokio::sync::oneshot::channel();
    let chunks = std::sync::Mutex::new(Some((first, last, released)));
    options.common.fetch = Some(std::sync::Arc::new(move |_| {
        let (first, last, released) = chunks.lock().unwrap().take().unwrap();
        Box::pin(async move {
            let body = futures_util::stream::once(async { Ok(first) }).chain(
                futures_util::stream::once(async move {
                    released.await.unwrap();
                    Ok(last)
                }),
            );
            Ok(crate::HttpResponse {
                body: Box::pin(body),
                ..response()
            })
        })
    }));
    (options, release)
}

#[test]
fn maestro_response_sessions_retain_shared_output_until_finalization() {
    run(maestro_response_sessions_retain_shared_output_until_finalization_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_retain_shared_output_until_finalization_scenario() {
    let first = b"data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"reasoning\",\"summary\":[]}}\n\ndata: {\"type\":\"response.reasoning_text.delta\",\"delta\":\"thinking\"}\n\ndata: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"content\":[{\"type\":\"output_text\"}]}}\n\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\ndata: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"id\":\"fc\",\"call_id\":\"call\",\"name\":\"lookup\",\"arguments\":\"\"}}\n\ndata: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{\\\"value\\\":\"}\n\n".to_vec();
    let last = b"data: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"17}\"}\n\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"arguments\":\"{\\\"value\\\":99}\"}}\n\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_vec();
    let (options, release) = gated_options(first, last);
    let (model, context, _) = super::invocation();
    let stream = stream_openai_codex_responses((*model).clone(), context, Some(options));
    let early = loop {
        if let Some(AssistantMessageEvent::ToolcallDelta { partial, .. }) = stream.next().await {
            break partial;
        }
    };
    let before = serde_json::to_value(&*early.read().unwrap()).unwrap();
    assert_eq!(before["content"][2]["arguments"], serde_json::json!({}));
    release.send(()).unwrap();
    let result = stream.result().await;
    assert!(std::sync::Arc::ptr_eq(&early, &result));
    let final_value = serde_json::to_value(&*early.read().unwrap()).unwrap();
    assert_eq!(final_value["content"][0]["thinking"], "thinking");
    assert_eq!(final_value["content"][1]["text"], "partial");
    assert_eq!(
        final_value["content"][2]["arguments"],
        serde_json::json!({"value":17})
    );
    assert!(final_value["content"][2].get("partialJson").is_none());
    let mut terminals = 0;
    while let Some(event) = stream.next().await {
        if let AssistantMessageEvent::Done { message, .. } = event {
            assert!(std::sync::Arc::ptr_eq(&message, &early));
            terminals += 1;
        }
    }
    assert_eq!(terminals, 1);
}

#[test]
fn maestro_response_sessions_finalize_partial_tool_on_error_and_abort() {
    run(maestro_response_sessions_finalize_partial_tool_on_error_and_abort_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_finalize_partial_tool_on_error_and_abort_scenario() {
    for aborted in [false, true] {
        partial_tool_case(aborted).await;
    }
}

/// Count terminal errors, requiring each to share the retained message.
async fn count_errors(
    stream: &crate::AssistantMessageEventStream,
    early: &crate::SharedAssistantMessage,
) -> usize {
    let mut errors = 0;
    while let Some(event) = stream.next().await {
        match event {
            AssistantMessageEvent::Error { error, .. } => {
                assert!(std::sync::Arc::ptr_eq(&error, early));
                errors += 1;
            }
            AssistantMessageEvent::Done { .. } => panic!("unexpected done"),
            _ => {}
        }
    }
    errors
}

/// Finalize one failed or aborted stream that holds a partial tool call.
async fn partial_tool_case(aborted: bool) {
    {
        let first = b"data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"id\":\"fc\",\"call_id\":\"call\",\"name\":\"lookup\",\"arguments\":\"\"}}\n\ndata: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{\\\"value\\\":17\"}\n\n".to_vec();
        let last = b"data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"boom\",\"message\":\"failed late\"}}}\n\n".to_vec();
        let (mut options, release) = gated_options(first, last);
        let signal = Cancellation::new();
        options.common.signal = Some(signal.clone());
        let (model, context, _) = super::invocation();
        let stream = stream_openai_codex_responses((*model).clone(), context, Some(options));
        let early = loop {
            if let Some(AssistantMessageEvent::ToolcallDelta { partial, .. }) = stream.next().await
            {
                break partial;
            }
        };
        if aborted {
            signal.abort();
        }
        release.send(()).unwrap();
        let result = stream.result().await;
        assert!(std::sync::Arc::ptr_eq(&early, &result));
        let errors = count_errors(&stream, &early).await;
        assert_eq!(errors, 1);
        let value = serde_json::to_value(&*early.read().unwrap()).unwrap();
        assert_eq!(
            value["stopReason"],
            if aborted { "aborted" } else { "error" }
        );
        assert_eq!(
            value["content"][0]["arguments"],
            serde_json::json!({"value":17})
        );
        assert!(value["content"][0].get("partialJson").is_none());
        assert!(value["content"][0].get("index").is_none());
    }
}

#[test]
fn maestro_response_sessions_detached_invocations_finish() {
    run(maestro_response_sessions_detached_invocations_finish_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_detached_invocations_finish_scenario() {
    let (model, context, _) = super::invocation();
    let start = b"data: {\"type\":\"response.created\",\"response\":{}}\n\n".to_vec();
    let terminal =
        b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n"
            .to_vec();
    let (options, release) = gated_options(start.clone(), terminal.clone());
    let reader = stream_openai_codex_responses((*model).clone(), context.clone(), Some(options));
    let observer = reader.clone();
    assert!(matches!(
        reader.next().await,
        Some(AssistantMessageEvent::Start { .. })
    ));
    drop(reader);
    let (options, release_other) = gated_options(start, terminal);
    let other = stream_openai_codex_responses((*model).clone(), context, Some(options));
    assert!(matches!(
        other.next().await,
        Some(AssistantMessageEvent::Start { .. })
    ));
    release.send(()).unwrap();
    release_other.send(()).unwrap();
    let (first, second) = futures_util::future::join(observer.result(), other.result()).await;
    assert!(!std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(first.read().unwrap().stop_reason, StopReason::Stop);
    assert_eq!(second.read().unwrap().stop_reason, StopReason::Stop);
}

/// Observe raw and simple key boundaries for one signal state.
async fn key_case(key: Option<String>, aborted: bool) {
    let (model, context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.provider = "fixture-no-key".to_owned();
    let signal = Cancellation::new();
    if aborted {
        signal.abort();
    }
    options.common.api_key = key.clone();
    options.common.signal = Some(signal);
    let simple = SimpleStreamOptions {
        common: options.common.clone(),
        ..Default::default()
    };
    let error = stream_simple_openai_codex_responses(model.clone(), context.clone(), Some(simple))
        .err()
        .unwrap();
    assert_eq!(error.message, "No API key for provider: fixture-no-key");
    let events = stream_openai_codex_responses(model, context, Some(options));
    let Some(AssistantMessageEvent::Error { reason, error }) = events.next().await else {
        panic!("missing terminal error")
    };
    assert!(matches!(
        (aborted, reason),
        (true, ErrorReason::Aborted) | (false, ErrorReason::Error)
    ));
    let result = events.result().await;
    assert!(std::sync::Arc::ptr_eq(&error, &result));
    {
        let output = result.read().unwrap();
        assert_eq!(
            output.stop_reason,
            if aborted {
                StopReason::Aborted
            } else {
                StopReason::Error
            }
        );
        assert!(output.content.is_empty());
        assert_eq!(
            output.usage.total_tokens.present().unwrap().to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            output.error_message.as_deref(),
            Some("No API key for provider: fixture-no-key")
        );
    }
    assert!(events.next().await.is_none());
}

/// Compare requested tier pricing through the public SSE stream.
async fn price_case(row: PriceCase) {
    let (mut model, context, mut options) = super::public_options::corpus_setup();
    model.id = row.input.id;
    options.service_tier = Some(super::super::Nullable::Value(row.input.service_tier));
    options.common.transport = Some(crate::Transport::Sse);
    options.common.fetch = Some(std::sync::Arc::new(|_| {
        Box::pin(async {
            let bytes = b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"service_tier\":\"default\",\"usage\":{\"input_tokens\":5,\"output_tokens\":3,\"total_tokens\":8,\"input_tokens_details\":{\"cached_tokens\":2}}}}\n\n".to_vec();
            Ok(crate::HttpResponse {
                body: Box::pin(futures_util::stream::iter([Ok(bytes)])),
                ..response()
            })
        })
    }));
    let output = stream_openai_codex_responses(model, context, Some(options))
        .result()
        .await;
    let actual = serde_json::to_value(&output.read().unwrap().usage).unwrap();
    assert_eq!(
        crate::providers::json_text::compact_json(&actual).unwrap(),
        crate::providers::json_text::compact_json(&row.expected.usage).unwrap()
    );
}

/// Encode a sequence of complete SSE records.
fn frames_bytes(frames: &[String]) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut wire = String::new();
    for frame in frames {
        writeln!(wire, "data: {frame}\n").unwrap();
    }
    wire.into_bytes()
}
