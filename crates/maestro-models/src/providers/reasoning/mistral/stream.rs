//! Independently produced conversation updates.
use super::{MistralOptions, request};
use crate::options::resolve_api_key;
use crate::providers::{
    assistant_output::{conclude, fail, initial_message, start_producer},
    http::RequestFailure,
};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Context, DiagnosticErrorInfo, Model,
    SimpleStreamOptions,
};
use std::borrow::Cow;
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
    options.common.api_key = resolve_api_key(
        model.provider.as_str(),
        options.common.api_key.take().map(Cow::Owned),
    )
    .map(Cow::into_owned);
    if options.common.api_key.is_none() {
        return Err(DiagnosticErrorInfo::new(
            None,
            format!("No API key for provider: {}", model.provider),
        ));
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
    start_producer(
        &stream,
        &output,
        None,
        Box::pin(async move {
            let model = Arc::new(model);
            let result = async {
                let prepared =
                    request::prepare(Arc::clone(&model), &context, &options, effort.as_deref())
                        .await?;
                let body = request::send(prepared).await?;
                produce(&producer, &message, &model, body, &options).await
            }
            .await;
            if let Err(error) = result {
                fail(&producer, &message, options.common.signal.as_ref(), error);
            }
        }),
    );
    stream
}

/// The failure a cancelled invocation reports.
pub(super) fn aborted() -> RequestFailure {
    RequestFailure::new("Request was aborted")
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
    conclude(stream, output, options.common.signal.as_ref())
}
