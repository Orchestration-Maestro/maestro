//! Proxy submission over the model-owned HTTP transport and assistant stream.
mod events;
mod request;
use crate::AgentSpawner;
pub use events::ProxyAssistantMessageEvent;
use futures_util::StreamExt;
use maestro_models::providers::http::{Raced, race};
use maestro_models::{
    AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream, Cancellation, Context,
    ErrorReason, HttpRequest, HttpResponse, Model, SharedAssistantMessage, StopReason, Usage,
    UsageCost, timestamp_now,
};
pub use request::ProxyStreamOptions;
use std::sync::{Arc, PoisonError, RwLock};

/// Diagnostic of a cancelled request, whether it was waiting or reading.
const ABORTED: &str = "Request aborted by user";

/// Submit one independently owned proxy producer and return its shared event stream.
#[must_use]
pub fn stream_proxy(
    model: Model,
    context: Context,
    options: ProxyStreamOptions,
    spawn: AgentSpawner,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let partial = initial_partial(&model);
    let producer = stream.clone();
    spawn(Box::pin(async move {
        let signal = options.options.common.signal.as_ref();
        if let Err(error) = produce(&model, &context, &options, &partial, &producer).await {
            fail(&partial, &producer, signal, error);
        }
        producer.end(None);
    }));
    drop(spawn);
    stream
}

/// Start the shared assistant message that every update reconstructs.
fn initial_partial(model: &Model) -> SharedAssistantMessage {
    let cost = UsageCost {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total: 0.0,
    };
    Arc::new(RwLock::new(AssistantMessage {
        content: vec![],
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            cost,
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: timestamp_now(),
    }))
}

/// Mark the shared message failed and publish the failure; a settled stream ignores it.
fn fail(
    partial: &SharedAssistantMessage,
    stream: &AssistantMessageEventStream,
    signal: Option<&Cancellation>,
    error: String,
) {
    let (reason, stop_reason) = if signal.is_some_and(Cancellation::is_aborted) {
        (ErrorReason::Aborted, StopReason::Aborted)
    } else {
        (ErrorReason::Error, StopReason::Error)
    };
    let mut message = partial.write().unwrap_or_else(PoisonError::into_inner);
    message.stop_reason = stop_reason;
    message.error_message = Some(error);
    drop(message);
    stream.push(AssistantMessageEvent::Error {
        reason,
        error: Arc::clone(partial),
    });
}

/// Perform the request and reduce its body; the body stays owned until a read fails or ends.
async fn produce(
    model: &Model,
    context: &Context,
    options: &ProxyStreamOptions,
    partial: &SharedAssistantMessage,
    stream: &AssistantMessageEventStream,
) -> Result<(), String> {
    let common = &options.options.common;
    let signal = common.signal.as_ref();
    let request = HttpRequest {
        method: "POST".into(),
        url: format!("{}/api/stream", options.proxy_url),
        headers: [
            (
                "authorization".into(),
                format!("Bearer {}", options.auth_token),
            ),
            ("content-type".into(), "application/json".into()),
        ]
        .into(),
        body: request::body(model, context, &options.options).map_err(|error| error.to_string())?,
        signal: common.signal.clone(),
    };
    let transport = common
        .fetch
        .clone()
        .unwrap_or_else(maestro_models::default_fetch);
    let Raced::Done(response) = race(transport(request), None, signal).await else {
        return Err(ABORTED.into());
    };
    let mut response = response.map_err(|error| error.to_string())?;
    if !(200..300).contains(&response.status) {
        return Err(status_error(response, signal).await);
    }
    let mut framing = Framing::default();
    while let Some(chunk) = next_chunk(&mut response, signal).await? {
        framing.chunk(&chunk, partial, stream)?;
    }
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(ABORTED.into());
    }
    if framing.reducer.terminal {
        Ok(())
    } else {
        Err("Proxy stream ended before a terminal event".into())
    }
}

/// Read one body chunk, failing when the signal aborts or the body fails; `None` is EOF.
async fn next_chunk(
    response: &mut HttpResponse,
    signal: Option<&Cancellation>,
) -> Result<Option<Vec<u8>>, String> {
    let next = race(response.body.next(), None, signal).await;
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(ABORTED.into());
    }
    match next {
        Raced::Done(chunk) => chunk.transpose().map_err(|error| error.to_string()),
        Raced::Cancelled | Raced::TimedOut => Err(ABORTED.into()),
    }
}

/// LF-only line assembly over raw bytes; a final unterminated line is never processed.
#[derive(Default)]
struct Framing {
    /// Bytes of the current incomplete line.
    buffer: Vec<u8>,
    /// Whether a line was already completed, after which a byte-order mark is ordinary text.
    started: bool,
    /// Reconstruction state shared by every line.
    reducer: events::Reducer,
}
impl Framing {
    /// Complete and reduce every line the chunk finishes, keeping the remainder.
    fn chunk(
        &mut self,
        bytes: &[u8],
        partial: &SharedAssistantMessage,
        stream: &AssistantMessageEventStream,
    ) -> Result<(), String> {
        for segment in bytes.split_inclusive(|byte| *byte == b'\n') {
            self.buffer.extend_from_slice(segment);
            if segment.last() != Some(&b'\n') {
                continue;
            }
            let text = String::from_utf8_lossy(&self.buffer);
            let line = if self.started {
                &*text
            } else {
                text.strip_prefix('\u{feff}').unwrap_or(&text)
            };
            self.reducer.line(line, partial, stream)?;
            self.started = true;
            self.buffer.clear();
        }
        Ok(())
    }
}

/// Prefer a nonempty JSON `error` string, keeping the status text on any read or decode failure.
async fn status_error(mut response: HttpResponse, signal: Option<&Cancellation>) -> String {
    let fallback = format!("Proxy error: {} {}", response.status, response.status_text);
    let mut bytes = Vec::new();
    loop {
        match race(response.body.next(), None, signal).await {
            Raced::Done(Some(Ok(chunk))) => bytes.extend(chunk),
            Raced::Done(None) => break,
            _ => return fallback,
        }
    }
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|value| match value.get("error")?.as_str()? {
            "" => None,
            error => Some(format!("Proxy error: {error}")),
        })
        .unwrap_or(fallback)
}
