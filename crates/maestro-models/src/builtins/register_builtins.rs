#![doc = include_str!("../../../../docs/models/module-linkage.md")]
use super::raw_options;
use crate::SharedHandle;
use crate::providers::assistant_output::{fail, initial_message};
use crate::providers::chat::openai_completions as chat;
use crate::providers::cloud::amazon_bedrock as bedrock;
use crate::providers::content::{google as content, google_vertex as cloud_content};
use crate::providers::http::RequestFailure;
use crate::providers::messages::anthropic as messages;
use crate::providers::reasoning::mistral as conversation;
use crate::providers::response_sessions::openai_codex_responses as sessions;
use crate::providers::responses::{azure_openai_responses as cloud, openai_responses as responses};
use crate::records::api_registry::{
    ApiProvider, ApiStreamFunction, ApiStreamSimpleFunction, clear_api_providers,
    register_api_provider,
};
use crate::{
    AssistantMessage, AssistantMessageEventStream, Context, DiagnosticErrorInfo, Invoke, Model,
    ProviderCall, ProviderExecution, ProviderSpawner, ProviderStreamOptions, SimpleStreamOptions,
};
use sessions::stream_simple_openai_codex_responses as session_simple;
use std::sync::{Arc, RwLock};

/// An already failed stream that carries `failure` as its terminal update.
fn settled(message: AssistantMessage, failure: RequestFailure) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(message));
    fail(&stream, &output, None, failure);
    stream
}

/// Run a dedicated simple invocation, turning its setup failure into a settled stream.
fn settle_simple<E: ProviderExecution>(
    start: impl FnOnce(
        Model,
        Context,
        Option<SimpleStreamOptions<E>>,
    ) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo>,
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
) -> AssistantMessageEventStream {
    let identity = initial_message(&model);
    start(model, context, options).unwrap_or_else(|error| settled(identity, error.into()))
}

/// Concrete bundled owners share one projection and their existing protocol selectors.
macro_rules! provider {
    ($owner:ident, $decode:path, $raw:path, $simple:expr) => {
        /// Retained raw and simple entry points for one bundled protocol.
        pub(crate) struct $owner;
        impl<E: ProviderExecution> Invoke<E, ProviderCall<E, ProviderStreamOptions<E>>> for $owner {
            type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
            fn invoke(&self, call: ProviderCall<E, ProviderStreamOptions<E>>) -> Self::Return {
                let ProviderCall {
                    model,
                    context,
                    options,
                    submit,
                } = call;
                Ok(match $decode(options.unwrap_or_default()) {
                    Ok(typed) => $raw(model, context, E::submission(Some(typed), submit)),
                    Err(error) => settled(
                        initial_message(&model),
                        RequestFailure::new(error.to_string()),
                    ),
                })
            }
        }
        impl<E: ProviderExecution> Invoke<E, ProviderCall<E, SimpleStreamOptions<E>>> for $owner {
            type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
            fn invoke(&self, call: ProviderCall<E, SimpleStreamOptions<E>>) -> Self::Return {
                ($simple)(
                    call.model,
                    call.context,
                    E::submission(call.options, call.submit),
                )
            }
        }
    };
}
provider!(
    Messages,
    raw_options::anthropic,
    messages::dispatch,
    messages::dispatch_simple
);
provider!(
    Chat,
    raw_options::chat,
    chat::dispatch,
    chat::dispatch_simple
);
provider!(
    Conversation,
    raw_options::conversation,
    conversation::dispatch,
    conversation::dispatch_simple
);
provider!(
    Responses,
    raw_options::responses,
    responses::dispatch,
    responses::dispatch_simple
);
provider!(
    CloudResponses,
    raw_options::cloud_responses,
    cloud::dispatch,
    cloud::dispatch_simple
);
provider!(
    Sessions,
    raw_options::response_sessions,
    sessions::select,
    sessions::dispatch_simple
);
provider!(
    Content,
    raw_options::google,
    content::dispatch,
    content::dispatch_simple
);
provider!(
    Bedrock,
    raw_options::bedrock,
    bedrock::dispatch,
    |model, context, invocation| Ok(bedrock::dispatch_simple(model, context, invocation))
);

provider!(
    CloudContent,
    raw_options::google_vertex,
    cloud_content::dispatch,
    |model, context, invocation| Ok(cloud_content::dispatch_simple(model, context, invocation))
);

/// One protocol whose raw and simple fields retain the same owner.
fn provider<T>(api: &str, owner: SharedHandle<T>) -> ApiProvider
where
    T: crate::StreamCallback<ProviderStreamOptions>
        + crate::StreamCallback<SimpleStreamOptions>
        + 'static,
{
    ApiProvider {
        api: api.into(),
        stream: owner.clone(),
        stream_simple: owner,
    }
}

/// The bundled protocols in registration order.
pub(crate) fn built_in_providers() -> [ApiProvider; 9] {
    [
        provider("anthropic-messages", SharedHandle::new(Messages)),
        provider("openai-completions", SharedHandle::new(Chat)),
        provider("mistral-conversations", SharedHandle::new(Conversation)),
        provider("openai-responses", SharedHandle::new(Responses)),
        provider("azure-openai-responses", SharedHandle::new(CloudResponses)),
        provider("openai-codex-responses", SharedHandle::new(Sessions)),
        provider("google-generative-ai", SharedHandle::new(Content)),
        provider("google-vertex", SharedHandle::new(CloudContent)),
        provider(
            "bedrock-converse-stream",
            SharedHandle::new(SelectedBedrock),
        ),
    ]
}

