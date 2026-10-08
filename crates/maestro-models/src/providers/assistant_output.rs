//! The assistant message a streamed invocation fills in and the update that ends it in failure.

use std::sync::{Arc, PoisonError};

use crate::providers::http::RequestFailure;
use crate::records::diagnostics::timestamp_now;
use crate::{
    AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream, Cancellation,
    ErrorReason, Model, SharedAssistantMessage, StopReason, Usage, UsageCost,
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
        usage: Usage {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            cost: UsageCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total: 0.0,
            },
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: timestamp_now(),
    }
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
