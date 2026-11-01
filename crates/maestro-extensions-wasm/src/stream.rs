//! Immediate model invocation and eager, independent completion observation.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    AssistantMessageEventStream, Context, ExtensionFuture, ExtensionResult, Model,
    ProviderStreamOptions, SharedAssistantMessage, SimpleStreamOptions,
};

/// Invoke the registered raw adapter immediately with the supplied owned operands.
///
/// # Errors
/// Returns missing registration or adapter setup failure before returning a stream.
pub fn stream(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions>,
) -> ExtensionResult<AssistantMessageEventStream> {
    crate::component_adapter::model_streams::raw(model, context, options)
}
/// Invoke the registered simple adapter immediately, without applying raw-provider defaults.
///
/// # Errors
/// Returns missing registration or adapter setup failure before returning a stream.
pub fn stream_simple(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> ExtensionResult<AssistantMessageEventStream> {
    crate::component_adapter::model_streams::simple(model, context, options)
}
/// Start raw invocation now; the returned observation reports setup errors without draining events.
#[must_use]
pub fn complete(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions>,
) -> ExtensionFuture<'static, SharedAssistantMessage> {
    completion(stream(model, context, options))
}
/// Start simple invocation now; the returned observation reports setup errors without draining events.
#[must_use]
pub fn complete_simple(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> ExtensionFuture<'static, SharedAssistantMessage> {
    completion(stream_simple(model, context, options))
}
/// Observe the already-started native result without consuming an event.
fn completion(
    stream: ExtensionResult<AssistantMessageEventStream>,
) -> ExtensionFuture<'static, SharedAssistantMessage> {
    match stream {
        Ok(stream) => stream.result(),
        Err(error) => Box::pin(std::future::ready(Err(error))),
    }
}
