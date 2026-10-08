use super::{FauxTokenSize, usage};
use crate::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    BoxFuture, Cancellation, DiagnosticErrorInfo, DoneReason, ErrorReason, JsonObject,
    SharedAssistantMessage, StopReason, ToolCall, records::diagnostics::timestamp_now,
};
use std::{
    borrow::Cow,
    sync::{Arc, RwLock},
};

/// Normalized chunk range and optional delivery rate.
pub(super) struct Pacing {
    /// Lower chunk bound in tokens.
    min: f64,
    /// Upper chunk bound in tokens.
    max: f64,
    /// Positive estimated-token delivery rate.
    speed: Option<f64>,
}
impl Pacing {
    /// Normalize the chunk range and retain the optional delivery rate.
    pub(super) fn new(size: &FauxTokenSize, speed: Option<f64>) -> Self {
        let min = size
            .min
            .unwrap_or(3.0)
            .min(size.max.unwrap_or(5.0))
            .max(1.0);
        Self {
            min,
            max: size.max.unwrap_or(5.0).max(min),
            speed,
        }
    }
    /// Partition text into randomly sized, scalar-aligned chunks.
    fn chunks(&self, text: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut chunk = String::new();
        let mut remaining = self.chunk_size();
        for character in text.chars() {
            if remaining < 1.0 && !chunk.is_empty() {
                chunks.push(std::mem::take(&mut chunk));
                remaining = self.chunk_size();
            }
            chunk.push(character);
            remaining -= 1.0;
        }
        if !chunk.is_empty() || chunks.is_empty() {
            chunks.push(chunk);
        }
        chunks
    }
    /// Sample an inclusive token offset and convert it to characters.
    fn chunk_size(&self) -> f64 {
        (self.min + (rand::random::<f64>() * (self.max - self.min + 1.0)).floor()) * 4.0
    }
    /// Yield or wait according to the chunk estimate and delivery rate.
    async fn schedule(&self, chunk: &str) -> Result<(), DiagnosticErrorInfo> {
        if let Some(speed) = self.speed.filter(|speed| *speed > 0.0) {
            delay(usage::tokens(chunk) / speed).await
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            tokio::task::yield_now().await;
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(
                &wasm_bindgen::JsValue::UNDEFINED,
            ))
            .await
            .map_err(|error| browser_error(&error))?;
            Ok(())
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Launch a producer on the target-owned asynchronous executor.
pub(super) fn spawn(future: BoxFuture<()>) -> Result<(), DiagnosticErrorInfo> {
    static RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime, std::io::Error>> =
        std::sync::OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .build()
    });
    let runtime = runtime.as_ref().map_err(|error| DiagnosticErrorInfo {
        name: None,
        message: error.to_string(),
        stack: None,
        code: None,
    })?;
    runtime.spawn(future);
    Ok(())
}
#[cfg(target_arch = "wasm32")]
/// Launch a producer on the target-owned asynchronous executor.
pub(super) fn spawn(future: BoxFuture<()>) {
    wasm_bindgen_futures::spawn_local(future);
}
#[cfg(not(target_arch = "wasm32"))]
/// Wait for the supplied duration through the target timer.
async fn delay(seconds: f64) -> Result<(), DiagnosticErrorInfo> {
    let duration =
        std::time::Duration::try_from_secs_f64(seconds).map_err(|error| DiagnosticErrorInfo {
            name: None,
            message: error.to_string(),
            stack: None,
            code: None,
        })?;
    tokio::time::sleep(duration).await;
    Ok(())
}
#[cfg(target_arch = "wasm32")]
/// Retain a browser binding failure as diagnostic data.
fn browser_error(error: &wasm_bindgen::JsValue) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: None,
        message: format!("{error:?}"),
        stack: None,
        code: None,
    }
}
/// Invoke the browser's global timer through existing JavaScript bindings.
#[cfg(target_arch = "wasm32")]
fn set_timeout(
    callback: &js_sys::Function,
    milliseconds: f64,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    let global = js_sys::global();
    let timer = js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str("setTimeout"))?
        .dyn_into::<js_sys::Function>()?;
    timer.call2(
        &global,
        callback,
        &wasm_bindgen::JsValue::from_f64(milliseconds),
    )?;
    Ok(())
}
#[cfg(target_arch = "wasm32")]
/// Wait for the supplied duration through the target timer.
async fn delay(seconds: f64) -> Result<(), DiagnosticErrorInfo> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        if let Err(error) = set_timeout(&resolve, seconds * 1000.0) {
            let _ = reject.call1(&wasm_bindgen::JsValue::UNDEFINED, &error);
        }
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| browser_error(&error))?;
    Ok(())
}
/// Retain one content-membership version for event observations.
fn shared(message: AssistantMessage) -> SharedAssistantMessage {
    Arc::new(RwLock::new(message))
}
/// Copy an owned message for the next content-membership version.
fn snapshot(message: &SharedAssistantMessage) -> AssistantMessage {
    message
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}
/// Publish the exhaustive successful or failed terminal event.
pub(super) fn terminal(stream: &AssistantMessageEventStream, message: AssistantMessage) {
    let reason = message.stop_reason.clone();
    let message = shared(message);
    stream.push(match reason {
        StopReason::Error => AssistantMessageEvent::Error {
            reason: ErrorReason::Error,
            error: message,
        },
        StopReason::Aborted => AssistantMessageEvent::Error {
            reason: ErrorReason::Aborted,
            error: message,
        },
        StopReason::Stop => AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message,
        },
        StopReason::Length => AssistantMessageEvent::Done {
            reason: DoneReason::Length,
            message,
        },
        StopReason::ToolUse => AssistantMessageEvent::Done {
            reason: DoneReason::ToolUse,
            message,
        },
    });
}
/// Publish accumulated content as an aborted result when signalled.
fn cancelled(
    signal: Option<&Cancellation>,
    partial: &SharedAssistantMessage,
    stream: &AssistantMessageEventStream,
) -> bool {
    if !signal.is_some_and(Cancellation::is_aborted) {
        return false;
    }
    let mut message = snapshot(partial);
    message.stop_reason = StopReason::Aborted;
    message.error_message = Some("Request was aborted".into());
    message.timestamp = timestamp_now();
    terminal(stream, message);
    true
}
/// Publish ordered block events with cooperative cancellation.
pub(super) async fn deliver(
    stream: &AssistantMessageEventStream,
    message: AssistantMessage,
    pacing: &Pacing,
    signal: Option<&Cancellation>,
) -> Result<(), DiagnosticErrorInfo> {
    let mut partial = shared(metadata(&message));
    if cancelled(signal, &partial, stream) {
        return Ok(());
    }
    stream.push(AssistantMessageEvent::Start {
        partial: Arc::clone(&partial),
    });
    for (index, block) in message.content.iter().enumerate() {
        if cancelled(signal, &partial, stream) {
            return Ok(());
        }
        let mut next = snapshot(&partial);
        next.content.push(empty_block(block));
        partial = shared(next);
        stream.push(start(block, index, Arc::clone(&partial)));
        let text = match block {
            AssistantContent::Text(value) => Cow::Borrowed(value.text.as_str()),
            AssistantContent::Thinking(value) => Cow::Borrowed(value.thinking.as_str()),
            AssistantContent::ToolCall(value) => Cow::Owned(usage::json(&value.arguments)?),
        };
        for chunk in pacing.chunks(&text) {
            pacing.schedule(&chunk).await?;
            if cancelled(signal, &partial, stream) {
                return Ok(());
            }
            accumulate(&partial, index, &chunk);
            stream.push(delta(block, index, chunk, Arc::clone(&partial)));
        }
        if let AssistantContent::ToolCall(value) = block {
            let arguments = value.arguments.clone();
            let mut guard = partial
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(AssistantContent::ToolCall(tool)) = guard.content.get_mut(index) {
                tool.arguments = arguments;
            }
        }
        stream.push(end(block, index, Arc::clone(&partial)));
    }
    terminal(stream, message);
    Ok(())
}
/// Copy only the metadata retained by an empty partial message.
fn metadata(message: &AssistantMessage) -> AssistantMessage {
    AssistantMessage {
        content: Vec::new(),
        api: message.api.clone(),
        provider: message.provider.clone(),
        model: message.model.clone(),
        response_model: message.response_model.clone(),
        response_id: message.response_id.clone(),
        diagnostics: message.diagnostics.clone(),
        usage: message.usage.clone(),
        stop_reason: message.stop_reason.clone(),
        error_message: message.error_message.clone(),
        timestamp: message.timestamp,
    }
}
/// Construct an unsigned partial block without its completed payload.
fn empty_block(block: &AssistantContent) -> AssistantContent {
    match block {
        AssistantContent::Text(_) => AssistantContent::Text(super::faux_text("")),
        AssistantContent::Thinking(_) => AssistantContent::Thinking(super::faux_thinking("")),
        AssistantContent::ToolCall(value) => AssistantContent::ToolCall(ToolCall {
            id: value.id.clone(),
            name: value.name.clone(),
            arguments: JsonObject::new(),
            thought_signature: None,
        }),
    }
}
/// Extend only the current partial text or thinking block.
fn accumulate(partial: &SharedAssistantMessage, index: usize, chunk: &str) {
    let mut guard = partial
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match guard.content.get_mut(index) {
        Some(AssistantContent::Text(value)) => value.text.push_str(chunk),
        Some(AssistantContent::Thinking(value)) => value.thinking.push_str(chunk),
        Some(AssistantContent::ToolCall(_)) | None => {}
    }
}
/// Construct the typed start event for a content block.
fn start(
    block: &AssistantContent,
    content_index: usize,
    partial: SharedAssistantMessage,
) -> AssistantMessageEvent {
    match block {
        AssistantContent::Text(_) => AssistantMessageEvent::TextStart {
            content_index,
            partial,
        },
        AssistantContent::Thinking(_) => AssistantMessageEvent::ThinkingStart {
            content_index,
            partial,
        },
        AssistantContent::ToolCall(_) => AssistantMessageEvent::ToolcallStart {
            content_index,
            partial,
        },
    }
}
/// Construct the typed delta event for a content block.
fn delta(
    block: &AssistantContent,
    content_index: usize,
    delta: String,
    partial: SharedAssistantMessage,
) -> AssistantMessageEvent {
    match block {
        AssistantContent::Text(_) => AssistantMessageEvent::TextDelta {
            content_index,
            delta,
            partial,
        },
        AssistantContent::Thinking(_) => AssistantMessageEvent::ThinkingDelta {
            content_index,
            delta,
            partial,
        },
        AssistantContent::ToolCall(_) => AssistantMessageEvent::ToolcallDelta {
            content_index,
            delta,
            partial,
        },
    }
}
/// Construct the typed completion event for a content block.
fn end(
    block: &AssistantContent,
    content_index: usize,
    partial: SharedAssistantMessage,
) -> AssistantMessageEvent {
    match block {
        AssistantContent::Text(value) => AssistantMessageEvent::TextEnd {
            content_index,
            content: value.text.clone(),
            partial,
        },
        AssistantContent::Thinking(value) => AssistantMessageEvent::ThinkingEnd {
            content_index,
            content: value.thinking.clone(),
            partial,
        },
        AssistantContent::ToolCall(value) => AssistantMessageEvent::ToolcallEnd {
            content_index,
            tool_call: value.clone(),
            partial,
        },
    }
}
