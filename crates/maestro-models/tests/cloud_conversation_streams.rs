//! Direct Bedrock conversation streams through the SDK with a controlled sender.

#[cfg(not(target_arch = "wasm32"))]
use crate::bedrock;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::transport;

#[cfg(not(target_arch = "wasm32"))]
use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{
    AssistantContent, AssistantMessage, BoxFuture, Cancellation, DiagnosticErrorInfo, FetchError,
    ProviderResponse,
};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::json;

#[cfg(not(target_arch = "wasm32"))]
use bedrock::{diagnostic as callback_error, options};
#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Compare token counts and costs, which are exact except for the rates' rounding.
fn near(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (got, want) in actual.iter().zip(expected) {
        assert!((got - want).abs() < 1e-9, "{got} != {want}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// One update-sequence case: name, frames, expected labels and expected block texts.
type EventCase<'a> = (&'a str, Vec<Vec<u8>>, Vec<&'a str>, Vec<&'a str>);

#[cfg(not(target_arch = "wasm32"))]
const ENDPOINT: &str = "https://bedrock-runtime.eu-central-1.amazonaws.com";

#[cfg(not(target_arch = "wasm32"))]
/// Run a conversation to its end and return its update labels and final message.
async fn run(chunks: Vec<Vec<u8>>) -> TestResult<(Vec<String>, AssistantMessage)> {
    let scripted = bedrock::serve(chunks);
    let stream = stream_bedrock(
        bedrock::model(ENDPOINT)?,
        bedrock::context("question")?,
        Some(options(&scripted)),
    );
    transport::drain(&stream).await
}

#[cfg(not(target_arch = "wasm32"))]
/// The message handle an update carries, for the updates this module sends.
fn handle(
    event: &maestro_models::AssistantMessageEvent,
) -> Option<&maestro_models::SharedAssistantMessage> {
    use maestro_models::AssistantMessageEvent as Event;
    match event {
        Event::Start { partial }
        | Event::TextStart { partial, .. }
        | Event::TextDelta { partial, .. }
        | Event::TextEnd { partial, .. } => Some(partial),
        Event::Done { message, .. } => Some(message),
        Event::Error { error, .. } => Some(error),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start() -> TestResult<Vec<u8>> {
    bedrock::event("messageStart", &json!({"role": "assistant"}))
}

#[cfg(not(target_arch = "wasm32"))]
fn delta(index: i64, text: &str) -> TestResult<Vec<u8>> {
    bedrock::event(
        "contentBlockDelta",
        &json!({"contentBlockIndex": index, "delta": {"text": text}}),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn stop(index: i64) -> TestResult<Vec<u8>> {
    bedrock::event("contentBlockStop", &json!({"contentBlockIndex": index}))
}

#[cfg(not(target_arch = "wasm32"))]
fn texts(message: &AssistantMessage) -> Vec<String> {
    message
        .content
        .iter()
        .map(|block| match block {
            AssistantContent::Text(text) => text.text.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
/// Plain update sequences the text reduction is checked against.
fn event_cases<'a>() -> TestResult<Vec<EventCase<'a>>> {
    Ok(vec![
        (
            "empty delta",
            vec![start()?, delta(9, "")?, stop(9)?],
            vec!["start", "text_start", "text_delta", "text_end", "done"],
            vec![""],
        ),
        (
            "repeated deltas",
            vec![start()?, delta(0, "a")?, delta(0, "b")?, stop(0)?],
            vec![
                "start",
                "text_start",
                "text_delta",
                "text_delta",
                "text_end",
                "done",
            ],
            vec!["ab"],
        ),
        (
            "unmatched stop",
            vec![start()?, stop(92)?],
            vec!["start", "done"],
            vec![],
        ),
    ])
}

#[cfg(not(target_arch = "wasm32"))]
/// Update sequences where block numbers repeat or arrive out of order.
fn numbered_cases<'a>() -> TestResult<Vec<EventCase<'a>>> {
    Ok(vec![
        (
            "reused index",
            vec![
                start()?,
                delta(7, "first")?,
                stop(7)?,
                delta(7, "second")?,
                stop(7)?,
            ],
            vec![
                "start",
                "text_start",
                "text_delta",
                "text_end",
                "text_start",
                "text_delta",
                "text_end",
                "done",
            ],
            vec!["first", "second"],
        ),
        (
            "out of order",
            vec![
                start()?,
                delta(7, "A")?,
                delta(2, "B")?,
                delta(7, "C")?,
                stop(2)?,
                stop(7)?,
            ],
            vec![
                "start",
                "text_start",
                "text_delta",
                "text_start",
                "text_delta",
                "text_delta",
                "text_end",
                "text_end",
                "done",
            ],
            vec!["AC", "B"],
        ),
    ])
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn text_events_keep_shared_result_and_completion() -> TestResult {
    crate::runtime::run(async {
        let all = event_cases()?.into_iter().chain(numbered_cases()?);
        for (name, chunks, labels, content) in all {
            let (seen, message) = run(chunks).await?;
            assert_eq!(seen, labels, "{name}");
            assert_eq!(texts(&message), content, "{name}");
        }
        updates_share_the_final_message().await
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Message, labels and the stop reason a service stop produces.
async fn finish_with(reason: Option<&str>) -> TestResult<(Vec<String>, AssistantMessage)> {
    let mut chunks = vec![start()?, delta(0, "text")?, stop(0)?];
    if let Some(reason) = reason {
        chunks.push(bedrock::event(
            "messageStop",
            &json!({"stopReason": reason}),
        )?);
    }
    run(chunks).await
}

#[cfg(not(target_arch = "wasm32"))]
/// Every update of a conversation carries the handle its result resolves to.
async fn updates_share_the_final_message() -> TestResult {
    let scripted = bedrock::serve(bedrock::answer("shared")?);
    let stream = stream_bedrock(
        bedrock::model(ENDPOINT)?,
        bedrock::context("question")?,
        Some(options(&scripted)),
    );
    let mut handles = Vec::new();
    while let Some(event) = stream.next().await {
        handles.push(
            handle(&event)
                .cloned()
                .ok_or("an update without a message")?,
        );
    }
    let result = stream.result().await;
    assert!(handles.len() >= 5);
    assert!(handles.iter().all(|seen| Arc::ptr_eq(seen, &result)));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn terminal_reasons_keep_success_and_error_outcomes() -> TestResult {
    crate::runtime::run(async {
        let outcomes = [
            (Some("end_turn"), "stop", None),
            (Some("stop_sequence"), "stop", None),
            (Some("max_tokens"), "length", None),
            (Some("model_context_window_exceeded"), "length", None),
            (Some("tool_use"), "toolUse", None),
            (
                Some("guardrail_intervened"),
                "error",
                Some("An unknown error occurred"),
            ),
            (
                Some("content_filtered"),
                "error",
                Some("An unknown error occurred"),
            ),
            (
                Some("future_reason"),
                "error",
                Some("An unknown error occurred"),
            ),
            (None, "stop", None),
        ];
        for (reason, stop_reason, error) in outcomes {
            let (labels, message) = finish_with(reason).await?;
            let ending = if error.is_some() { "error" } else { "done" };
            assert_eq!(
                labels.last().map(String::as_str),
                Some(ending),
                "{reason:?}"
            );
            assert_eq!(
                serde_json::to_value(&message)?["stopReason"],
                stop_reason,
                "{reason:?}"
            );
            assert_eq!(message.error_message.as_deref(), error, "{reason:?}");
        }
        let (labels, message) = run(vec![bedrock::event(
            "messageStart",
            &json!({"role": "user"}),
        )?])
        .await?;
        assert_eq!(labels, ["error"]);
        assert_eq!(
            message.error_message.as_deref(),
            Some("Unexpected assistant message start but got user message start instead")
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// The usage the final message reports after a metadata event.
async fn usage_after(metadata: &serde_json::Value) -> TestResult<maestro_models::Usage> {
    let (_, message) = run(vec![
        start()?,
        bedrock::event("metadata", metadata)?,
        bedrock::event("messageStop", &json!({"stopReason": "end_turn"}))?,
    ])
    .await?;
    Ok(message.usage)
}

#[cfg(not(target_arch = "wasm32"))]
fn reported(total: i64) -> serde_json::Value {
    json!({"usage": {"inputTokens": 1_000_000, "outputTokens": 2_000_000,
        "cacheReadInputTokens": 3_000_000, "cacheWriteInputTokens": 4_000_000,
        "totalTokens": total}, "metrics": {"latencyMs": 1}})
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn usage_preserves_reported_totals_and_zero_fallback() -> TestResult {
    crate::runtime::run(async {
        let usage = usage_after(&reported(9_999_999)).await?;
        near(
            &[
                usage.input,
                usage.output,
                usage.cache_read,
                usage.cache_write,
                usage.total_tokens.present().copied().unwrap(),
            ],
            &[1e6, 2e6, 3e6, 4e6, 9_999_999.0],
        );
        let cost = usage.cost;
        near(
            &[
                cost.input,
                cost.output,
                cost.cache_read,
                cost.cache_write,
                cost.total,
            ],
            &[1.0, 4.0, 9.0, 16.0, 30.0],
        );
        near(
            &[usage_after(&reported(0))
                .await?
                .total_tokens
                .present()
                .copied()
                .unwrap()],
            &[3e6],
        );
        let counts_only = json!({"usage": {"inputTokens": 7, "outputTokens": 3, "totalTokens": 0}});
        let usage = usage_after(&counts_only).await?;
        near(
            &[
                usage.cache_read,
                usage.cache_write,
                usage.total_tokens.present().copied().unwrap(),
            ],
            &[0.0, 0.0, 10.0],
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Run one conversation with prepared options and return its labels and final message.
async fn run_with(options: BedrockOptions) -> TestResult<(Vec<String>, AssistantMessage)> {
    let stream = stream_bedrock(
        bedrock::model(ENDPOINT)?,
        bedrock::context("question")?,
        Some(options),
    );
    transport::drain(&stream).await
}

#[cfg(not(target_arch = "wasm32"))]
/// The service failure a sender answers with.
fn service_failure(status: u16, class: &str, message: &str) -> TestResult<transport::Attempt> {
    Ok(transport::Attempt::body(
        status,
        &[
            ("x-amzn-errortype", class),
            ("content-type", "application/json"),
        ],
        serde_json::to_vec(&json!({"message": message}))?,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn service_failures_keep_readable_prefixes() -> TestResult {
    crate::runtime::run_paused(async {
        let sent = [
            (500, "InternalServerException", "Internal server error"),
            (424, "ModelStreamErrorException", "Model stream error"),
            (400, "ValidationException", "Validation error"),
            (429, "ThrottlingException", "Throttling error"),
            (503, "ServiceUnavailableException", "Service unavailable"),
            (403, "AccessDeniedException", "AccessDeniedException"),
        ];
        for (status, class, prefix) in sent {
            let scripted = transport::transport(vec![service_failure(status, class, "the cause")?]);
            let (labels, message) = run_with(options(&scripted)).await?;
            assert_eq!(labels, ["error"], "{class}");
            assert_eq!(
                message.error_message.as_deref(),
                Some(format!("{prefix}: the cause").as_str()),
                "{class}"
            );
        }
        let streamed = [
            ("internalServerException", "Internal server error"),
            ("modelStreamErrorException", "Model stream error"),
            ("validationException", "Validation error"),
            ("throttlingException", "Throttling error"),
            ("serviceUnavailableException", "Service unavailable"),
        ];
        for (class, prefix) in streamed {
            let chunks = vec![
                start()?,
                delta(0, "partial")?,
                bedrock::exception(class, "the cause")?,
            ];
            let (labels, message) = run(chunks).await?;
            assert_eq!(labels.last().map(String::as_str), Some("error"), "{class}");
            assert_eq!(
                message.error_message.as_deref(),
                Some(format!("{prefix}: the cause").as_str()),
                "{class}"
            );
            assert_eq!(texts(&message), ["partial"], "{class}");
        }
        for text in ["hook says", ""] {
            let scripted = bedrock::serve(bedrock::answer("unused")?);
            let mut hooked = options(&scripted);
            hooked.common.on_payload = Some(Arc::new(move |_, _| {
                Box::pin(std::future::ready(Err(callback_error(text))))
            }));
            let (_, message) = run_with(hooked).await?;
            assert_eq!(message.error_message.as_deref(), Some(text));
            assert!(scripted.seen()?.is_empty());
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// A payload hook that keeps the model it received and sends the payload unchanged.
fn model_collector(
    models: &Arc<std::sync::Mutex<Vec<Arc<maestro_models::Model>>>>,
) -> maestro_models::OnPayload {
    let models = Arc::clone(models);
    Arc::new(move |payload, model| {
        if let Ok(mut models) = models.lock() {
            models.push(model);
        }
        Box::pin(std::future::ready(Ok(payload)))
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// What a response hook saw: status and headers.
type Seen = Arc<std::sync::Mutex<Vec<(f64, Vec<(String, String)>)>>>;

#[cfg(not(target_arch = "wasm32"))]
/// A response hook that records what it saw, announces itself and waits for its release.
fn recording_hook(
    seen: &Seen,
    models: &Arc<std::sync::Mutex<Vec<Arc<maestro_models::Model>>>>,
    entered: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
) -> maestro_models::OnResponse {
    let seen = Arc::clone(seen);
    let models = Arc::clone(models);
    let entered = std::sync::Mutex::new(Some(entered));
    let release = std::sync::Mutex::new(Some(release));
    Arc::new(
        move |response: ProviderResponse, model| -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
            if let Ok(mut models) = models.lock() {
                models.push(model);
            }
            if let Ok(mut seen) = seen.lock() {
                seen.push((response.status, response.headers.into_iter().collect()));
            }
            let entered = entered.lock().ok().and_then(|mut entered| entered.take());
            let release = release.lock().ok().and_then(|mut release| release.take());
            Box::pin(async move {
                if let Some(entered) = entered {
                    entered.send(()).ok();
                }
                if let Some(release) = release {
                    release.await.ok();
                }
                Ok(())
            })
        },
    )
}

#[cfg(not(target_arch = "wasm32"))]
/// Response headers a case sends and the headers the hook must see.
type HeaderCase = (Vec<(&'static str, &'static str)>, Vec<(String, String)>);

#[cfg(not(target_arch = "wasm32"))]
fn header_cases() -> Vec<HeaderCase> {
    let named = |id: &str| vec![("x-amzn-requestid".to_owned(), id.to_owned())];
    vec![
        (
            vec![("x-amzn-requestid", "fixture-id")],
            named("fixture-id"),
        ),
        (vec![("x-amzn-requestid", "")], vec![]),
        (
            vec![("x-amzn-request-id", "hyphenated")],
            named("hyphenated"),
        ),
        (vec![("x-amz-request-id", "amazon")], named("amazon")),
        (
            vec![
                ("x-amzn-request-id", "second"),
                ("x-amz-request-id", "third"),
            ],
            named("second"),
        ),
        (
            vec![("x-amzn-requestid", ""), ("x-amzn-request-id", "later")],
            vec![],
        ),
    ]
}

#[cfg(not(target_arch = "wasm32"))]
/// Run one conversation whose response hook is held open and check what the hook saw.
async fn hook_sees(
    headers: Vec<(&'static str, &'static str)>,
    expected: Vec<(String, String)>,
) -> TestResult {
    let polled = Arc::new(AtomicBool::new(false));
    let body_flag = Arc::clone(&polled);
    let chunks = bedrock::answer("text")?;
    let scripted = transport::transport(vec![transport::Attempt::streaming_with_headers(
        200,
        &headers,
        move || bedrock::watched(Arc::clone(&body_flag), chunks.clone()),
    )]);
    let (entered, entered_signal) = tokio::sync::oneshot::channel();
    let (release_signal, release) = tokio::sync::oneshot::channel();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let models = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut hooked = options(&scripted);
    hooked.common.on_payload = Some(model_collector(&models));
    hooked.common.on_response = Some(recording_hook(&seen, &models, entered, release));
    let stream = stream_bedrock(
        bedrock::model(ENDPOINT)?,
        bedrock::context("question")?,
        Some(hooked),
    );
    entered_signal.await?;
    assert!(
        polled.load(Ordering::SeqCst),
        "the hook ran before the first frame was read"
    );
    assert_eq!(
        *seen.lock().map_err(|e| e.to_string())?,
        [(200.0, expected)]
    );
    {
        let models = models.lock().map_err(|e| e.to_string())?;
        assert_eq!(models.len(), 2);
        assert!(
            Arc::ptr_eq(&models[0], &models[1]),
            "the hooks received different models"
        );
    }
    release_signal.send(()).ok();
    let (labels, message) = transport::drain(&stream).await?;
    assert_eq!(labels.last().map(String::as_str), Some("done"));
    assert_eq!(texts(&message), ["text"]);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn response_hook_runs_after_the_first_frame_before_updates() -> TestResult {
    crate::runtime::run(async {
        for (headers, expected) in header_cases() {
            hook_sees(headers, expected).await?;
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn first_frame_failure_ends_before_the_response_hook() -> TestResult {
    crate::runtime::run(async {
        let mut damaged = start()?;
        if let Some(last) = damaged.last_mut() {
            *last ^= 0xFF;
        }
        let truncated = start()?[..10].to_vec();
        for (name, first) in [("bad checksum", damaged), ("truncated", truncated)] {
            let hooked = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&hooked);
            let scripted = bedrock::serve(vec![first]);
            let mut failing = options(&scripted);
            failing.common.on_response = Some(Arc::new(move |_, _| {
                flag.store(true, Ordering::SeqCst);
                Box::pin(std::future::ready(Err(callback_error("hook says"))))
            }));
            let (labels, message) = run_with(failing).await?;
            assert_eq!(labels, ["error"], "{name}");
            assert!(!hooked.load(Ordering::SeqCst), "{name}");
            assert!(
                message
                    .error_message
                    .as_deref()
                    .is_some_and(|text| !text.is_empty() && text != "hook says"),
                "{name}"
            );
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// A frame of an event kind the SDK does not know.
fn unknown() -> TestResult<Vec<u8>> {
    bedrock::event("futureEvent", &json!({"note": "ignored"}))
}

#[cfg(not(target_arch = "wasm32"))]
/// Run `chunks` with a response hook that counts its calls; returns the hook count too.
async fn run_counting(
    chunks: Vec<Vec<u8>>,
    failing: bool,
) -> TestResult<(Vec<String>, AssistantMessage, usize)> {
    let scripted = bedrock::serve(chunks);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let mut hooked = options(&scripted);
    hooked.common.on_response = Some(Arc::new(move |_, _| {
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(if failing {
            Err(callback_error("hook says"))
        } else {
            Ok(())
        }))
    }));
    let (labels, message) = run_with(hooked).await?;
    Ok((labels, message, calls.load(Ordering::SeqCst)))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_events_are_skipped_before_and_between_real_events() -> TestResult {
    crate::runtime::run(async {
        let plain = bedrock::answer("text")?;
        let (expected, expected_message, _) = run_counting(plain.clone(), false).await?;
        let mut leading = vec![unknown()?];
        leading.extend(plain.clone());
        let mut between = plain.clone();
        between.insert(2, unknown()?);
        for (name, chunks) in [("leading", leading), ("between", between)] {
            let (labels, message, hooks) = run_counting(chunks, false).await?;
            assert_eq!(labels, expected, "{name}");
            assert_eq!(texts(&message), texts(&expected_message), "{name}");
            assert_eq!(hooks, 1, "{name}");
        }
        let (labels, message, hooks) = run_counting(vec![unknown()?], false).await?;
        let (empty_labels, empty_message, empty_hooks) = run_counting(vec![], false).await?;
        assert_eq!(labels, empty_labels);
        assert_eq!(texts(&message), texts(&empty_message));
        assert_eq!((hooks, empty_hooks), (1, 1));
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn damaged_frame_after_unknown_event_wins_over_the_response_hook() -> TestResult {
    crate::runtime::run(async {
        let mut damaged = start()?;
        if let Some(last) = damaged.last_mut() {
            *last ^= 0xFF;
        }
        let (labels, message, hooks) = run_counting(vec![unknown()?, damaged], true).await?;
        assert_eq!(labels, ["error"]);
        assert_eq!(hooks, 0);
        assert!(
            message
                .error_message
                .as_deref()
                .is_some_and(|text| !text.is_empty() && text != "hook says")
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn callback_failure_stops_before_events() -> TestResult {
    crate::runtime::run(async {
        for text in ["response hook says", ""] {
            let scripted = bedrock::serve(bedrock::answer("unseen")?);
            let mut hooked = options(&scripted);
            hooked.common.on_response = Some(Arc::new(move |_, _| {
                Box::pin(std::future::ready(Err(callback_error(text))))
            }));
            let (labels, message) = run_with(hooked).await?;
            assert_eq!(labels, ["error"]);
            assert_eq!(message.error_message.as_deref(), Some(text));
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
use bedrock::held;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cancellation_settles_same_operation() -> TestResult {
    crate::runtime::run(async {
        let signal = Cancellation::new();
        signal.abort();
        let scripted = bedrock::serve(bedrock::answer("unused")?);
        let mut preaborted = options(&scripted);
        preaborted.common.signal = Some(signal);
        let hook_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&hook_calls);
        preaborted.common.on_payload = Some(Arc::new(move |payload, _| {
            counter.fetch_add(1, Ordering::SeqCst);
            Box::pin(std::future::ready(Ok(payload)))
        }));
        let (labels, message) = run_with(preaborted).await?;
        assert_eq!(labels.last().map(String::as_str), Some("error"));
        assert_eq!(
            message.error_message.as_deref(),
            Some("Request was aborted")
        );
        assert_eq!(serde_json::to_value(&message)?["stopReason"], "aborted");
        assert_eq!(
            hook_calls.load(Ordering::SeqCst),
            1,
            "the hook is awaited under an aborted signal"
        );

        let mut pending = held(vec![start()?, delta(0, "partial")?], vec![]);
        let signal = Cancellation::new();
        let mut midway = options(&pending.scripted);
        midway.common.signal = Some(signal.clone());
        let stream = stream_bedrock(
            bedrock::model(ENDPOINT)?,
            bedrock::context("question")?,
            Some(midway),
        );
        let mut labels = Vec::new();
        while let Some(event) = stream.next().await {
            let label = serde_json::to_value(&event)?["type"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            labels.push(label.clone());
            if label == "text_delta" {
                signal.abort();
            }
        }
        let message = stream.result().await;
        let message = message.read().map_err(|e| e.to_string())?.clone();
        assert_eq!(labels, ["start", "text_start", "text_delta", "error"]);
        assert_eq!(serde_json::to_value(&message)?["stopReason"], "aborted");
        assert_eq!(texts(&message), ["partial"]);
        assert!(
            pending.dropped.load(Ordering::SeqCst),
            "the body was not closed"
        );
        assert!(pending.received.try_recv().is_ok());
        pending.release.send(()).ok();
        drop(pending.ended);
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn binary_stream_failures_preserve_partial_text() -> TestResult {
    crate::runtime::run(async {
        let whole: Vec<u8> = bedrock::answer("fragmented")?.concat();
        let pieces: Vec<Vec<u8>> = whole.chunks(7).map(<[u8]>::to_vec).collect();
        let (labels, message) = run(pieces).await?;
        assert_eq!(labels.last().map(String::as_str), Some("done"));
        assert_eq!(texts(&message), ["fragmented"]);

        let mut damaged = stop(0)?;
        if let Some(last) = damaged.last_mut() {
            *last ^= 0xFF;
        }
        let truncated = stop(0)?[..10].to_vec();
        for (name, tail) in [("bad checksum", damaged), ("truncated", truncated)] {
            let (labels, message) = run(vec![start()?, delta(0, "partial")?, tail]).await?;
            assert_eq!(labels.last().map(String::as_str), Some("error"), "{name}");
            assert_eq!(
                serde_json::to_value(&message)?["stopReason"],
                "error",
                "{name}"
            );
            assert!(
                message
                    .error_message
                    .as_deref()
                    .is_some_and(|text| !text.is_empty()),
                "{name}"
            );
            assert_eq!(texts(&message), ["partial"], "{name}");
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn dropped_consumer_does_not_abort_producer() -> TestResult {
    crate::runtime::run(async {
        let mut pending = held(
            vec![start()?, delta(0, "kept")?],
            bedrock::answer("-later")?.split_off(1),
        );
        let stream = stream_bedrock(
            bedrock::model(ENDPOINT)?,
            bedrock::context("question")?,
            Some(options(&pending.scripted)),
        );
        let result = stream.result();
        pending.received.recv().await.ok_or("no request")?;
        drop(stream);
        pending.release.send(()).ok();
        pending.ended.await?;
        let message = result.await;
        let message = message.read().map_err(|e| e.to_string())?.clone();
        assert_eq!(texts(&message), ["kept-later"]);
        assert_eq!(serde_json::to_value(&message)?["stopReason"], "stop");
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Run an unauthenticated conversation whose setup cannot succeed and check how it ends.
fn setup_failure_ends_stream(profile: Option<&str>) -> TestResult {
    crate::runtime::run_paused(async {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        let scripted = bedrock::serve(bedrock::answer("unused")?);
        let mut unauthenticated = options(&scripted);
        unauthenticated.bearer_token = None;
        unauthenticated.profile = profile.map(str::to_owned);
        unauthenticated.common.on_payload = Some(Arc::new(move |payload, _| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(std::future::ready(Ok(payload)))
        }));
        let (labels, message) = run_with(unauthenticated).await?;
        assert_eq!(labels, ["error"]);
        assert!(
            message
                .error_message
                .as_deref()
                .is_some_and(|text| !text.is_empty())
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the hook runs before setup fails"
        );
        assert!(scripted.seen()?.is_empty());
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn sdk_setup_failures_end_stream() -> TestResult {
    match maestro_test_process::selection() {
        selection if selection.is_child() && selection.runs_case("no credentials") => {
            return setup_failure_ends_stream(None);
        }
        selection if selection.is_child() && selection.runs_case("unknown profile") => {
            return setup_failure_ends_stream(Some("absent"));
        }
        _ => {}
    }
    let isolated = [
        ("HOME", "/nonexistent"),
        ("AWS_EC2_METADATA_DISABLED", "true"),
    ];
    for case in ["no credentials", "unknown profile"] {
        {
            let output =
                maestro_test_process::TestProcess::current("sdk_setup_failures_end_stream")?
                    .nocapture()
                    .env_clear()
                    .envs(isolated.iter().copied())
                    .case(case)
                    .output()?;
            assert!(output.status.success(), "{output:?}");
        };
    }
    crate::runtime::run_paused(async {
        let scripted = transport::transport(vec![transport::Attempt::Fail(FetchError::Timeout)]);
        let (labels, message) = run_with(options(&scripted)).await?;
        assert_eq!(labels, ["error"]);
        assert!(
            message
                .error_message
                .as_deref()
                .is_some_and(|text| !text.is_empty())
        );
        assert!(!scripted.seen()?.is_empty());
        Ok(())
    })
}
