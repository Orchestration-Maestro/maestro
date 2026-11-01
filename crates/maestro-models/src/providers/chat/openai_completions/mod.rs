#![doc = include_str!("../../../../../../docs/models/chat-completions.md")]

mod chunk;
mod compat;
mod events;
pub mod messages;
mod payload;
mod request;
mod sse;
mod usage;

use std::borrow::Cow;
use std::sync::{Arc, RwLock};

pub use compat::ResolvedOpenAICompletionsCompat;

use crate::options::resolve_api_key;
use crate::providers::assistant_output::{fail, initial_message, start_producer_for};
use crate::providers::http::{HttpRequest, HttpResponse, RequestFailure, endpoint_url, send};
use crate::providers::json_text::compact_json;
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Cancellation, Context, DiagnosticErrorInfo,
    Local, Model, ProviderExecution, ProviderInvocation, ProviderResponse, ProviderSpawner,
    SharedAssistantMessage, SimpleStreamOptions, StopReason, StreamOptions, StreamOptionsFamily,
    ThinkingLevel, Threaded, ToolChoice, build_base_options_for, clamp_thinking_level,
};
use events::Reducer;
use request::{ENDPOINT_PATH, Invocation};
pub(crate) use request::{level_name, scope_headers};
use sse::consume;

/// Text of the failure that ends an aborted invocation.
const ABORTED_TEXT: &str = "Request was aborted";

/// Options for a direct chat-completion invocation.
#[derive(Clone, Default)]
pub struct OpenAICompletionsOptions<E: ProviderExecution = Threaded> {
    /// Settings common to every protocol.
    pub common: StreamOptions<E>,
    /// Whether the model may call tools, must call one, or must call a named one.
    pub tool_choice: Option<ToolChoice>,
    /// Requested reasoning effort. A reasoning model asked for none is told to turn reasoning
    /// off where the endpoint's convention has an explicit switch or the model maps `off`;
    /// a model that does not reason never receives reasoning fields.
    pub reasoning_effort: Option<ThinkingLevel>,
}

/// Start a streamed completion and return its updates at once; the work continues if the
/// returned stream is dropped.
///
/// Every failure the call detects, including a missing key, ends the stream with an error
/// update. On native targets the call must run inside a Tokio runtime with the time driver
/// enabled, and with the I/O driver when the default HTTP client is used. A runtime without a
/// driver the request needs is not detected: Tokio panics inside the request work and the
/// stream never ends.
#[must_use]
pub fn stream_openai_completions(
    model: Model,
    context: Context,
    options: Option<OpenAICompletionsOptions>,
) -> AssistantMessageEventStream {
    dispatch(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start a chat completion using the caller's producer admission and hook family.
#[must_use]
pub fn stream_openai_completions_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<OpenAICompletionsOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    dispatch(model, context, E::submission(options, E::provided(submit)))
}

impl StreamOptionsFamily for OpenAICompletionsOptions {
    type Options<E: ProviderExecution> = OpenAICompletionsOptions<E>;
}

/// Select concrete future erasure before starting the shared producer.
pub(crate) fn dispatch(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<OpenAICompletionsOptions>,
) -> AssistantMessageEventStream {
    invocation.dispatch(model, context, produce::<Local>, produce::<Threaded>)
}

/// Box the same producer with this concrete family's future bounds.
trait Producer: ProviderExecution {
    /// Erase a producer future without changing its work or admission.
    fn drive(
        model: Arc<Model>,
        context: Context,
        options: OpenAICompletionsOptions<Self>,
        stream: AssistantMessageEventStream,
        output: SharedAssistantMessage,
    ) -> Self::Future<'static, ()>;
}

/// Only the concrete future auto-trait bounds differ.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            fn drive(
                model: Arc<Model>,
                context: Context,
                options: OpenAICompletionsOptions<Self>,
                stream: AssistantMessageEventStream,
                output: SharedAssistantMessage,
            ) -> Self::Future<'static, ()> {
                Box::pin(run::<Self>(model, context, options, stream, output))
            }
        }
    };
}
producer!(Local);
producer!(Threaded);

/// Prepare one retained invocation and submit its producer once.
fn produce<E: Producer>(
    model: Model,
    context: Context,
    options: Option<OpenAICompletionsOptions<E>>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output: SharedAssistantMessage = Arc::new(RwLock::new(initial_message(&model)));
    let options = options.unwrap_or_default();
    let signal = options.common.signal.clone();
    let task = E::drive(
        Arc::new(model),
        context,
        options,
        stream.clone(),
        Arc::clone(&output),
    );
    start_producer_for::<E>(&stream, &output, signal.as_ref(), task, submit);
    stream
}

