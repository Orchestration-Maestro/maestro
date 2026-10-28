//! Immediate invocation and eager, independently observed completion.
pub use crate::builtins::get_env_api_key;

use super::api_registry::{ApiProvider, get_api_provider, get_api_stream_simple};
use super::diagnostics::DiagnosticErrorInfo;
use super::types::{
    AssistantMessageEventStream, BoxFuture, Context, Model, ProviderExecution, ProviderInvocation,
    ProviderSpawner, ProviderStreamOptions, SharedAssistantMessage, SimpleStreamOptions,
};

/// Resolve the registered provider or report the missing API.
fn resolve(api: &str) -> Result<ApiProvider, DiagnosticErrorInfo> {
    get_api_provider(api).ok_or_else(|| {
        DiagnosticErrorInfo::error(format!("No API provider registered for api: {api}"))
    })
}
/// Immediately invoke the registered raw adapter with unchanged inputs.
///
/// # Errors
/// Returns missing registration or supplied adapter setup failure immediately.
pub fn stream(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    resolve(&model.api)?
        .stream
        .call(model, context, ProviderInvocation::Threaded(options, None))
}
/// Immediately invoke the registered simple adapter with unchanged inputs.
///
/// # Errors
/// Returns missing registration or supplied adapter setup failure immediately.
pub fn stream_simple(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    resolve(&model.api)?.stream_simple.call(
        model,
        context,
        ProviderInvocation::Threaded(options, None),
    )
}
/// Start raw invocation now, returning its result observation without consuming events.
/// Setup failures are returned by the future, not by this eager function.
#[must_use]
pub fn complete(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions>,
) -> BoxFuture<Result<SharedAssistantMessage, DiagnosticErrorInfo>> {
    completion(stream(model, context, options))
}
/// Start simple invocation now, returning its independent result observation.
/// Setup failures are returned by the future, not by this eager function.
#[must_use]
pub fn complete_simple(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> BoxFuture<Result<SharedAssistantMessage, DiagnosticErrorInfo>> {
    completion(stream_simple(model, context, options))
}

/// Observe an already-started invocation without consuming its events.
fn completion(
    invocation: Result<AssistantMessageEventStream, DiagnosticErrorInfo>,
) -> BoxFuture<Result<SharedAssistantMessage, DiagnosticErrorInfo>> {
    Box::pin(async move { Ok(invocation?.result().await) })
}

/// Immediately invoke the raw adapter using the caller's admission and hook family.
///
/// # Errors
/// Returns missing registration or adapter setup failure immediately.
pub fn stream_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    resolve(&model.api)?
        .stream
        .call(model, context, E::submission(options, E::provided(submit)))
}
/// Immediately invoke the simple adapter using the caller's admission and hook family.
///
/// # Errors
/// Returns missing registration or adapter setup failure immediately.
pub fn stream_simple_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    resolve(&model.api)?.stream_simple.call(
        model,
        context,
        E::submission(options, E::provided(submit)),
    )
}
/// Start raw invocation now and return an independent result observation, thread-safe on native targets.
#[must_use]
pub fn complete_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> BoxFuture<Result<SharedAssistantMessage, DiagnosticErrorInfo>> {
    completion(stream_for::<E>(model, context, options, submit))
}
/// Start simple invocation now and return an independent result observation, thread-safe on native targets.
#[must_use]
pub fn complete_simple_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> BoxFuture<Result<SharedAssistantMessage, DiagnosticErrorInfo>> {
    completion(stream_simple_for::<E>(model, context, options, submit))
}

/// Invoke a chosen simple protocol while retaining the supplied model identity.
///
/// # Errors
/// Returns missing registration or adapter setup failure immediately.
pub fn stream_simple_via_for<E: ProviderExecution>(
    api: &str,
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    get_api_stream_simple(api)
        .ok_or_else(|| {
            DiagnosticErrorInfo::error(format!("No API provider registered for api: {api}"))
        })?
        .call(model, context, E::submission(options, E::provided(submit)))
}
