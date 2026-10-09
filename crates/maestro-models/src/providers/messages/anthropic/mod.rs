#![doc = include_str!("../../../../../../docs/models/messages.md")]

mod events;
mod messages;
mod request;
mod sse;
mod tool_names;
mod wire;

use std::sync::{Arc, PoisonError, RwLock};

use crate::providers::assistant_output::{fail, initial_message};
use crate::providers::http::{
    HttpResponse, RequestFailure, envelope_failure, send_with_status_error, spawn_detached,
};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, BoxFuture, Cancellation, Context,
    DiagnosticErrorInfo, DoneReason, Model, ProviderResponse, SharedAssistantMessage,
    SimpleStreamOptions, StopReason, StreamOptions, ThinkingLevel, ToolChoice,
    adjust_max_tokens_for_thinking, build_base_options, get_env_api_key,
};
use events::Reducer;
use request::Invocation;
use serde_json::Value;
use sse::{Reading, consume};

/// Text of the failure that ends a cancelled invocation.
const ABORTED_TEXT: &str = "Request was aborted";
/// Failure of a message whose stop reason names an error or an abort.
const UNKNOWN_ERROR_TEXT: &str = "An unknown error occurred";

/// Effort supplied to adaptive thinking on a raw invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnthropicEffort {
    /// Low effort.
    Low,
    /// Medium effort.
    Medium,
    /// High effort.
    High,
    /// Extra-high effort.
    Xhigh,
    /// Maximum effort.
    Max,
}
impl AnthropicEffort {
    /// The protocol spelling.
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }
}

/// Display mode for enabled thinking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnthropicThinkingDisplay {
    /// Request summarized thinking display.
    Summarized,
    /// Request omitted thinking display.
    Omitted,
}
impl AnthropicThinkingDisplay {
    /// The protocol spelling.
    fn as_str(self) -> &'static str {
        match self {
            Self::Summarized => "summarized",
            Self::Omitted => "omitted",
        }
    }
}

/// Provenance of thinking settings resolved by the simple entry.
enum ThinkingInput {
    /// Caller-supplied raw fields.
    Raw,
    /// Adaptive effort, preserving the model's open string vocabulary.
    Effort(String),
    /// Already-adjusted budget, including zero.
    Budget(f64),
}

/// Raw fields accompanied by private simple-option provenance.
struct CallOptions {
    /// Shared protocol settings.
    raw: AnthropicOptions,
    /// Resolved thinking input.
    thinking: ThinkingInput,
}

/// Options for a direct message-protocol invocation.
#[derive(Clone, Default)]
pub struct AnthropicOptions {
    /// Settings common to every protocol.
    pub common: StreamOptions,
    /// Whether thinking is enabled; false explicitly disables it on reasoning models.
    pub thinking_enabled: Option<bool>,
    /// Raw budget; absent, zero and NaN use 1,024 tokens when budget thinking is enabled.
    pub thinking_budget_tokens: Option<f64>,
    /// Raw adaptive effort; absence emits no effort field.
    pub effort: Option<AnthropicEffort>,
    /// Enabled thinking defaults to summarized display.
    pub thinking_display: Option<AnthropicThinkingDisplay>,
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
/// update. On native targets the call must run inside a Tokio runtime. A request through the
/// shared HTTP sender needs its time driver, and the default HTTP client its I/O driver; an
/// injected client reads the body without timers and needs neither. A runtime without a driver
/// the request needs is not detected: Tokio panics inside the request work and the stream never
/// ends.
#[must_use]
pub fn stream_anthropic(
    model: Model,
    context: Context,
    options: Option<AnthropicOptions>,
) -> AssistantMessageEventStream {
    start(
        model,
        context,
        CallOptions {
            raw: options.unwrap_or_default(),
            thinking: ThinkingInput::Raw,
        },
    )
}

/// Resolve simple settings before starting the shared invocation.
///
/// # Errors
/// Fails before returning a stream when it cannot select a nonempty API key.
pub fn stream_simple_anthropic(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let mut options = options.unwrap_or_default();
    let key = options
        .common
        .api_key
        .take()
        .filter(|key| !key.is_empty())
        .or_else(|| get_env_api_key(&model.provider))
        .filter(|key| !key.is_empty())
        .ok_or_else(|| DiagnosticErrorInfo {
            name: Some("Error".to_owned()),
            message: format!("No API key for provider: {}", model.provider),
            stack: None,
            code: None,
        })?;
    let mut raw = AnthropicOptions {
        common: build_base_options(&model, Some(&options), Some(&key)),
        thinking_enabled: Some(options.reasoning.is_some()),
        ..AnthropicOptions::default()
    };
    let thinking = match options.reasoning {
        None => ThinkingInput::Raw,
        Some(level) if request::supports_adaptive_thinking(&model.id) => {
            let mapped = model
                .thinking_level_map
                .as_ref()
                .and_then(|mapping| mapping.get(&level.into()))
                .and_then(Option::as_ref);
            let fallback = match level {
                ThinkingLevel::Minimal | ThinkingLevel::Low => "low",
                ThinkingLevel::Medium => "medium",
                ThinkingLevel::High | ThinkingLevel::Xhigh => "high",
            };
            ThinkingInput::Effort(mapped.map_or_else(|| fallback.to_owned(), Clone::clone))
        }
        Some(level) => {
            let base = raw
                .common
                .max_tokens
                .filter(|n| *n != 0.0 && !n.is_nan())
                .unwrap_or(0.0);
            let adjusted = adjust_max_tokens_for_thinking(
                base,
                model.max_tokens,
                level,
                options.thinking_budgets.as_ref(),
            );
            raw.common.max_tokens = Some(adjusted.max_tokens);
            ThinkingInput::Budget(adjusted.thinking_budget)
        }
    };
    Ok(start(model, context, CallOptions { raw, thinking }))
}

/// Start the producer shared by raw and simple options.
fn start(model: Model, context: Context, options: CallOptions) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output: SharedAssistantMessage = Arc::new(RwLock::new(initial_message(&model)));
    let signal = options.raw.common.signal.clone();
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
    options: CallOptions,
    stream: AssistantMessageEventStream,
    output: SharedAssistantMessage,
) {
    let outcome = invoke(&model, &context, &options, &stream, &output).await;
    if let Err(failure) = outcome {
        fail(
            &stream,
            &output,
            options.raw.common.signal.as_ref(),
            failure,
        );
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
    call: &CallOptions,
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
) -> Result<(), RequestFailure> {
    let options = &call.raw;
    let invocation = Invocation::new(model, context, call)?;
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
        ..
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
    let tools = matches!(invocation.naming, tool_names::Naming::Subscription)
        .then(|| context.tools.as_deref().unwrap_or_default());
    let mut reducer = Reducer::new(model, output, stream, tools);
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