/// Start a streamed completion from the shared simple options.
///
/// # Errors
/// Fails at once, before any stream exists, when no API key is available for the provider.
pub fn stream_simple_openai_completions(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    simple(model, context, options, None)
}

/// Start a simple chat completion with caller-owned producer admission.
///
/// # Errors
/// Returns the existing missing-key setup failure before creating a stream.
pub fn stream_simple_openai_completions_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    // The common carrier selects the concrete family before future erasure.
    dispatch_simple(model, context, E::submission(options, E::provided(submit)))
}

/// Select concrete simple preparation while preserving the original submission.
pub(crate) fn dispatch_simple(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    invocation.dispatch(model, context, simple::<Local>, simple::<Threaded>)
}

/// Resolve simple options once before using the same retained producer.
fn simple<E: Producer>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: E::Submission,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let api_key = resolve_api_key(
        &model.provider,
        options
            .as_ref()
            .and_then(|options| options.common.api_key.as_deref())
            .map(Cow::Borrowed),
    )
    .ok_or_else(|| {
        DiagnosticErrorInfo::error(format!("No API key for provider: {}", model.provider))
    })?;
    let common = build_base_options_for(&model, &context, options.as_ref(), Some(&api_key));
    let reasoning_effort = options
        .as_ref()
        .and_then(|options| options.reasoning)
        .and_then(|level| {
            crate::reasoning_from_thinking_level(&clamp_thinking_level(&model, level.into()))
        });
    let tool_choice = options.and_then(|options| options.tool_choice);
    Ok(produce::<E>(
        model,
        context,
        Some(OpenAICompletionsOptions {
            common,
            tool_choice,
            reasoning_effort,
        }),
        submit,
    ))
}

/// Drive one invocation to its terminal update.
async fn run<E: ProviderExecution>(
    model: Arc<Model>,
    context: Context,
    options: OpenAICompletionsOptions<E>,
    stream: AssistantMessageEventStream,
    output: SharedAssistantMessage,
) {
    let outcome = invoke(&model, &context, &options, &stream, &output).await;
    if let Err(failure) = outcome {
        fail(&stream, &output, options.common.signal.as_ref(), failure);
    }
}

/// Send the request and reduce the response into the shared message; a normal end concludes
/// with a done update.
async fn invoke<E: ProviderExecution>(
    model: &Arc<Model>,
    context: &Context,
    options: &OpenAICompletionsOptions<E>,
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
) -> Result<(), RequestFailure> {
    let invocation = Invocation::new(model, context, options);
    let api_key = invocation.api_key()?;
    let base_url = invocation.base_url()?;
    let headers = invocation.headers(&api_key);
    let mut payload = invocation.payload()?;
    if let Some(hook) = &options.common.on_payload {
        payload = hook(
            crate::PayloadHookInput {
                payload,
                signal: None,
            },
            Arc::clone(model),
        )
        .await?
        .payload;
    }
    let body = compact_json(&payload).map_err(|error| RequestFailure::new(error.to_string()))?;
    let request = HttpRequest {
        method: "POST".to_owned(),
        url: endpoint_url(&base_url, ENDPOINT_PATH)?,
        headers,
        body: body.into_bytes(),
        signal: options.common.signal.clone(),
    };
    let HttpResponse {
        status,
        headers: response_headers,
        body: response_body,
        ..
    } = send(request, &options.common).await?;
    if let Some(hook) = &options.common.on_response {
        let observed = ProviderResponse {
            status: f64::from(status),
            headers: response_headers,
        };
        hook(observed, Arc::clone(model)).await?;
    }
    stream.push(AssistantMessageEvent::Start {
        partial: Arc::clone(output),
    });
    let mut reducer = Reducer::new(model, output, stream);
    consume(response_body, options.common.signal.as_ref(), &mut reducer).await?;
    reducer.finish();
    conclude(stream, output, options.common.signal.as_ref())
}

/// Report cancellation or a provider error, otherwise announce completion.
///
/// An aborted signal or a message whose stop reason is `Aborted` reports the authored
/// cancellation text; an error stop reason keeps its text unless that is empty.
fn conclude(
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
) -> Result<(), RequestFailure> {
    crate::providers::assistant_output::conclude_with(stream, output, signal, completion_error)
}

/// Preserve the chat provider's supplied text, with its cancellation and empty-text fallbacks.
fn completion_error(message: &crate::AssistantMessage) -> &str {
    if message.stop_reason == StopReason::Aborted {
        ABORTED_TEXT
    } else {
        message
            .error_message
            .as_deref()
            .filter(|text| !text.is_empty())
            .unwrap_or("Provider returned an error stop reason")
    }
}

#[cfg(test)]
mod tests;
