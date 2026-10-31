#![doc = include_str!("../../../../../../docs/models/cloud-conversations.md")]

#[cfg(not(target_arch = "wasm32"))]
mod client;
#[cfg(not(target_arch = "wasm32"))]
mod credentials;
#[cfg(not(target_arch = "wasm32"))]
mod events;
#[cfg(not(target_arch = "wasm32"))]
mod messages;
#[cfg(not(target_arch = "wasm32"))]
mod proxy;
#[cfg(not(target_arch = "wasm32"))]
mod request;
mod thinking;
#[cfg(not(target_arch = "wasm32"))]
mod wire;

#[cfg(test)]
mod tests;

use std::sync::{Arc, RwLock};

use indexmap::IndexMap;

#[cfg(not(target_arch = "wasm32"))]
use crate::SharedAssistantMessage;
use crate::options::token_base;
#[cfg(target_arch = "wasm32")]
use crate::providers::assistant_output::fail;
use crate::providers::assistant_output::initial_message_for_api;
#[cfg(not(target_arch = "wasm32"))]
use crate::providers::assistant_output::start_producer_for;
#[cfg(target_arch = "wasm32")]
use crate::providers::http::RequestFailure;
use crate::providers::messages::anthropic::AnthropicThinkingDisplay;
use crate::{
    AssistantMessageEventStream, Context, Local, Model, ProviderCall, ProviderExecution,
    ProviderInvocation, ProviderSpawner, SimpleStreamOptions, StreamOptions, StreamOptionsFamily,
    ThinkingBudgets, ThinkingLevel, Threaded, ToolChoice, adjust_max_tokens_for_thinking,
    build_base_options_for,
};
use thinking::{is_anthropic_claude_model, supports_adaptive_thinking};

/// Thinking display of a Claude request; the same values as the message protocol's display.
pub type BedrockThinkingDisplay = AnthropicThinkingDisplay;

/// Options for a direct Bedrock conversation.
#[derive(Clone, Default)]
pub struct BedrockOptions<E: ProviderExecution = Threaded> {
    /// Settings common to every protocol.
    pub common: StreamOptions<E>,
    /// Region to use; an empty value counts as absent.
    pub region: Option<String>,
    /// Shared-config profile to read; an empty value counts as absent.
    pub profile: Option<String>,
    /// With nonempty tools, `Auto` sends `auto`, `Required` sends `any` and `Function` sends
    /// `tool` with its name; unset sends the tools without a choice. `None` sends no tool
    /// configuration, as do absent or empty tools.
    pub tool_choice: Option<ToolChoice>,
    /// Thinking level; it takes effect on a reasoning Claude model.
    pub reasoning: Option<ThinkingLevel>,
    /// Token budgets by level for fixed-budget thinking.
    pub thinking_budgets: Option<ThinkingBudgets>,
    /// Absent means the interleaved beta is requested for fixed-budget thinking.
    pub interleaved_thinking: Option<bool>,
    /// Absent means summarized, except where the government target omits the display.
    pub thinking_display: Option<BedrockThinkingDisplay>,
    /// Tags attached to the request for cost allocation, sent as written.
    pub request_metadata: Option<IndexMap<String, String>>,
    /// Bearer token that replaces request signing; an empty value counts as absent.
    pub bearer_token: Option<String>,
}

