//! Wire records for model invocations and conversation events.
mod messages;
pub use messages::*;

/// Open model protocol identifier.
pub type Api = String;
/// Extensible model protocol identifier.
pub type KnownApi = Api;
/// Open model provider identifier.
pub type Provider = String;
/// Extensible model provider identifier.
pub type KnownProvider = Provider;
/// Open JSON object for schemas, arguments and metadata.
pub type JsonObject = serde_json::Map<String, serde_json::Value>;

/// Preserve an explicitly present value, including null, as an outer option.
pub(super) fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

mod models;
mod options;
use crate::Cancellation;
use crate::records::diagnostics::DiagnosticErrorInfo;
pub use crate::records::event_stream::AssistantMessageEventStream;
pub use models::*;
pub use options::*;

/// Provider reasoning names, with explicit null for unsupported choices.
pub type ThinkingLevelMap = std::collections::BTreeMap<ModelThinkingLevel, Option<String>>;
/// Retained live assistant message shared by partial and terminal observations.
pub type SharedAssistantMessage = std::sync::Arc<std::sync::RwLock<AssistantMessage>>;
/// Owned native callback future.
#[cfg(not(target_arch = "wasm32"))]
pub type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;
/// Owned browser-local callback future.
#[cfg(target_arch = "wasm32")]
pub type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'static>>;
/// Retained callback that receives the adapter payload and returns the payload to send.
///
/// The hook owns the payload it receives: returning it, edited or not, sends it; returning
/// another value sends that value instead. The model is shared and read-only.
#[cfg(not(target_arch = "wasm32"))]
pub type OnPayload = std::sync::Arc<
    dyn Fn(
            serde_json::Value,
            std::sync::Arc<Model>,
        ) -> BoxFuture<Result<serde_json::Value, DiagnosticErrorInfo>>
        + Send
        + Sync,
>;
/// Browser-local callback that receives the adapter payload and returns the payload to send.
#[cfg(target_arch = "wasm32")]
pub type OnPayload = std::sync::Arc<
    dyn Fn(
        serde_json::Value,
        std::sync::Arc<Model>,
    ) -> BoxFuture<Result<serde_json::Value, DiagnosticErrorInfo>>,
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

mod events;
pub use events::*;
