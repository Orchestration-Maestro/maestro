#![doc = include_str!("../../../../docs/models/module-linkage.md")]
use super::raw_options;
use crate::providers::assistant_output::{fail, initial_message};
use crate::providers::chat::openai_completions as chat;
use crate::providers::http::RequestFailure;
use crate::providers::messages::anthropic as messages;
use crate::providers::reasoning::mistral as conversation;
use crate::providers::responses::{azure_openai_responses as cloud, openai_responses as responses};
use crate::records::api_registry::{
    ApiProvider, ApiStreamFunction, clear_api_providers, register_api_provider,
};
use crate::{
    AssistantMessage, AssistantMessageEventStream, Context, DiagnosticErrorInfo, Model,
    ProviderStreamOptions, SimpleStreamOptions,
};
use std::sync::{Arc, RwLock};

/// Raw invocation of one protocol with its typed options.
type RawStart<T> = fn(Model, Context, Option<T>) -> AssistantMessageEventStream;
/// Root simple invocation of one protocol.
type RawSimple = fn(Model, Context, Option<SimpleStreamOptions>) -> AssistantMessageEventStream;
/// Simple invocation of one protocol whose setup can fail before a stream exists.
type SimpleStart = fn(
    Model,
    Context,
    Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo>;

/// An already failed stream that carries `failure` as its terminal update.
fn settled(message: AssistantMessage, failure: RequestFailure) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(message));
    fail(&stream, &output, None, failure);
    stream
}

/// Run a dedicated simple invocation, turning its setup failure into a settled stream.
fn settle_simple(
    start: SimpleStart,
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    let identity = initial_message(&model);
    start(model, context, options).unwrap_or_else(|error| settled(identity, error.into()))
}

/// Registered raw callback: decode the extras, then start the protocol.
fn raw_callback<T: 'static>(
    decode: fn(ProviderStreamOptions) -> Result<T, serde_json::Error>,
    start: RawStart<T>,
) -> ApiStreamFunction {
    Arc::new(move |model, context, options| {
        Ok(match decode(options.unwrap_or_default()) {
            Ok(typed) => start(model, context, Some(typed)),
            Err(error) => settled(
                initial_message(&model),
                RequestFailure::new(error.to_string()),
            ),
        })
    })
}

/// One bundled protocol with its raw and simple callbacks.
fn provider<T: 'static>(
    api: &str,
    decode: fn(ProviderStreamOptions) -> Result<T, serde_json::Error>,
    raw: RawStart<T>,
    simple: RawSimple,
) -> ApiProvider {
    ApiProvider {
        api: api.to_owned(),
        stream: raw_callback(decode, raw),
        stream_simple: Arc::new(move |model, context, options| Ok(simple(model, context, options))),
    }
}

/// The bundled protocols in registration order.
pub(crate) fn built_in_providers() -> [ApiProvider; 5] {
    [
        provider(
            "anthropic-messages",
            raw_options::anthropic,
            messages::stream_anthropic,
            stream_simple_anthropic,
        ),
        provider(
            "openai-completions",
            raw_options::chat,
            chat::stream_openai_completions,
            stream_simple_openai_completions,
        ),
        provider(
            "mistral-conversations",
            raw_options::conversation,
            conversation::stream_mistral,
            stream_simple_mistral,
        ),
        provider(
            "openai-responses",
            raw_options::responses,
            responses::stream_openai_responses,
            stream_simple_openai_responses,
        ),
        provider(
            "azure-openai-responses",
            raw_options::cloud_responses,
            cloud::stream_azure_openai_responses,
            stream_simple_azure_openai_responses,
        ),
    ]
}

/// Register every bundled protocol, replacing earlier entries for the same APIs in place and
/// leaving other registrations untouched.
pub fn register_built_in_api_providers() {
    for provider in built_in_providers() {
        register_api_provider(provider, None);
    }
}

/// Remove every registration, then restore the bundled protocols.
pub fn reset_api_providers() {
    clear_api_providers();
    register_built_in_api_providers();
}

/// Start a message-protocol simple invocation; a missing key ends the stream with an error.
#[must_use]
pub fn stream_simple_anthropic(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    settle_simple(messages::stream_simple_anthropic, model, context, options)
}

/// Start a chat-protocol simple invocation; a missing key ends the stream with an error.
#[must_use]
pub fn stream_simple_openai_completions(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    settle_simple(
        chat::stream_simple_openai_completions,
        model,
        context,
        options,
    )
}

/// Start a conversation-protocol simple invocation; a missing key ends the stream with an error.
#[must_use]
pub fn stream_simple_mistral(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    settle_simple(conversation::stream_simple_mistral, model, context, options)
}

/// Start a response-protocol simple invocation; a missing key ends the stream with an error.
#[must_use]
pub fn stream_simple_openai_responses(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    settle_simple(
        responses::stream_simple_openai_responses,
        model,
        context,
        options,
    )
}

/// Start a cloud-response simple invocation; a missing key ends the stream with an error.
#[must_use]
pub fn stream_simple_azure_openai_responses(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    settle_simple(
        cloud::stream_simple_azure_openai_responses,
        model,
        context,
        options,
    )
}
