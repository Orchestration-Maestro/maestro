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
use std::sync::{Arc, PoisonError, RwLock};

pub use compat::ResolvedOpenAICompletionsCompat;

use crate::providers::assistant_output::{fail, initial_message};
use crate::providers::http::{
    HttpRequest, HttpResponse, RequestFailure, endpoint_url, send, spawn_detached,
};
use crate::providers::json_text::compact_json;
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Cancellation, Context, DiagnosticErrorInfo,
    DoneReason, Model, ModelThinkingLevel, ProviderResponse, SharedAssistantMessage,
    SimpleStreamOptions, StopReason, StreamOptions, ThinkingLevel, ToolChoice, build_base_options,
    clamp_thinking_level, get_env_api_key,
};
use events::Reducer;
use request::{ENDPOINT_PATH, Invocation};
use sse::consume;

/// Text of the failure that ends an aborted invocation.
const ABORTED_TEXT: &str = "Request was aborted";

/// Options for a direct chat-completion invocation.
#[derive(Clone, Default)]
pub struct OpenAICompletionsOptions {
    /// Settings common to every protocol.
    pub common: StreamOptions,
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
    let stream = AssistantMessageEventStream::new();
    let output: SharedAssistantMessage = Arc::new(RwLock::new(initial_message(&model)));
    let options = options.unwrap_or_default();
    let signal = options.common.signal.clone();
    let task = run(
        Arc::new(model),
        context,
        options,
        stream.clone(),
        Arc::clone(&output),
    );
    if !spawn_detached(task) {
        let failure = RequestFailure::new("Streaming requires a running Tokio runtime.");
        fail(&stream, &output, signal.as_ref(), failure);
    }
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
    let api_key = options
        .as_ref()
        .and_then(|options| options.common.api_key.as_deref())
        .filter(|key| !key.is_empty())
        .map(Cow::Borrowed)
        .or_else(|| get_env_api_key(&model.provider).map(Cow::Owned))
        .ok_or_else(|| DiagnosticErrorInfo {
            name: Some("Error".to_owned()),
            message: format!("No API key for provider: {}", model.provider),
            stack: None,
            code: None,
        })?;
    let common = build_base_options(&model, options.as_ref(), Some(&api_key));
    let reasoning_effort = options
        .as_ref()
        .and_then(|options| options.reasoning)
        .and_then(|level| match clamp_thinking_level(&model, level.into()) {
            ModelThinkingLevel::Off => None,
            ModelThinkingLevel::Minimal => Some(ThinkingLevel::Minimal),
            ModelThinkingLevel::Low => Some(ThinkingLevel::Low),
            ModelThinkingLevel::Medium => Some(ThinkingLevel::Medium),
            ModelThinkingLevel::High => Some(ThinkingLevel::High),
            ModelThinkingLevel::Xhigh => Some(ThinkingLevel::Xhigh),
        });
    let tool_choice = options.and_then(|options| options.tool_choice);
    Ok(stream_openai_completions(
        model,
        context,
        Some(OpenAICompletionsOptions {
            common,
            tool_choice,
            reasoning_effort,
        }),
    ))
}

/// Drive one invocation to its terminal update.
async fn run(
    model: Arc<Model>,
    context: Context,
    options: OpenAICompletionsOptions,
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
async fn invoke(
    model: &Arc<Model>,
    context: &Context,
    options: &OpenAICompletionsOptions,
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
) -> Result<(), RequestFailure> {
    let invocation = Invocation::new(model, context, options);
    let api_key = invocation.api_key()?;
    let base_url = invocation.base_url()?;
    let headers = invocation.headers(&api_key);
    let mut payload = invocation.payload()?;
    if let Some(hook) = &options.common.on_payload {
        payload = hook(payload, Arc::clone(model)).await?;
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
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(RequestFailure::new(ABORTED_TEXT));
    }
    let reason = {
        let message = output.read().unwrap_or_else(PoisonError::into_inner);
        match message.stop_reason {
            StopReason::Stop => DoneReason::Stop,
            StopReason::Length => DoneReason::Length,
            StopReason::ToolUse => DoneReason::ToolUse,
            StopReason::Aborted => return Err(RequestFailure::new(ABORTED_TEXT)),
            StopReason::Error => {
                let supplied = message
                    .error_message
                    .as_deref()
                    .filter(|text| !text.is_empty());
                return Err(RequestFailure::new(
                    supplied.unwrap_or("Provider returned an error stop reason"),
                ));
            }
        }
    };
    stream.push(AssistantMessageEvent::Done {
        reason,
        message: Arc::clone(output),
    });
    stream.end(None);
    Ok(())
}
