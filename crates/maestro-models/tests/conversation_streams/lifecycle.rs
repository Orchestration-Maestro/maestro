//! Controlled producer ownership, admission and failure observations.
#![cfg(test)]
use crate::{
    chat::{self, TestResult},
    corpus,
};
use futures_util::StreamExt;
use maestro_models::providers::reasoning::mistral::stream_simple_mistral;
use maestro_models::{
    AssistantContent, AssistantMessageEvent as Event, Cancellation, DiagnosticErrorInfo, Fetch,
    FetchError, HttpBody, HttpResponse, MistralOptions, OnPayload, SharedAssistantMessage,
    SimpleStreamOptions, StopReason, StreamOptions, stream_mistral,
};
use serde_json::{Value, json};
use std::{
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};
use tokio::sync::oneshot;

/// Exact complete text frame.
fn text(value: &str) -> Vec<u8> {
    format!("data: {}\n\n", json!({"id":"r","model":"server","choices":[{"index":0,"finish_reason":null,"delta":{"content":value}}]})).into_bytes()
}
/// Options with a controlled nonempty credential.
fn options(fetch: Fetch) -> MistralOptions {
    MistralOptions {
        common: StreamOptions {
            api_key: Some("fixture-key".into()),
            fetch: Some(fetch),
            ..Default::default()
        },
        ..Default::default()
    }
}
/// Context shared by direct calls.
fn context() -> TestResult<maestro_models::Context> {
    chat::context(&json!({"messages":[]}))
}
/// One-use controlled body transported outside its collection lock.
fn body_fetch(body: HttpBody) -> Fetch {
    let slot = Arc::new(Mutex::new(Some(body)));
    Arc::new(move |_| {
        let body = slot.lock().unwrap().take().unwrap();
        Box::pin(std::future::ready(Ok(HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: [("content-type".into(), "text/event-stream".into())].into(),
            body,
        })))
    })
}
/// Signals teardown of the response body carrier.
struct WitnessBody {
    /// Bytes and the controlled gate.
    body: HttpBody,
    /// Observes the same invocation's carrier drop.
    completed: Option<oneshot::Sender<()>>,
}
impl futures_core::Stream for WitnessBody {
    type Item = Result<Vec<u8>, FetchError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.body.as_mut().poll_next(cx)
    }
}
impl Drop for WitnessBody {
    fn drop(&mut self) {
        if let Some(sender) = self.completed.take() {
            sender.send(()).ok();
        }
    }
}
/// Extract the live message identity from any event.
pub(super) fn handle(event: &Event) -> &SharedAssistantMessage {
    match event {
        Event::Start { partial }
        | Event::TextStart { partial, .. }
        | Event::TextDelta { partial, .. }
        | Event::TextEnd { partial, .. }
        | Event::ThinkingStart { partial, .. }
        | Event::ThinkingDelta { partial, .. }
        | Event::ThinkingEnd { partial, .. }
        | Event::ToolcallStart { partial, .. }
        | Event::ToolcallDelta { partial, .. }
        | Event::ToolcallEnd { partial, .. } => partial,
        Event::Done { message, .. } => message,
        Event::Error { error, .. } => error,
    }
}

/// Producer progresses without polling its reader or result after a gated hook.
pub async fn dropped_reader() -> TestResult {
    let (reached, waiting) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let gate = Arc::new(Mutex::new(Some((reached, released))));
    let hook: OnPayload = Arc::new(move |payload, _| {
        let (reached, released) = gate.lock().unwrap().take().unwrap();
        Box::pin(async move {
            reached.send(()).ok();
            released.await.unwrap();
            Ok(payload)
        })
    });
    let (completed, completion) = oneshot::channel();
    let body = WitnessBody {
        body: Box::pin(futures_util::stream::iter([Ok(text("independent"))])),
        completed: Some(completed),
    };
    let mut settings = options(body_fetch(Box::pin(body)));
    settings.common.on_payload = Some(hook);
    let stream = stream_mistral(corpus::model()?, context()?, Some(settings));
    let result = stream.result();
    waiting
        .await
        .expect("independent producer must reach the hook without polling its reader");
    drop(stream);
    release.send(()).unwrap();
    completion
        .await
        .expect("dropped reader must not cancel its producer before body completion");
    let output = result.await;
    assert_eq!(
        output.read().unwrap().content,
        vec![AssistantContent::Text(maestro_models::TextContent {
            text: "independent".into(),
            text_signature: None
        })]
    );
    Ok(())
}

