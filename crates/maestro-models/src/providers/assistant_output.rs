//! The assistant message a streamed invocation fills in, the update that ends it in failure and the
//! failure a callback reports.

use std::sync::{Arc, PoisonError};

use crate::providers::http::RequestFailure;
use crate::records::diagnostics::timestamp_now;
use crate::{
    AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream, Cancellation,
    DiagnosticErrorInfo, DoneReason, ErrorReason, Model, SharedAssistantMessage, StopReason, Usage,
};

/// The assistant message an invocation fills in, before any response arrives.
pub(crate) fn initial_message(model: &Model) -> AssistantMessage {
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
    let (stop_reason, reason) = if signal.is_some_and(Cancellation::is_aborted) {
        (StopReason::Aborted, ErrorReason::Aborted)
    } else {
        (StopReason::Error, ErrorReason::Error)
    };
    {
        let mut message = output.write().unwrap_or_else(PoisonError::into_inner);
        message.stop_reason = stop_reason;
        message.error_message = Some(failure.into_text());
    }
    stream.push(AssistantMessageEvent::Error {
        reason,
        error: Arc::clone(output),
    });
    stream.end(None);
}

/// Publish completion only after reduction and cancellation checks succeed.
pub(crate) fn conclude(
    events: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
) -> Result<(), RequestFailure> {
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(RequestFailure::new("Request was aborted"));
    }
    let reason = match output
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .stop_reason
    {
        StopReason::Stop => DoneReason::Stop,
        StopReason::Length => DoneReason::Length,
        StopReason::ToolUse => DoneReason::ToolUse,
        StopReason::Error | StopReason::Aborted => {
            return Err(RequestFailure::new("An unknown error occurred"));
        }
    };
    events.push(AssistantMessageEvent::Done {
        reason,
        message: Arc::clone(output),
    });
    events.end(None);
    Ok(())
}