/// Start a streamed Bedrock conversation and return its updates at once; the work continues if
/// the returned stream is dropped.
///
/// Every failure the call detects ends the stream with an error update. On native targets the
/// call must run inside a Tokio runtime with its time and I/O drivers. On browser targets the
/// stream ends at once with `Bedrock is not supported in browsers.`
///
/// Output uses `bedrock-converse-stream` even if the model names another API;
/// request processing and payload hooks receive the original descriptor.
#[must_use]
pub fn stream_bedrock(
    model: Model,
    context: Context,
    options: Option<BedrockOptions>,
) -> AssistantMessageEventStream {
    dispatch(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start cloud conversation using caller-owned admission and hook captures.
#[must_use]
pub fn stream_bedrock_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<BedrockOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    dispatch(model, context, E::submission(options, E::provided(submit)))
}

impl StreamOptionsFamily for BedrockOptions {
    type Options<E: ProviderExecution> = BedrockOptions<E>;
}

/// Select concrete future bounds without changing the SDK request worker.
pub(crate) fn dispatch(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<BedrockOptions>,
) -> AssistantMessageEventStream {
    invocation.dispatch(model, context, start_call::<Local>, start_call::<Threaded>)
}

/// Resolve simple options, give a fixed-budget Claude model a budget that fits its token limit,
/// then start [`stream_bedrock`]. Only the common options, the level and the budgets are read.
#[must_use]
pub fn stream_simple_bedrock(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    dispatch_simple(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start simple cloud conversation using caller-owned admission.
#[must_use]
pub fn stream_simple_bedrock_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    dispatch_simple(model, context, E::submission(options, E::provided(submit)))
}

/// Select raw cloud controls while retaining the hook family.
fn simple_options<E: ProviderExecution>(
    model: &Model,
    context: &Context,
    options: Option<SimpleStreamOptions<E>>,
) -> BedrockOptions<E> {
    let mut raw = BedrockOptions {
        common: build_base_options_for(model, context, options.as_ref(), None),
        ..BedrockOptions::default()
    };
    if let Some(SimpleStreamOptions {
        reasoning: Some(level),
        thinking_budgets,
        ..
    }) = options
    {
        raw.reasoning = Some(level);
        raw.thinking_budgets = thinking_budgets;
        if is_anthropic_claude_model(model) && !supports_adaptive_thinking(model) {
            fit_budget(model, &mut raw, level);
        }
    }
    raw
}

/// Store [`adjust_max_tokens_for_thinking`]'s returned token limit and budget in the selected
/// level's slot, leaving the other budget slots as given.
fn fit_budget<E: ProviderExecution>(
    model: &Model,
    raw: &mut BedrockOptions<E>,
    level: ThinkingLevel,
) {
    let adjusted = adjust_max_tokens_for_thinking(
        Some(token_base(raw.common.max_tokens)),
        model.max_tokens,
        level,
        raw.thinking_budgets.as_ref(),
    );
    raw.common.max_tokens = Some(adjusted.max_tokens);
    let budgets = raw
        .thinking_budgets
        .get_or_insert_with(ThinkingBudgets::default);
    let slot = match level {
        ThinkingLevel::Minimal => &mut budgets.minimal,
        ThinkingLevel::Low => &mut budgets.low,
        ThinkingLevel::Medium => &mut budgets.medium,
        ThinkingLevel::High | ThinkingLevel::Xhigh => &mut budgets.high,
    };
    *slot = Some(adjusted.thinking_budget);
}

/// Concrete erasure stays beside the cloud worker.
trait Producer: ProviderExecution {
    /// Box the shared SDK operation under this family's future bounds.
    #[cfg(not(target_arch = "wasm32"))]
    fn drive(
        call: native::Call<Self>,
        stream: AssistantMessageEventStream,
        output: SharedAssistantMessage,
    ) -> Self::Future<'static, ()>;
}

/// Only concrete future bounds differ between the families.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            #[cfg(not(target_arch = "wasm32"))]
            fn drive(
                call: native::Call<Self>,
                stream: AssistantMessageEventStream,
                output: SharedAssistantMessage,
            ) -> Self::Future<'static, ()> {
                Box::pin(native::run::<Self>(call, stream, output))
            }
        }
    };
}
producer!(Local);
producer!(Threaded);

/// End the stream with the existing browser failure without starting SDK work.
#[cfg(target_arch = "wasm32")]
fn start<E: Producer>(call: ProviderCall<E, BedrockOptions<E>>) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(initial_message_for_api(
        &call.model,
        "bedrock-converse-stream",
    )));
    let failure = RequestFailure::new("Bedrock is not supported in browsers.");
    let signal = call.options.unwrap_or_default().common.signal;
    fail(&stream, &output, signal.as_ref(), failure);
    stream
}

/// Capture SDK setup and submit the same independently owned request once.
#[cfg(not(target_arch = "wasm32"))]
fn start<E: Producer>(call: ProviderCall<E, BedrockOptions<E>>) -> AssistantMessageEventStream {
    let ProviderCall {
        model,
        context,
        options,
        submit,
    } = call;
    let options = options.unwrap_or_default();
    let stream = AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(initial_message_for_api(
        &model,
        "bedrock-converse-stream",
    )));
    let signal = options.common.signal.clone();
    let call = native::Call {
        model: Arc::new(model),
        context,
        options,
        env: proxy::Environment::capture(),
    };
    let task = E::drive(call, stream.clone(), Arc::clone(&output));
    start_producer_for::<E>(&stream, &output, signal.as_ref(), task, submit);
    stream
}

