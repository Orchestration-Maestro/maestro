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

/// Wrap a successfully decoded value in `Some`; with `serde(default)` an absent field is
/// `None`. `T` alone decides whether null is accepted: a `T` that admits it, such as a JSON
/// value, gives `Some` of that null, and `bool` rejects it.
///
/// # Errors
/// Returns the error of decoding `T`.
pub fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

mod cost;
pub use cost::calculate_cost;

mod events;
mod levels;
mod models;
pub use events::*;
pub use levels::*;
pub use models::*;
/// Insertion-ordered provider reasoning names, with null for unsupported choices.
pub type ThinkingLevelMap = indexmap::IndexMap<ModelThinkingLevel, Option<String>>;
/// Retained live assistant message shared by partial and terminal observations.
pub type SharedAssistantMessage = std::sync::Arc<std::sync::RwLock<AssistantMessage>>;

mod options;
pub use options::*;