/// Public simple wrappers preserve all five catalog reasoning selections.
pub async fn simple_controls() -> TestResult {
    for (id, reasoning, field, expected) in [
        (
            "mistral-small-2603",
            Some(maestro_models::ThinkingLevel::High),
            "reasoning_effort",
            json!("high"),
        ),
        ("mistral-small-2603", None, "reasoning_effort", Value::Null),
        (
            "magistral-medium-latest",
            Some(maestro_models::ThinkingLevel::High),
            "prompt_mode",
            json!("reasoning"),
        ),
        (
            "mistral-medium-3.5",
            Some(maestro_models::ThinkingLevel::High),
            "reasoning_effort",
            json!("high"),
        ),
        ("mistral-medium-3.5", None, "reasoning_effort", Value::Null),
    ] {
        let model = maestro_models::get_model("mistral", id).unwrap();
        let (sent, request) = oneshot::channel();
        let slot = Arc::new(Mutex::new(Some(sent)));
        let fetch: Fetch = Arc::new(move |request| {
            let sent = slot.lock().unwrap().take().unwrap();
            sent.send(serde_json::from_slice::<Value>(&request.body).unwrap())
                .unwrap();
            Box::pin(std::future::ready(Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: [("content-type".into(), "text/event-stream".into())].into(),
                body: Box::pin(futures_util::stream::empty()),
            })))
        });
        let stream = stream_simple_mistral(
            model,
            context()?,
            Some(SimpleStreamOptions {
                common: options(fetch).common,
                reasoning,
                ..Default::default()
            }),
        )?;
        assert_eq!(
            stream.result().await.read().unwrap().stop_reason,
            StopReason::Stop
        );
        assert_eq!(request.await?[field], expected);
    }
    Ok(())
}

/// An early message alias observes later text while the response hook stays unused.
pub async fn shared_admission() -> TestResult {
    let (reached, waiting) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let tail = futures_util::stream::once(async move {
        reached.send(()).ok();
        released.await.unwrap();
        Ok(text("B"))
    });
    let body: HttpBody = Box::pin(futures_util::stream::iter([Ok(text("A"))]).chain(tail));
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&calls);
    let mut settings = options(body_fetch(body));
    settings.common.on_response = Some(Arc::new(move |_, _| {
        seen.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(())))
    }));
    let before = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs_f64()
        * 1000.0)
        .floor();
    let stream = stream_mistral(corpus::model()?, context()?, Some(settings));
    let start = stream.next().await.unwrap();
    assert!(matches!(start, Event::Start { .. }));
    let retained = Arc::clone(handle(&start));
    until_delta(&stream, Some(&retained)).await;
    waiting.await?;
    assert!(
        matches!(&retained.read().unwrap().content[0], AssistantContent::Text(block) if block.text == "A")
    );
    release.send(()).unwrap();
    while let Some(event) = stream.next().await {
        assert!(Arc::ptr_eq(handle(&event), &retained));
    }
    let result = stream.result().await;
    assert!(Arc::ptr_eq(&result, &retained));
    assert_shared_result(&retained, before)?;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    Ok(())
}

/// Refused hooks and HTTP responses publish no Start after their closed result.
pub async fn refused_admission() -> TestResult {
    for hook in [true, false] {
        let fetch: Fetch = Arc::new(move |_| {
            Box::pin(std::future::ready(Ok(HttpResponse {
                status: 400,
                status_text: String::new(),
                headers: std::collections::BTreeMap::default(),
                body: Box::pin(futures_util::stream::iter([Ok(b"bad request".to_vec())])),
            })))
        });
        let mut settings = options(fetch);
        if hook {
            settings.common.on_payload = Some(Arc::new(|_, _| {
                Box::pin(std::future::ready(Err(DiagnosticErrorInfo {
                    message: "hook failed".into(),
                    name: None,
                    code: None,
                    stack: None,
                })))
            }));
        }
        let stream = stream_mistral(corpus::model()?, context()?, Some(settings));
        let output = stream.result().await;
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Event::Error { .. }));
        assert_eq!(
            output.read().unwrap().error_message.as_deref(),
            Some(if hook {
                "hook failed"
            } else {
                "Mistral API error (400): bad request"
            })
        );
    }
    Ok(())
}