/// The native request: payload, SDK send and event reduction.
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::sync::Arc;

    use super::BedrockOptions;
    use super::client::{Settings, connect, describe, response_headers, streamed_class};
    use super::events::Reducer;
    use super::proxy::Environment;
    use super::request::{payload, prepare};
    use crate::providers::assistant_output::conclude;
    use crate::providers::assistant_output::{callback_failure, fail};
    use crate::providers::http::{Raced, RequestFailure, race};
    use crate::{
        AssistantMessageEventStream, Cancellation, Context, Model, ProviderResponse,
        SharedAssistantMessage,
    };
    use aws_sdk_bedrockruntime::primitives::event_stream::EventReceiver;
    use aws_sdk_bedrockruntime::types::ConverseStreamOutput;
    use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError;

    /// Text of the failure that ends a cancelled request.
    const ABORTED_TEXT: &str = "Request was aborted";

    /// Everything one request needs, taken when the call was made.
    pub(super) struct Call<E: crate::ProviderExecution> {
        /// Model that was asked.
        pub(super) model: Arc<Model>,
        /// Conversation to send.
        pub(super) context: Context,
        /// Options of the call.
        pub(super) options: BedrockOptions<E>,
        /// Environment as the call saw it.
        pub(super) env: Environment,
    }

    /// Drive one request to its terminal update.
    pub(super) async fn run<E: crate::ProviderExecution>(
        call: Call<E>,
        stream: AssistantMessageEventStream,
        output: SharedAssistantMessage,
    ) {
        let signal = call.options.common.signal.clone();
        if let Err(failure) = invoke(call, &stream, &output).await {
            fail(&stream, &output, signal.as_ref(), failure);
        }
    }

    /// The next known event of the stream, or none at its end. Events of an unknown kind are
    /// skipped here, so nothing downstream sees them. The response hook waits for the first
    /// known event or the end of the stream, so a stream that fails before either never reaches
    /// the hook.
    async fn next_event(
        events: &mut EventReceiver<ConverseStreamOutput, ConverseStreamOutputError>,
        signal: Option<&Cancellation>,
    ) -> Result<Option<ConverseStreamOutput>, RequestFailure> {
        loop {
            match race(events.recv(), None, signal).await {
                Raced::Done(Ok(Some(event))) if event.is_unknown() => {}
                Raced::Done(Ok(event)) => return Ok(event),
                Raced::Done(Err(error)) => return Err(describe(&error, streamed_class)),
                Raced::Cancelled | Raced::TimedOut => {
                    return Err(RequestFailure::new(ABORTED_TEXT));
                }
            }
        }
    }

    /// Send the request and reduce the stream into the shared message; a normal end concludes
    /// with a done update.
    async fn invoke<E: crate::ProviderExecution>(
        call: Call<E>,
        stream: &AssistantMessageEventStream,
        output: &SharedAssistantMessage,
    ) -> Result<(), RequestFailure> {
        let Call {
            model,
            context,
            options,
            env,
        } = call;
        let model = &model;
        let signal = options.common.signal.as_ref();
        let mut input = payload(model, context, &options, &env)?;
        if let Some(hook) = &options.common.on_payload {
            input = hook(
                crate::PayloadHookInput {
                    payload: input,
                    signal: None,
                },
                Arc::clone(model),
            )
            .await
            .map_err(callback_failure)?
            .payload;
        }
        let prepared = prepare(input)?;
        let settings = Settings::resolve(model, &options, &env)?;
        let service = connect(&settings, &options, env).await;
        let sent = match race(service.send(prepared), None, signal).await {
            Raced::Done(sent) => sent?,
            Raced::Cancelled | Raced::TimedOut => return Err(RequestFailure::new(ABORTED_TEXT)),
        };
        let mut events = sent.stream;
        let mut next = next_event(&mut events, signal).await?;
        if let (Some(observed), Some(hook)) = (service.observed(), &options.common.on_response) {
            let seen = ProviderResponse {
                status: f64::from(observed.status),
                headers: response_headers(&observed),
            };
            hook(seen, Arc::clone(model))
                .await
                .map_err(callback_failure)?;
        }
        let mut reducer = Reducer::new(model, output, stream);
        while let Some(event) = next {
            reducer.event(&event)?;
            next = next_event(&mut events, signal).await?;
        }
        conclude(stream, output, signal)
    }
}

/// Project simple options under the selected submission without changing SDK ownership.
pub(crate) fn dispatch_simple(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    invocation.dispatch(
        model,
        context,
        simple_call::<Local>,
        simple_call::<Threaded>,
    )
}

/// Pack original raw inputs for the existing SDK producer.
fn start_call<E: Producer>(
    model: Model,
    context: Context,
    options: Option<BedrockOptions<E>>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    start::<E>(ProviderCall {
        model,
        context,
        options,
        submit,
    })
}

/// Project simple controls before the existing SDK dispatch.
fn simple_call<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    let raw = simple_options(&model, &context, options);
    dispatch(model, context, E::submission(Some(raw), submit))
}
