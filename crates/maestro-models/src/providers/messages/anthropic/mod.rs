#![doc = include_str!("../../../../../../docs/models/messages.md")]

mod events;
mod messages;
mod request;
mod sse;
mod tool_names;
mod wire;

use std::borrow::Cow;
use std::sync::{Arc, RwLock};

use crate::options::{fit_thinking, resolve_api_key};
use crate::providers::assistant_output::{
    callback_failure, conclude, fail, initial_message, start_producer_for,
};
use crate::providers::http::{
    HttpResponse, RequestFailure, envelope_failure, send_with_status_error,
};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, BoxFuture, Cancellation, Context,
    DiagnosticErrorInfo, Local, Model, ProviderExecution, ProviderInvocation, ProviderResponse,
    ProviderSpawner, SharedAssistantMessage, SimpleStreamOptions, StreamOptions,
    StreamOptionsFamily, ThinkingLevel, Threaded, ToolChoice, build_base_options_for,
};
use events::Reducer;
use request::Invocation;
use serde_json::Value;
use sse::{Reading, consume};

/// Text of the failure that ends a cancelled invocation.
const ABORTED_TEXT: &str = "Request was aborted";

/// Effort supplied to adaptive thinking on a raw invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnthropicThinkingDisplay {
    /// Request summarized thinking display.
    Summarized,
    /// Request omitted thinking display.
    Omitted,
}
impl AnthropicThinkingDisplay {
    /// The protocol spelling.
    pub(crate) fn as_str(self) -> &'static str {
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
struct CallOptions<E: ProviderExecution> {
    /// Shared protocol settings.
    raw: AnthropicOptions<E>,
    /// Resolved thinking input.
    thinking: ThinkingInput,
}

/// Options for a direct message-protocol invocation.
#[derive(Clone, Default)]
pub struct AnthropicOptions<E: ProviderExecution = Threaded> {
    /// Settings common to every protocol.
    pub common: StreamOptions<E>,
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
    raw_start::<Threaded>(model, context, options, None)
}

/// Start a message request with the caller's producer admission and hook family.
#[must_use]
pub fn stream_anthropic_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<AnthropicOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    dispatch(model, context, E::submission(options, E::provided(submit)))
}

/// Select concrete raw preparation without changing thinking provenance.
pub(crate) fn dispatch(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<AnthropicOptions>,
) -> AssistantMessageEventStream {
    invocation.dispatch(model, context, raw_start::<Local>, raw_start::<Threaded>)
}

impl StreamOptionsFamily for AnthropicOptions {
    type Options<E: ProviderExecution> = AnthropicOptions<E>;
}

/// Retain raw thinking provenance before starting the single producer.
fn raw_start<E: Producer>(
    model: Model,
    context: Context,
    options: Option<AnthropicOptions<E>>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    start::<E>(
        model,
        context,
        CallOptions {
            raw: options.unwrap_or_default(),
            thinking: ThinkingInput::Raw,
        },
        submit,
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
    simple::<Threaded>(model, context, options, None)
}

/// Resolve simple message settings with caller-owned admission.
///
/// # Errors
/// Returns a missing provider key before creating a stream.
pub fn stream_simple_anthropic_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
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

/// Resolve thinking and key policy once for either family.
fn simple<E: Producer>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: E::Submission,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let mut options = options.unwrap_or_default();
    let key = resolve_api_key(
        &model.provider,
        options.common.api_key.take().map(Cow::Owned),
    )
    .map(Cow::into_owned)
    .ok_or_else(|| {
        DiagnosticErrorInfo::error(format!("No API key for provider: {}", model.provider))
    })?;
    let mut raw = AnthropicOptions {
        common: build_base_options_for(&model, &context, Some(&options), Some(&key)),
        thinking_enabled: Some(options.reasoning.is_some()),
        ..AnthropicOptions::default()
    };
    let thinking = match options.reasoning {
        None => ThinkingInput::Raw,
        Some(level) if request::supports_adaptive_thinking(&model) => {
            ThinkingInput::Effort(map_thinking_level_to_effort(&model, level))
        }
        Some(level) => {
            let adjusted = fit_thinking(
                &model,
                &context,
                raw.common.max_tokens,
                level,
                options.thinking_budgets.as_ref(),
            );
            raw.common.max_tokens = Some(adjusted.max_tokens);
            ThinkingInput::Budget(adjusted.thinking_budget)
        }
    };
    Ok(start::<E>(
        model,
        context,
        CallOptions { raw, thinking },
        submit,
    ))
}

/// The effort the model's level map names for `level`, otherwise the level's own effort.
pub(crate) fn map_thinking_level_to_effort(model: &Model, level: ThinkingLevel) -> String {
    let mapped = model
        .thinking_level_map
        .as_ref()
        .and_then(|mapping| mapping.get(&crate::ModelThinkingLevel::from(level)))
        .and_then(Option::as_ref);
    mapped.map_or_else(
        || {
            match level {
                ThinkingLevel::Minimal | ThinkingLevel::Low => "low",
                ThinkingLevel::Medium => "medium",
                ThinkingLevel::High | ThinkingLevel::Xhigh => "high",
            }
            .to_owned()
        },
        Clone::clone,
    )
}

/// Box the same producer with this concrete family's future bounds.
trait Producer: ProviderExecution {
    /// Erase a producer future without changing its work or admission.
    fn drive(
        model: Arc<Model>,
        context: Context,
        options: CallOptions<Self>,
        stream: AssistantMessageEventStream,
        output: SharedAssistantMessage,
    ) -> Self::Future<'static, ()>;
}

/// Only concrete future auto-trait bounds differ.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            fn drive(
                model: Arc<Model>,
                context: Context,
                options: CallOptions<Self>,
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

/// Start the producer shared by raw and simple options.
fn start<E: Producer>(
    model: Model,
    context: Context,
    options: CallOptions<E>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output: SharedAssistantMessage = Arc::new(RwLock::new(initial_message(&model)));
    let signal = options.raw.common.signal.clone();
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

/// Drive one invocation to its terminal update.
async fn run<E: ProviderExecution>(
    model: Arc<Model>,
    context: Context,
    options: CallOptions<E>,
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

/// Send the request and reduce the response into the shared message; a normal end concludes
/// with a done update.
async fn invoke<E: ProviderExecution>(
    model: &Arc<Model>,
    context: &Context,
    call: &CallOptions<E>,
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
) -> Result<(), RequestFailure> {
    let options = &call.raw;
    let invocation = Invocation::new(model, context, call)?;
    let mut payload = invocation.payload()?;
    if let Some(hook) = &options.common.on_payload {
        payload = hook(
            crate::PayloadHookInput {
                payload,
                signal: None,
            },
            Arc::clone(model),
        )
        .await
        .map_err(callback_failure)?
        .payload;
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
