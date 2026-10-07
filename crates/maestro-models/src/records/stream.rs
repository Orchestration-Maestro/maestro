//! Eager invocation by the caller-supplied protocol.
use super::{
    api_registry::*,
    diagnostics::{ThrownValue, error},
    event_stream::AssistantMessageEventStream,
    types::*,
};
pub use crate::builtins::get_env_api_key;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};
#[cfg(not(target_arch = "wasm32"))]
type Completion =
    Pin<Box<dyn Future<Output = Result<Arc<RwLock<AssistantMessage>>, ThrownValue>> + Send>>;
#[cfg(target_arch = "wasm32")]
type Completion = Pin<Box<dyn Future<Output = Result<Arc<RwLock<AssistantMessage>>, ThrownValue>>>>;
fn resolve(api: &str) -> Result<ApiProviderHandle, ThrownValue> {
    get_api_provider(api).ok_or_else(|| error(format!("No API provider registered for api: {api}")))
}
/// Invoke the raw adapter once, forwarding every supplied field unchanged.
pub fn stream(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions>,
) -> Result<AssistantMessageEventStream, ThrownValue> {
    let callback = resolve(&model.api)?
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .stream
        .clone();
    callback(model, context, options)
}
/// Start raw invocation immediately, then observe its result without draining events.
pub fn complete(
    model: Model,
    context: Context,
    options: Option<ProviderStreamOptions>,
) -> Completion {
    let started = stream(model, context, options);
    Box::pin(async move { Ok(started?.result().await) })
}
/// Invoke the simple adapter once without applying defaults or resolving credentials.
pub fn stream_simple(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, ThrownValue> {
    let callback = resolve(&model.api)?
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .stream_simple
        .clone();
    callback(model, context, options)
}
/// Start simple invocation immediately, then independently observe its result.
pub fn complete_simple(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Completion {
    let started = stream_simple(model, context, options);
    Box::pin(async move { Ok(started?.result().await) })
}
