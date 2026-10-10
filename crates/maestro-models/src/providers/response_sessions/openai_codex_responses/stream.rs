//! Dedicated response-session invocation.

use super::{
    OpenAICodexResponsesOptions,
    http::invoke_sse,
    request::{diagnostic, prepare_request},
};
use crate::providers::assistant_output::{fail, initial_message};
use crate::providers::http::{RequestFailure, spawn_detached};
use crate::providers::responses::openai_responses::conclude;
use crate::{
    AssistantMessageEventStream, Context, DiagnosticErrorInfo, Model, ModelThinkingLevel,
    SharedAssistantMessage, SimpleStreamOptions, build_base_options, clamp_thinking_level,
    get_env_api_key,
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
    let task = run(
        Arc::new(model),
        context,
        options,
        stream.clone(),
        Arc::clone(&output),
    );
    if !spawn_detached(task) {
        fail(
            &stream,
            &output,
            signal.as_ref(),
            RequestFailure::new("Streaming requires a running Tokio runtime."),
        );
    }
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
    let key = options
        .as_ref()
        .and_then(|o| o.common.api_key.as_deref())
        .filter(|key| !key.is_empty())
        .map(Cow::Borrowed)
        .or_else(|| get_env_api_key(&model.provider).map(Cow::Owned))
        .ok_or_else(|| diagnostic(format!("No API key for provider: {}", model.provider)))?;
    let common = build_base_options(&model, options.as_ref(), Some(&key));
    let reasoning_effort = options
        .as_ref()
        .and_then(|o| o.reasoning)
        .map(|level| clamp_thinking_level(&model, level.into()))
        .filter(|level| *level != ModelThinkingLevel::Off);
    drop(options);
    Ok(stream_openai_codex_responses(
        model,
        context,
        Some(OpenAICodexResponsesOptions {
            common,
            reasoning_effort,
            ..Default::default()
        }),
    ))
}

/// Prepare once, process the selected transport, then publish the shared outcome.
async fn run(
    model: Arc<Model>,
    context: Context,
    options: OpenAICodexResponsesOptions,
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
        let _removal = crate::register_session_resource_cleanup(Arc::new(|session| {
            super::close_openai_codex_web_socket_sessions(session);
            Ok(())
        }));
    });
}

/// Attempt sockets unless explicitly bypassed or disabled by the session's retained failure.
async fn dispatch(
    prepared: &super::request::PreparedRequest,
    model: &Arc<Model>,
    options: &OpenAICodexResponsesOptions,
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
async fn invoke_socket(
    prepared: &super::request::PreparedRequest,
    model: &Arc<Model>,
    options: &OpenAICodexResponsesOptions,
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
async fn invoke_socket(
    _prepared: &super::request::PreparedRequest,
    _model: &Arc<Model>,
    _options: &OpenAICodexResponsesOptions,
    _output: (&SharedAssistantMessage, &AssistantMessageEventStream),
    _started: &mut bool,
) -> Result<(), super::CodexError> {
    Err(super::CodexError::Transport(diagnostic(
        "WebSocket transport is not available in this runtime",
    )))
}

/// Attribute a socket failure to the configured transport and the full edited request.
pub(super) fn transport_diagnostic(
    options: &OpenAICodexResponsesOptions,
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
