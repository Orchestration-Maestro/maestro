//! Explicit current chat wire declarations.
use serde_json::{Map, Value};
/// Supplied chat dialect; no capability is inferred from identity or endpoint.
#[derive(Clone)]
pub struct ChatDialect {
    /// Whether to send store:false.
    pub store: bool,
    /// Whether reasoning instructions use the developer role.
    pub developer_role: bool,
    /// Whether to request streamed usage.
    pub usage_in_stream: bool,
    /// Whether tool results carry their name.
    pub tool_result_name: bool,
    /// Whether result batches need an assistant bridge.
    pub assistant_after_tool_result: bool,
    /// Whether readable thinking replays as text.
    pub thinking_as_text: bool,
    /// Whether assistant replay needs a reasoning field.
    pub empty_reasoning_content: bool,
    /// Whether declarations enable tool streaming.
    pub tool_stream: bool,
    /// Whether declarations carry strict:false.
    pub strict_tools: bool,
    /// Whether short caching accepts a session key.
    pub prompt_cache_key: bool,
    /// Whether caching generates affinity headers.
    pub session_affinity_headers: bool,
    /// Whether long caching accepts key retention and marker TTL.
    pub long_cache_retention: bool,
    /// Whether plain foreign IDs use a forty-unit limit.
    pub truncate_plain_call_ids: bool,
    /// Output allowance field.
    pub output_field: ChatOutputField,
    /// Declared thinking encoding.
    pub thinking_format: Option<ChatThinkingFormat>,
    /// Verbatim provider routing.
    pub provider_routing: Option<Map<String, Value>>,
    /// Verbatim providerOptions object.
    pub provider_options: Option<Map<String, Value>>,
    /// Declared cache marker convention.
    pub cache_control: Option<ChatCacheControl>,
    /// Credential header name.
    pub auth_header: String,
    /// Literal credential prefix.
    pub auth_prefix: String,
}
/// Output allowance wire field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatOutputField {
    /// max_tokens field.
    MaxTokens,
    /// max_completion_tokens field.
    MaxCompletionTokens,
}
/// Explicit thinking wire encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatThinkingFormat {
    /// Top-level reasoning effort.
    Effort,
    /// Nested reasoning effort.
    NestedEffort,
    /// Top-level thinking toggle.
    Toggle,
    /// Template thinking toggle with preservation.
    TemplateToggle,
    /// Typed thinking toggle with effort.
    TypedToggle,
}
/// Cache marker convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatCacheControl {
    /// Ephemeral marker on eligible content.
    Ephemeral,
}