/// Lifetime timeout and explicit cancellation preserve observed content.
pub async fn lifetime_failures() -> TestResult {
    for caller_signal in [false, true] {
        let (reached, waiting) = oneshot::channel();
        let tail = futures_util::stream::once(async move {
            reached.send(()).ok();
            std::future::pending::<Result<Vec<u8>, FetchError>>().await
        });
        let body = Box::pin(futures_util::stream::iter([Ok(text("kept"))]).chain(tail));
        let mut settings = options(body_fetch(body));
        let signal = Cancellation::new();
        if caller_signal {
            settings.common.signal = Some(signal.clone());
        }
        let stream = stream_mistral(corpus::model()?, context()?, Some(settings));
        until_delta(&stream, None).await;
        waiting.await?;
        if caller_signal {
            signal.abort();
        } else {
            tokio::time::advance(std::time::Duration::from_secs(30)).await;
        }
        let output = stream.result().await;
        assert_lifetime_result(&output, caller_signal);
        let mut terminal = Vec::new();
        while let Some(event) = stream.next().await {
            terminal.push(event);
        }
        assert_eq!(terminal.len(), 1);
        assert!(matches!(terminal[0], Event::Error { .. }));
    }
    Ok(())
}

/// Hook/body causes win without replacing their text with the signal classification.
pub async fn failure_causes() -> TestResult {
    for hook in [true, false] {
        for aborted in [false, true] {
            let settings = cause_options(
                if hook {
                    FailureSite::Hook
                } else {
                    FailureSite::Body
                },
                aborted,
            );
            let stream = stream_mistral(corpus::model()?, context()?, Some(settings));
            let output = stream.result().await;
            let message = output.read().unwrap();
            assert_eq!(
                message.error_message.as_deref(),
                Some(if hook { "hook failed" } else { "body failed" })
            );
            assert_eq!(
                message.stop_reason,
                if aborted {
                    StopReason::Aborted
                } else {
                    StopReason::Error
                }
            );
        }
    }
    Ok(())
}

/// Clean EOF signal check precedes the generic stop error after normal finalization.
pub async fn exhausted_signal() -> TestResult {
    let signal = Cancellation::new();
    let control = signal.clone();
    let tail = futures_util::stream::poll_fn(move |_| {
        control.abort();
        Poll::Ready(None)
    });
    let frame = format!("data: {}\n\n", json!({"id":"r","model":"server","choices":[{"index":0,"finish_reason":"error","delta":{"content":"kept"}}]})).into_bytes();
    let body = Box::pin(futures_util::stream::iter([Ok(frame)]).chain(tail));
    let mut settings = options(body_fetch(body));
    settings.common.signal = Some(signal);
    let stream = stream_mistral(corpus::model()?, context()?, Some(settings));
    let output = stream.result().await;
    assert_eq!(
        output.read().unwrap().error_message.as_deref(),
        Some("Request was aborted")
    );
    let mut ended = false;
    while let Some(event) = stream.next().await {
        if matches!(event, Event::TextEnd { .. }) {
            ended = true;
        }
    }
    assert!(ended);
    Ok(())
}

/// Explicit whitespace credentials stay nonempty and reach the raw producer.
pub async fn raw_whitespace_key() -> TestResult {
    let mut settings = options(corpus::fetch(vec![text("raw")]));
    settings.common.api_key = Some(" ".into());
    let stream = stream_mistral(corpus::model()?, context()?, Some(settings));
    assert_eq!(
        stream.result().await.read().unwrap().stop_reason,
        StopReason::Stop
    );
    Ok(())
}

/// A blocked header producer cannot publish Start before admission.
pub async fn headers_gate() -> TestResult {
    let (reached, waiting) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let slot = Arc::new(Mutex::new(Some((reached, released))));
    let fetch: Fetch = Arc::new(move |_| {
        let (reached, released) = slot.lock().unwrap().take().unwrap();
        Box::pin(async move {
            reached.send(()).ok();
            released.await.unwrap();
            Ok(HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: [("content-type".into(), "text/event-stream".into())].into(),
                body: Box::pin(futures_util::stream::empty()),
            })
        })
    });
    let stream = corpus::invoke(fetch)?;
    waiting.await?;
    let mut first = std::pin::pin!(stream.next());
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(first.as_mut(), cx).is_pending());
        Poll::Ready(())
    })
    .await;
    release.send(()).unwrap();
    assert!(matches!(first.await, Some(Event::Start { .. })));
    assert_eq!(
        stream.result().await.read().unwrap().stop_reason,
        StopReason::Stop
    );
    Ok(())
}

