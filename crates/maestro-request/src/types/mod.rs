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

mod events;
mod levels;
mod models;
pub use events::*;
pub use levels::*;
pub use models::*;
/// Provider reasoning names, with explicit null for unsupported choices.
pub type ThinkingLevelMap = std::collections::BTreeMap<ModelThinkingLevel, Option<String>>;
/// Retained live assistant message shared by partial and terminal observations.
pub type SharedAssistantMessage = std::sync::Arc<std::sync::RwLock<AssistantMessage>>;
