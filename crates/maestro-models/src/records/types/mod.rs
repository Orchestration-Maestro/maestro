//! Model invocation types with retained adapter hooks.
pub use maestro_request::types::*;
/// Closed ownership and future representations.
mod execution;
pub use execution::{Execution, Local, Threaded};
mod options;
use crate::Cancellation;
use crate::records::diagnostics::DiagnosticErrorInfo;
pub use crate::records::event_stream::AssistantMessageEventStream;
pub use options::*;

/// Owned native callback future.
#[cfg(not(target_arch = "wasm32"))]
pub type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;
/// Owned browser-local callback future.
#[cfg(target_arch = "wasm32")]
pub type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'static>>;
/// Provider payload with its selectable request cancellation handle.
pub struct PayloadHookInput {
    /// JSON parameters selected for projection and dispatch.
    pub payload: serde_json::Value,
    /// Request cancellation; only the content-generation adapter consumes this handle.
    pub signal: Option<Cancellation>,
}
/// Retained callback selecting JSON parameters and a provider-specific request signal.
///
/// The hook owns its input and returns the selected record. Content generation consumes
/// its signal; other bundled adapters supply `None` and ignore the returned signal.
/// The model is shared and read-only.
#[cfg(not(target_arch = "wasm32"))]
pub type OnPayload = std::sync::Arc<
    dyn Fn(
            PayloadHookInput,
            std::sync::Arc<Model>,
        ) -> BoxFuture<Result<PayloadHookInput, DiagnosticErrorInfo>>
        + Send
        + Sync,
>;
/// Browser-local counterpart of the payload-selection callback.
#[cfg(target_arch = "wasm32")]
pub type OnPayload = std::sync::Arc<
    dyn Fn(
        PayloadHookInput,
        std::sync::Arc<Model>,
    ) -> BoxFuture<Result<PayloadHookInput, DiagnosticErrorInfo>>,
>;
/// Retained callback observing response metadata before body consumption.
///
/// The model is shared and read-only.
#[cfg(not(target_arch = "wasm32"))]
pub type OnResponse = std::sync::Arc<
    dyn Fn(ProviderResponse, std::sync::Arc<Model>) -> BoxFuture<Result<(), DiagnosticErrorInfo>>
        + Send
        + Sync,
>;
/// Browser-local callback observing response metadata.
#[cfg(target_arch = "wasm32")]
pub type OnResponse = std::sync::Arc<
    dyn Fn(ProviderResponse, std::sync::Arc<Model>) -> BoxFuture<Result<(), DiagnosticErrorInfo>>,
>;
/// Adapter invocation with an owned model, conversation and unchanged options.
#[cfg(not(target_arch = "wasm32"))]
pub type StreamFunction<O = ProviderStreamOptions> = std::sync::Arc<
    dyn Fn(Model, Context, Option<O>) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo>
        + Send
        + Sync,
>;
/// Browser-local adapter invocation with unchanged options.
#[cfg(target_arch = "wasm32")]
pub type StreamFunction<O = ProviderStreamOptions> = std::sync::Arc<
    dyn Fn(Model, Context, Option<O>) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo>,
>;
