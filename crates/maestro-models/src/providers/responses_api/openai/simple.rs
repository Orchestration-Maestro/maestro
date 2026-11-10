//! Simple response settings resolved before invoking the shared producer.
use super::{OpenAIResponsesOptions, Producer, diagnostic, produce};
use crate::options::resolve_api_key;
use crate::{
    AssistantMessageEventStream, Context, DiagnosticErrorInfo, Local, Model, ProviderExecution,
    ProviderInvocation, ProviderSpawner, SimpleStreamOptions, Threaded, build_base_options_for,
    clamp_thinking_level,
};
use std::borrow::Cow;

/// Start a response using shared simple budgets and reasoning support.
///
/// # Errors
/// Returns a missing provider key before creating a stream.
pub fn stream_simple_openai_responses(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    simple::<Threaded>(model, context, options, None)
}

/// Start a simple response request with caller-owned producer admission.
///
/// # Errors
/// Returns the existing missing-key failure before creating a stream.
pub fn stream_simple_openai_responses_for<E: ProviderExecution>(
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

/// Resolve shared simple options before starting the same producer.
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
        .and_then(|level| {
            crate::reasoning_from_thinking_level(&clamp_thinking_level(&model, level.into()))
        });
    drop(options);
    Ok(produce::<E>(
        model,
        context,
        Some(OpenAIResponsesOptions {
            common,
            reasoning_effort,
            ..OpenAIResponsesOptions::default()
        }),
        submit,
    ))
}
