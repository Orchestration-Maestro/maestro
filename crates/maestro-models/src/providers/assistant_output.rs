//! The assistant message a streamed invocation fills in, the update that ends it in failure and the
//! failure a callback reports.

use std::sync::{Arc, PoisonError};

use crate::providers::http::RequestFailure;
use crate::records::diagnostics::timestamp_now;
use crate::{
    AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream, BoxFuture, Cancellation,
    DiagnosticErrorInfo, DoneReason, ErrorReason, Model, SharedAssistantMessage, StopReason, Usage,
};

/// The assistant message an invocation fills in, before any response arrives.
#[must_use]
pub fn initial_message(model: &Model) -> AssistantMessage {
    AssistantMessage {
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: timestamp_now(),
    }
}

/// The failure a callback reports, with its message as it is.
pub(crate) fn callback_failure(error: DiagnosticErrorInfo) -> RequestFailure {
    RequestFailure::new(error.message)
}

/// End the stream with an error or abort update that keeps the partial message.
pub(crate) fn fail(
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
    failure: RequestFailure,
) {
    publish_failure(stream, output, signal, failure.into_text());
    stream.end(None);
}

/// Update the retained partial message before publishing failure, without explicitly ending.
pub fn publish_failure(
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
    text: String,
) {
    let (stop_reason, reason) = if signal.is_some_and(Cancellation::is_aborted) {
        (StopReason::Aborted, ErrorReason::Aborted)
    } else {
        (StopReason::Error, ErrorReason::Error)
    };
    {
        let mut message = output.write().unwrap_or_else(PoisonError::into_inner);
        message.stop_reason = stop_reason;
        message.error_message = Some(text);
    }
    stream.push(AssistantMessageEvent::Error {
        reason,
        error: Arc::clone(output),
    });
}

/// Launch an owned producer, or publish the existing native runtime failure synchronously.
pub(crate) fn start_producer(
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
    task: BoxFuture<()>,
) {
    start_producer_for::<crate::Threaded>(stream, output, signal, task, None);
}

/// Admit the family's producer without changing the provider's native fallback.
pub(crate) fn start_producer_for<E: crate::ProviderExecution>(
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
    task: E::Future<'static, ()>,
    submit: E::Submission,
) {
    if let Err(error) = E::submit(task, submit, submit_http) {
        fail(stream, output, signal, error.into());
    }
}

/// Preserve the existing HTTP runtime's admission policy and failure.
fn submit_http(task: BoxFuture<()>) -> Result<(), DiagnosticErrorInfo> {
    crate::records::types::admitted(crate::providers::http::spawn_detached(task))
}

/// Publish completion only after reduction and cancellation checks succeed.
pub(crate) fn conclude(
    events: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
) -> Result<(), RequestFailure> {
    conclude_with(events, output, signal, |_| "An unknown error occurred")
}

/// Classify one retained state using the protocol's synchronous failure-text selector.
pub(crate) fn conclude_with(
    events: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
    failure_message: fn(&AssistantMessage) -> &str,
) -> Result<(), RequestFailure> {
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(RequestFailure::new("Request was aborted"));
    }
    let reason = {
        let message = output.read().unwrap_or_else(PoisonError::into_inner);
        match message.stop_reason {
            StopReason::Stop => DoneReason::Stop,
            StopReason::Length => DoneReason::Length,
            StopReason::ToolUse => DoneReason::ToolUse,
            StopReason::Error | StopReason::Aborted => {
                return Err(RequestFailure::new(failure_message(&message)));
            }
        }
    };
    events.push(AssistantMessageEvent::Done {
        reason,
        message: Arc::clone(output),
    });
    events.end(None);
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
