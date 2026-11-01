//! Model invocation types with retained adapter hooks.
pub use maestro_request::types::*;
/// Closed ownership and future representations.
mod execution;
pub use execution::{Execution, Local, Threaded};
mod options;
use crate::Cancellation;
use crate::records::diagnostics::DiagnosticErrorInfo;
pub use crate::records::event_stream::AssistantMessageEventStream;
pub(crate) use options::{FunctionName, FunctionTag, Literal};
pub use options::{
    ProviderObjects, ProviderStreamOptions, SimpleStreamOptions, StreamOptions, StreamOptionsInput,
    ToolChoice,
};

/// Owned native callback future.
#[cfg(not(target_arch = "wasm32"))]
pub type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;
/// Owned browser-local callback future.
#[cfg(target_arch = "wasm32")]
pub type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'static>>;
/// Shared native callback or retained operation handle.
#[cfg(not(target_arch = "wasm32"))]
pub type SharedHandle<T> = std::sync::Arc<T>;
/// Shared browser-local callback or retained operation handle.
#[cfg(target_arch = "wasm32")]
pub type SharedHandle<T> = std::rc::Rc<T>;
/// Provider payload with its selectable request cancellation handle.
pub struct PayloadHookInput {
    /// JSON parameters selected for projection and dispatch.
    pub payload: serde_json::Value,
    /// Request cancellation; only the content-generation adapter consumes this handle.
    pub signal: Option<Cancellation>,
}
/// Invocation-associated callables and original-option envelopes.
mod callable;
pub use callable::{
    Invoke, ProviderCall, ProviderInvocation, RegistryCapture, StreamCallback, StreamOptionsFamily,
};
/// Request hooks and producer submission representations.
mod provider_execution;
pub(crate) use provider_execution::admitted;
pub use provider_execution::{OnPayload, OnResponse, ProviderExecution, ProviderSpawner};
/// Retained adapter callable in either invocation family.
pub type StreamFunction<K = ProviderStreamOptions> = SharedHandle<dyn StreamCallback<K>>;
