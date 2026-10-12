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

#[cfg(test)]
mod tests;

use std::sync::{Arc, RwLock};

use indexmap::IndexMap;

use crate::providers::assistant_output::{fail, initial_message};
use crate::providers::http::RequestFailure;
use crate::{AssistantMessageEventStream, Context, Model, SharedAssistantMessage, StreamOptions};

/// Options for a direct Bedrock conversation.
#[derive(Clone, Default)]
pub struct BedrockOptions {
    /// Settings common to every protocol.
    pub common: StreamOptions,
    /// Region to use; an empty value counts as absent.
    pub region: Option<String>,
    /// Shared-config profile to read; an empty value counts as absent.
    pub profile: Option<String>,
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
#[must_use]
pub fn stream_bedrock(
    model: Model,
    context: Context,
    options: Option<BedrockOptions>,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output: SharedAssistantMessage = Arc::new(RwLock::new(initial_message(&model)));
    let options = options.unwrap_or_default();
    start(model, context, options, &stream, &output);
    stream
}

/// End the stream with the browser failure.
#[cfg(target_arch = "wasm32")]
fn start(
    _model: Model,
    _context: Context,
    options: BedrockOptions,
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
) {
    let failure = RequestFailure::new("Bedrock is not supported in browsers.");
    fail(stream, output, options.common.signal.as_ref(), failure);
}

/// Run the request on the runtime, or end the stream when there is none.
#[cfg(not(target_arch = "wasm32"))]
fn start(
    model: Model,
    context: Context,
    options: BedrockOptions,
    stream: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
) {
    let signal = options.common.signal.clone();
    let call = native::Call {
        model: Arc::new(model),
        context,
        options,
        env: proxy::Environment::capture(),
    };
    let task = native::run(call, stream.clone(), Arc::clone(output));
    if !crate::providers::http::spawn_detached(task) {
        let failure = RequestFailure::new("Streaming requires a running Tokio runtime.");
        fail(stream, output, signal.as_ref(), failure);
    }
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
    use crate::providers::assistant_output::{callback_failure, fail};
    use crate::providers::http::{Raced, RequestFailure, race};
    use crate::providers::responses::openai_responses::conclude;
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
    pub(super) struct Call {
        /// Model that was asked.
        pub(super) model: Arc<Model>,
        /// Conversation to send.
        pub(super) context: Context,
        /// Options of the call.
        pub(super) options: BedrockOptions,
        /// Environment as the call saw it.
        pub(super) env: Environment,
    }

    /// Drive one request to its terminal update.
    pub(super) async fn run(
        call: Call,
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
    async fn invoke(
        call: Call,
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
        let mut input = payload(model, &context, &options)?;
        if let Some(hook) = &options.common.on_payload {
            input = hook(input, Arc::clone(model))
                .await
                .map_err(callback_failure)?;
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
