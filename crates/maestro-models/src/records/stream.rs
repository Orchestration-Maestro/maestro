//! Immediate invocation and eager, independently observed completion.
pub use crate::builtins::get_env_api_key;

use super::api_registry::{ApiProvider, get_api_provider};
use super::diagnostics::DiagnosticErrorInfo;
use super::types::{
    AssistantMessageEventStream, BoxFuture, Context, Model, ProviderStreamOptions,
    SharedAssistantMessage, SimpleStreamOptions,
};

fn resolve(api: &str) -> Result<ApiProvider, DiagnosticErrorInfo> {
    get_api_provider(api).ok_or_else(|| DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: format!("No API provider registered for api: {api}"),
        stack: None,
        code: None,
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
    (resolve(&model.api)?.stream)(model, context, options)
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
    (resolve(&model.api)?.stream_simple)(model, context, options)
}
/// Start raw invocation now, returning its result observation without consuming events.
/// Setup failures are returned by the future, not by this eager function.
#[must_use]
pub fn complete(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions>,
) -> BoxFuture<Result<SharedAssistantMessage, DiagnosticErrorInfo>> {
    let invocation = stream(model, context, options);
    Box::pin(async move { Ok(invocation?.result().await) })
}
/// Start simple invocation now, returning its independent result observation.
/// Setup failures are returned by the future, not by this eager function.
#[must_use]
pub fn complete_simple(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> BoxFuture<Result<SharedAssistantMessage, DiagnosticErrorInfo>> {
    let invocation = stream_simple(model, context, options);
    Box::pin(async move { Ok(invocation?.result().await) })
}
