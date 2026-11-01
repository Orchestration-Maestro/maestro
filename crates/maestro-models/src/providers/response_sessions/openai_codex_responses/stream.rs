//! Dedicated response-session invocation.

use super::{
    OpenAICodexResponsesOptions,
    http::invoke_sse,
    request::{diagnostic, prepare_request},
};
use crate::options::resolve_api_key;
use crate::providers::assistant_output::conclude;
use crate::providers::assistant_output::{fail, initial_message, start_producer_for};
use crate::providers::http::RequestFailure;
use crate::{
    AssistantMessageEventStream, Context, DiagnosticErrorInfo, Local, Model, ModelThinkingLevel,
    ProviderExecution, ProviderInvocation, ProviderSpawner, SharedAssistantMessage,
    SimpleStreamOptions, StreamOptionsFamily, Threaded, build_base_options_for,
    clamp_thinking_level,
};
use std::{
    borrow::Cow,
    sync::{Arc, RwLock},
};

/// Start independently produced response events; setup failures end the returned stream.
#[must_use]
pub fn stream_openai_codex_responses(
    model: Model,
    context: Context,
    options: Option<OpenAICodexResponsesOptions>,
) -> AssistantMessageEventStream {
    select(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start response sessions with caller-owned admission and hook captures.
#[must_use]
pub fn stream_openai_codex_responses_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<OpenAICodexResponsesOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    select(model, context, E::submission(options, E::provided(submit)))
}

impl StreamOptionsFamily for OpenAICodexResponsesOptions {
    type Options<E: ProviderExecution> = OpenAICodexResponsesOptions<E>;
}

/// Select concrete future bounds before entering one shared session producer.
pub(crate) fn select(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<OpenAICodexResponsesOptions>,
) -> AssistantMessageEventStream {
    invocation.dispatch(model, context, start::<Local>, start::<Threaded>)
}

/// Future erasure beside the concrete session worker.
trait Producer: ProviderExecution {
    /// Box one shared worker under this family's auto-trait bounds.
    fn drive(
        model: Arc<Model>,
        context: Context,
        options: OpenAICodexResponsesOptions<Self>,
        events: AssistantMessageEventStream,
        output: SharedAssistantMessage,
    ) -> Self::Future<'static, ()>;
}

/// Only concrete future boxing differs between families.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            fn drive(
                model: Arc<Model>,
                context: Context,
                options: OpenAICodexResponsesOptions<Self>,
                events: AssistantMessageEventStream,
                output: SharedAssistantMessage,
            ) -> Self::Future<'static, ()> {
                Box::pin(run::<Self>(model, context, options, events, output))
            }
        }
    };
}
producer!(Local);
producer!(Threaded);