/// Snapshot the replaceable cloud callbacks only when invocation begins.
struct SelectedBedrock;
/// Both option families use the same callback-selection and setup-error boundary.
macro_rules! selected_bedrock {
    ($options:ident, $field:ident) => {
        impl<E: ProviderExecution> Invoke<E, ProviderCall<E, $options<E>>> for SelectedBedrock {
            type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
            fn invoke(&self, call: ProviderCall<E, $options<E>>) -> Self::Return {
                let callback = selected_bedrock().$field;
                let identity = initial_message(&call.model);
                Ok(callback
                    .call(
                        call.model,
                        call.context,
                        E::submission(call.options, call.submit),
                    )
                    .unwrap_or_else(|error| settled(identity, error.into())))
            }
        }
    };
}
selected_bedrock!(ProviderStreamOptions, stream_bedrock);
selected_bedrock!(SimpleStreamOptions, stream_simple_bedrock);

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

/// Start a simple message invocation with caller-owned admission, settling setup errors.
#[must_use]
pub fn stream_simple_anthropic_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    settle_simple(
        move |model, context, options| {
            messages::stream_simple_anthropic_for::<E>(model, context, options, submit)
        },
        model,
        context,
        options,
    )
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

/// Start a simple chat invocation with caller-owned admission, settling setup errors.
#[must_use]
pub fn stream_simple_openai_completions_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    settle_simple(
        move |model, context, options| {
            chat::stream_simple_openai_completions_for::<E>(model, context, options, submit)
        },
        model,
        context,
        options,
    )
}

/// Start simple response sessions with caller-owned admission, settling setup errors.
#[must_use]
pub fn stream_simple_openai_codex_responses_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    settle_simple(
        move |model, context, options| {
            sessions::stream_simple_openai_codex_responses_for::<E>(model, context, options, submit)
        },
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

/// Start a simple conversation with caller-owned producer admission.
#[must_use]
pub fn stream_simple_mistral_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    settle_simple(
        move |model, context, options| {
            conversation::stream_simple_mistral_for::<E>(model, context, options, submit)
        },
        model,
        context,
        options,
    )
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

/// Start a response-session simple invocation; a missing key ends the stream with an error.
#[must_use]
pub fn stream_simple_openai_codex_responses(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    settle_simple(session_simple, model, context, options)
}

/// Replaceable raw and simple cloud-conversation callbacks.
#[derive(Clone)]
pub struct BedrockProviderModule {
    /// Raw invocation, receiving open options unchanged.
    pub stream_bedrock: ApiStreamFunction,
    /// Simple invocation, receiving its options unchanged.
    pub stream_simple_bedrock: ApiStreamSimpleFunction,
}

/// Native callback override, independent of registry reset.
#[cfg(not(target_arch = "wasm32"))]
static BEDROCK: std::sync::Mutex<Option<BedrockProviderModule>> = std::sync::Mutex::new(None);
#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Browser-local callback override.
    static BEDROCK: std::cell::RefCell<Option<BedrockProviderModule>> = const { std::cell::RefCell::new(None) };
}

crate::records::platform_store_accessor!(
    /// Access the selected slot only to snapshot or replace callback handles.
    with_bedrock, BEDROCK, Option<BedrockProviderModule>
);

/// Select the current pair, releasing the slot before invocation.
fn selected_bedrock() -> BedrockProviderModule {
    with_bedrock(|slot| slot.clone())
        .unwrap_or_else(crate::bedrock_provider::bedrock_provider_module)
}

/// Replace the callbacks used by subsequent bundled cloud-conversation invocations.
///
/// Already started calls keep their selected callbacks. Registry resets do not clear this pair.
pub fn set_bedrock_provider_module(module: BedrockProviderModule) {
    let retired = with_bedrock(|slot| slot.replace(module));
    drop(retired);
}

/// Start simple content generation, settling missing-key failures into the stream.
#[must_use]
pub fn stream_simple_google(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    settle_simple(content::stream_simple_google, model, context, options)
}

/// Start simple content generation using caller-owned admission, settling setup errors.
#[must_use]
pub fn stream_simple_google_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    settle_simple(
        move |model, context, options| {
            content::stream_simple_google_for::<E>(model, context, options, submit)
        },
        model,
        context,
        options,
    )
}

/// Start a simple response invocation with caller-owned admission, settling setup errors.
#[must_use]
pub fn stream_simple_openai_responses_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    settle_simple(
        move |model, context, options| {
            responses::stream_simple_openai_responses_for::<E>(model, context, options, submit)
        },
        model,
        context,
        options,
    )
}

/// Start a simple response invocation with caller-owned admission, settling setup errors.
#[must_use]
pub fn stream_simple_azure_openai_responses_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    settle_simple(
        move |model, context, options| {
            cloud::stream_simple_azure_openai_responses_for::<E>(model, context, options, submit)
        },
        model,
        context,
        options,
    )
}
