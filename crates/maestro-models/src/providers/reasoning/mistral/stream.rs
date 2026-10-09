//! Independently produced conversation updates.
use super::{MistralOptions, request};
use crate::providers::{
    assistant_output::{fail, initial_message},
    http::{RequestFailure, spawn_detached},
};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Context, DiagnosticErrorInfo, DoneReason,
    Model, SimpleStreamOptions, StopReason,
};
use std::sync::{Arc, RwLock};

/// Start a raw conversation stream on the caller's runtime.
///
/// Request failures are terminal error updates. Dropping the reader does not cancel work.
#[must_use]
pub fn stream_mistral(
    model: Model,
    context: Context,
    options: Option<MistralOptions>,
) -> AssistantMessageEventStream {
    start(model, context, options.unwrap_or_default(), None)
}

/// Resolve credentials before starting a simple conversation stream.
///
/// # Errors
/// Returns a diagnostic when no nonempty credential is available.
/// Native streaming requires a running Tokio runtime.
pub fn stream_simple_mistral(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let mut options = options.unwrap_or_default();
    options.common.api_key = options
        .common
        .api_key
        .filter(|key| !key.is_empty())
        .or_else(|| crate::get_env_api_key(model.provider.as_str()));
    if options.common.api_key.is_none() {
        return Err(DiagnosticErrorInfo {
            message: format!("No API key for provider: {}", model.provider),
            name: None,
            stack: None,
            code: None,
        });
    }
    let (raw, effort) = request::simple_options(&model, &options);
    Ok(start(model, context, raw, effort))
}

/// Own the producer separately from its returned reader.
fn start(
    model: Model,
    context: Context,
    options: MistralOptions,
    effort: Option<String>,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(initial_message(&model)));
    let producer = stream.clone();
    let message = Arc::clone(&output);
    if !spawn_detached(async move {
        let model = Arc::new(model);
        let result = async {
            let prepared =
                request::prepare(Arc::clone(&model), &context, &options, effort.as_deref()).await?;
            let body = request::send(prepared).await?;
            produce(&producer, &message, &model, body, &options).await
        }
        .await;
        if let Err(error) = result {
            fail(&producer, &message, options.common.signal.as_ref(), error);
        }
    }) {
        fail(
            &stream,
            &output,
            None,
            RequestFailure::new("Streaming requires a running Tokio runtime."),
        );
    }
    stream
}

/// Admit the response before publishing its first update.
async fn produce(
    stream: &AssistantMessageEventStream,
    output: &crate::SharedAssistantMessage,
    model: &Model,
    body: crate::HttpBody,
    options: &MistralOptions,
) -> Result<(), RequestFailure> {
    stream.push(AssistantMessageEvent::Start {
        partial: Arc::clone(output),
    });
    super::sse::read(body, model, stream, output).await?;
    if options
        .common
        .signal
        .as_ref()
        .is_some_and(crate::Cancellation::is_aborted)
    {
        return Err(RequestFailure::new("Request was aborted"));
    }
    let reason = match output
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .stop_reason
    {
        StopReason::Length => DoneReason::Length,
        StopReason::ToolUse => DoneReason::ToolUse,
        StopReason::Error | StopReason::Aborted => {
            return Err(RequestFailure::new("An unknown error occurred"));
        }
        StopReason::Stop => DoneReason::Stop,
    };
    stream.push(AssistantMessageEvent::Done {
        reason,
        message: Arc::clone(output),
    });
    stream.end(None);
    Ok(())
}