/// Retain request setup and cleanup registration at call time.
fn start<E: Producer>(
    model: Model,
    context: Context,
    options: Option<OpenAICodexResponsesOptions<E>>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let mut message = initial_message(&model);
    "openai-codex-responses".clone_into(&mut message.api);
    let output = Arc::new(RwLock::new(message));
    let mut options = options.unwrap_or_default();
    options
        .common
        .transport
        .get_or_insert(crate::Transport::Auto);
    register_cleanup();
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

/// Resolve the key before starting a stream, using shared simple option conversion.
///
/// # Errors
/// Returns a missing provider key before a stream exists.
pub fn stream_simple_openai_codex_responses(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    simple::<Threaded>(model, context, options, None)
}

/// Resolve simple session options with caller-owned admission.
///
/// # Errors
/// Returns the existing missing-key diagnostic before producing a stream.
pub fn stream_simple_openai_codex_responses_for<E: ProviderExecution>(
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

/// Resolve credentials before reusing the same raw producer.
fn simple<E: Producer>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: E::Submission,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let key = resolve_api_key(
        &model.provider,
        options
            .as_ref()
            .and_then(|o| o.common.api_key.as_deref())
            .map(Cow::Borrowed),
    )
    .ok_or_else(|| diagnostic(format!("No API key for provider: {}", model.provider)))?;
    let common = build_base_options_for(&model, &context, options.as_ref(), Some(&key));
    let reasoning_effort = options
        .as_ref()
        .and_then(|o| o.reasoning)
        .map(|level| clamp_thinking_level(&model, level.into()))
        .filter(|level| *level != ModelThinkingLevel::Off);
    drop(options);
    Ok(start::<E>(
        model,
        context,
        Some(OpenAICodexResponsesOptions {
            common,
            reasoning_effort,
            ..Default::default()
        }),
        submit,
    ))
}

/// Prepare once, process the selected transport, then publish the shared outcome.
async fn run<E: ProviderExecution>(
    model: Arc<Model>,
    context: Context,
    options: OpenAICodexResponsesOptions<E>,
    events: AssistantMessageEventStream,
    output: SharedAssistantMessage,
) {
    let outcome = async {
        let prepared = prepare_request(&model, &context, &options, &super::host::user_agent())
            .await
            .map_err(|error| RequestFailure::new(error.diagnostic().to_string()))?;
        dispatch(&prepared, &model, &options, &output, &events)
            .await
            .map_err(|error| RequestFailure::new(error.diagnostic().to_string()))?;
        conclude(&events, &output, options.common.signal.as_ref())
    }
    .await;
    if let Err(error) = outcome {
        fail(&events, &output, options.common.signal.as_ref(), error);
    }
}

/// Register the capture-free socket cleanup once for this process.
fn register_cleanup() {
    static REGISTER: std::sync::Once = std::sync::Once::new();
    REGISTER.call_once(|| {
        let _removal =
            crate::register_session_resource_cleanup(crate::SharedHandle::new(|session| {
                super::close_openai_codex_web_socket_sessions(session);
                Ok(())
            }));
    });
}

/// Attempt sockets unless explicitly bypassed or disabled by the session's retained failure.
async fn dispatch<E: ProviderExecution>(
    prepared: &super::request::PreparedRequest,
    model: &Arc<Model>,
    options: &OpenAICodexResponsesOptions<E>,
    output: &SharedAssistantMessage,
    events: &AssistantMessageEventStream,
) -> Result<(), super::CodexError> {
    let session = options.common.session_id.as_deref();
    if matches!(options.common.transport, Some(crate::Transport::Sse)) {
        return invoke_sse(prepared, model, options, output, events).await;
    }
    if super::debug::is_web_socket_sse_fallback_active(session) {
        super::debug::record_web_socket_sse_fallback(session);
        return invoke_sse(prepared, model, options, output, events).await;
    }
    let mut started = false;
    match invoke_socket(prepared, model, options, (output, events), &mut started).await {
        Ok(()) => return Ok(()),
        Err(super::CodexError::Transport(error))
            if !options
                .common
                .signal
                .as_ref()
                .is_some_and(crate::Cancellation::is_aborted) =>
        {
            let diagnostic = transport_diagnostic(options, &prepared.body, started, &error)?;
            crate::append_assistant_message_diagnostic(
                &mut output
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                diagnostic,
            );
            super::debug::record_web_socket_failure(session, crate::DiagnosticInput::Error(&error));
            if started {
                return Err(super::CodexError::Transport(error));
            }
            super::debug::record_web_socket_sse_fallback(session);
        }
        Err(error) => return Err(error),
    }
    invoke_sse(prepared, model, options, output, events).await
}

/// Native socket invocation forwards the existing reducer's start witness.
#[cfg(not(target_arch = "wasm32"))]
async fn invoke_socket<E: ProviderExecution>(
    prepared: &super::request::PreparedRequest,
    model: &Arc<Model>,
    options: &OpenAICodexResponsesOptions<E>,
    output: (&SharedAssistantMessage, &AssistantMessageEventStream),
    started: &mut bool,
) -> Result<(), super::CodexError> {
    let request_id = options
        .common
        .session_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(Cow::Borrowed)
        .map_or_else(|| super::host::request_id().map(Cow::Owned), Ok)?;
    super::websocket::process_web_socket_stream(
        prepared,
        model,
        options,
        super::websocket::WebSocketOutput {
            output: output.0,
            stream: output.1,
            on_start: &mut || *started = true,
        },
        &request_id,
    )
    .await
}

/// Browsers cannot emit authenticated socket headers.
#[cfg(target_arch = "wasm32")]
fn invoke_socket<E: ProviderExecution>(
    _prepared: &super::request::PreparedRequest,
    _model: &Arc<Model>,
    _options: &OpenAICodexResponsesOptions<E>,
    _output: (&SharedAssistantMessage, &AssistantMessageEventStream),
    _started: &mut bool,
) -> std::future::Ready<Result<(), super::CodexError>> {
    std::future::ready(Err(super::CodexError::Transport(diagnostic(
        "WebSocket transport is not available in this runtime",
    ))))
}

/// Attribute a socket failure to the configured transport and the full edited request.
pub(super) fn transport_diagnostic<E: ProviderExecution>(
    options: &OpenAICodexResponsesOptions<E>,
    body: &serde_json::Value,
    started: bool,
    error: &DiagnosticErrorInfo,
) -> Result<crate::AssistantMessageDiagnostic, super::CodexError> {
    let wire = crate::providers::json_text::compact_json(body)
        .map_err(|error| diagnostic(error.to_string()))?;
    let mut details = crate::JsonObject::new();
    details.insert(
        "configuredTransport".to_owned(),
        serde_json::to_value(&options.common.transport)
            .map_err(|error| diagnostic(error.to_string()))?,
    );
    if !started {
        details.insert("fallbackTransport".to_owned(), "sse".into());
    }
    details.insert("eventsEmitted".to_owned(), started.into());
    details.insert(
        "phase".to_owned(),
        if started {
            "after_message_stream_start"
        } else {
            "before_message_stream_start"
        }
        .into(),
    );
    details.insert("requestBytes".to_owned(), wire.len().into());
    Ok(crate::create_assistant_message_diagnostic(
        "provider_transport_failure",
        crate::DiagnosticInput::Error(error),
        Some(details),
    ))
}