/// Both supplied-key operations report the existing startup failure outside the runtime.
pub fn no_runtime() -> TestResult {
    let common = StreamOptions {
        api_key: Some("fixture-key".into()),
        ..Default::default()
    };
    let raw = stream_mistral(
        corpus::model()?,
        context()?,
        Some(MistralOptions {
            common: common.clone(),
            ..Default::default()
        }),
    );
    let simple = stream_simple_mistral(
        corpus::model()?,
        context()?,
        Some(SimpleStreamOptions {
            common,
            ..Default::default()
        }),
    )?;
    chat::block_on(false, async {
        for stream in [raw, simple] {
            assert_eq!(
                stream
                    .result()
                    .await
                    .read()
                    .unwrap()
                    .error_message
                    .as_deref(),
                Some("Streaming requires a running Tokio runtime.")
            );
        }
        Ok(())
    })
}
/// Final content and bounded timestamp observed through a surviving Start alias.
fn assert_shared_result(retained: &SharedAssistantMessage, before: f64) -> TestResult {
    let message = retained.read().unwrap();
    assert!(matches!(&message.content[0], AssistantContent::Text(block) if block.text == "AB"));
    let after = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs_f64()
        * 1000.0)
        .floor();
    assert!(message.timestamp >= before && message.timestamp <= after);
    Ok(())
}
/// Inspect lifetime failure without retaining a guard across the event drain.
fn assert_lifetime_result(output: &SharedAssistantMessage, caller_signal: bool) {
    let message = output.read().unwrap();
    let (reason, text) = if caller_signal {
        (StopReason::Aborted, "aborted")
    } else {
        (StopReason::Error, "timed out")
    };
    assert_eq!(message.stop_reason, reason);
    assert!(
        message
            .error_message
            .as_ref()
            .is_some_and(|message| message.contains(text))
    );
    assert!(matches!(&message.content[0], AssistantContent::Text(block) if block.text == "kept"));
}
/// Construct a hook or body that optionally sets the signal before failing.
fn cause_options(site: FailureSite, aborted: bool) -> MistralOptions {
    let signal = Cancellation::new();
    let control = signal.clone();
    let body = futures_util::stream::once(async move {
        if aborted {
            control.abort();
        }
        Err(FetchError::Connection(diagnostic("body failed")))
    });
    let mut settings = options(body_fetch(Box::pin(body)));
    settings.common.signal = Some(signal.clone());
    if matches!(site, FailureSite::Hook) {
        settings.common.on_payload = Some(Arc::new(move |_, _| {
            if aborted {
                signal.abort();
            }
            Box::pin(std::future::ready(Err(diagnostic("hook failed"))))
        }));
    }
    settings
}
/// A controlled cause with no foreign diagnostic formatting.
fn diagnostic(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        message: message.into(),
        name: None,
        stack: None,
        code: None,
    }
}

/// The public failure boundary under observation.
#[derive(Clone, Copy)]
enum FailureSite {
    /// Payload preparation.
    Hook,
    /// Admitted body reading.
    Body,
}
/// Observe a delta before releasing the named body gate.
async fn until_delta(
    stream: &maestro_models::AssistantMessageEventStream,
    retained: Option<&SharedAssistantMessage>,
) {
    while let Some(event) = stream.next().await {
        if let Some(retained) = retained {
            assert!(Arc::ptr_eq(handle(&event), retained));
        }
        if matches!(event, Event::TextDelta { .. }) {
            return;
        }
    }
    panic!("stream ended before the gated delta");
}

/// Model-mapped efforts stay open and simple tool selection is not forwarded.
pub async fn mapped_simple_controls() -> TestResult {
    let mut model = maestro_models::get_model("mistral", "mistral-small-2603").unwrap();
    model.thinking_level_map = Some(
        [(
            maestro_models::ModelThinkingLevel::High,
            Some("future-effort".into()),
        )]
        .into(),
    );
    let (sent, request) = oneshot::channel();
    let slot = Arc::new(Mutex::new(Some(sent)));
    let fetch: Fetch = Arc::new(move |request| {
        let sent = slot.lock().unwrap().take().unwrap();
        sent.send(serde_json::from_slice::<Value>(&request.body).unwrap())
            .unwrap();
        Box::pin(std::future::ready(Ok(HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: [("content-type".into(), "text/event-stream".into())].into(),
            body: Box::pin(futures_util::stream::empty()),
        })))
    });
    let stream = stream_simple_mistral(
        model,
        context()?,
        Some(SimpleStreamOptions {
            common: options(fetch).common,
            reasoning: Some(maestro_models::ThinkingLevel::High),
            tool_choice: Some(maestro_models::ToolChoice::Required),
            ..Default::default()
        }),
    )?;
    assert_eq!(
        stream.result().await.read().unwrap().stop_reason,
        StopReason::Stop
    );
    let payload = request.await?;
    assert_eq!(payload["reasoning_effort"], "future-effort");
    assert!(!payload.as_object().unwrap().contains_key("tool_choice"));
    Ok(())
}
