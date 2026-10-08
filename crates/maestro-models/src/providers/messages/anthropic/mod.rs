#![doc = include_str!("../../../../../../docs/models/messages.md")]

mod events;
mod messages;
mod request;
mod sse;
mod wire;

use std::sync::{Arc, PoisonError, RwLock};

use crate::providers::assistant_output::{fail, initial_message};
use crate::providers::http::{
    HttpResponse, RequestFailure, envelope_failure, send_with_status_error, spawn_detached,
};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, BoxFuture, Cancellation, Context,
    DiagnosticErrorInfo, DoneReason, Model, ProviderResponse, SharedAssistantMessage, StopReason,
    StreamOptions, ToolChoice,
};
use events::Reducer;
use request::Invocation;
use serde_json::Value;
use sse::{Reading, consume};

/// Text of the failure that ends a cancelled invocation.
const ABORTED_TEXT: &str = "Request was aborted";
/// Failure of a message whose stop reason names an error or an abort.
const UNKNOWN_ERROR_TEXT: &str = "An unknown error occurred";

/// Options for a direct message-protocol invocation.
#[derive(Clone, Default)]
pub struct AnthropicOptions {
    /// Settings common to every protocol.
    pub common: StreamOptions,
    /// Whether the interleaved-reasoning beta may be requested; absent means it may. It is never
    /// requested for models whose reasoning is adaptive.
    pub interleaved_thinking: Option<bool>,
    /// Whether the model may call tools, must call one, or must call a named one.
    pub tool_choice: Option<ToolChoice>,
    /// A client that sends the request instead of the default HTTP transport.
    pub client: Option<AnthropicClient>,
}

/// The settings an injected client receives for one request.
#[derive(Clone, Default)]
pub struct AnthropicRequestOptions {
    /// Signal the call was given.
    pub signal: Option<Cancellation>,
    /// Setup timeout the call was given, in milliseconds.
    pub timeout_ms: Option<f64>,
    /// Retry count the call was given.
    pub max_retries: Option<f64>,
}

/// A client that sends the request payload and returns the accepted response.
///
/// It owns the request, its authentication, its retries and its status handling: a response
/// it returns is read as an event stream whatever its status, and a failure it reports ends
/// the call with the failure's `message`, even when that is empty.
#[cfg(not(target_arch = "wasm32"))]
pub type AnthropicClient = Arc<
    dyn Fn(Value, AnthropicRequestOptions) -> BoxFuture<Result<HttpResponse, DiagnosticErrorInfo>>
        + Send
        + Sync,
>;
/// A client that sends the request payload and returns the accepted response.
#[cfg(target_arch = "wasm32")]
pub type AnthropicClient = Arc<
    dyn Fn(Value, AnthropicRequestOptions) -> BoxFuture<Result<HttpResponse, DiagnosticErrorInfo>>,
>;

/// Start a streamed message request and return its updates at once; the work continues if the
/// returned stream is dropped.
///
/// Every failure the call detects, including a missing key, ends the stream with an error
/// update. On native targets the call must run inside a Tokio runtime with the time driver
/// enabled, and with the I/O driver when the default HTTP client is used. A runtime without a
/// driver the request needs is not detected: Tokio panics inside the request work and the
/// stream never ends.
#[must_use]
pub fn stream_anthropic(
    model: Model,
    context: Context,
    options: Option<AnthropicOptions>,
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

/// Drive one invocation to its terminal update.
async fn run(
    model: Arc<Model>,
    context: Context,
    options: AnthropicOptions,
    stream: AssistantMessageEventStream,
    output: SharedAssistantMessage,
) {
    let outcome = invoke(&model, &context, &options, &stream, &output).await;
    if let Err(failure) = outcome {
        fail(&stream, &output, options.common.signal.as_ref(), failure);
    }
}

/// The failure a callback reports, with its message as it is.
fn callback_failure(error: DiagnosticErrorInfo) -> RequestFailure {
    RequestFailure::new(error.message)
}

/// Send the request and reduce the response into the shared message; a normal end concludes
/// with a done update.
async fn invoke(
    model: &Arc<Model>,
    context: &Context,
    options: &AnthropicOptions,
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
) -> Result<(), RequestFailure> {
    let invocation = Invocation::new(model, context, options);
    let mut payload = invocation.payload()?;
    if let Some(hook) = &options.common.on_payload {
        payload = hook(payload, Arc::clone(model))
            .await
            .map_err(callback_failure)?;
    }
    match &mut payload {
        Value::Object(members) => {
            members.insert("stream".to_owned(), Value::Bool(true));
        }
        other => *other = serde_json::json!({"stream": true}),
    }
    let (response, reading) = if let Some(client) = &options.client {
        let settings = AnthropicRequestOptions {
            signal: options.common.signal.clone(),
            timeout_ms: options.common.timeout_ms,
            max_retries: options.common.max_retries,
        };
        let response = client(payload, settings).await.map_err(callback_failure)?;
        (response, Reading::Injected)
    } else {
        let request = invocation.request(&payload)?;
        let response = send_with_status_error(request, &options.common, envelope_failure).await?;
        (response, Reading::Transport)
    };
    let HttpResponse {
        status,
        headers,
        body,
    } = response;
    if let Some(hook) = &options.common.on_response {
        let observed = ProviderResponse {
            status: f64::from(status),
            headers,
        };
        hook(observed, Arc::clone(model))
            .await
            .map_err(callback_failure)?;
    }
    stream.push(AssistantMessageEvent::Start {
        partial: Arc::clone(output),
    });
    let mut reducer = Reducer::new(model, output, stream);
    consume(body, options.common.signal.as_ref(), reading, &mut reducer).await?;
    conclude(stream, output, options.common.signal.as_ref())
}

/// Report cancellation or an error outcome, otherwise announce completion.
///
/// An aborted signal reports the cancellation text; a stop reason of `error` or `aborted`
/// reports an unknown error.
fn conclude(
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
) -> Result<(), RequestFailure> {
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(RequestFailure::new(ABORTED_TEXT));
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
            return Err(RequestFailure::new(UNKNOWN_ERROR_TEXT));
        }
    };
    stream.push(AssistantMessageEvent::Done {
        reason,
        message: Arc::clone(output),
    });
    stream.end(None);
    Ok(())
}
