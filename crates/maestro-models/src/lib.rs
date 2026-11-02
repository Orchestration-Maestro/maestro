#![doc = include_str!("../../../docs/records.md")]

pub mod arguments;
pub mod bedrock_provider;
pub use arguments::*;
/// Built-in provider environment utilities.
mod builtins;
pub use builtins::{
    BedrockProviderModule, find_env_keys, get_env_api_key, register_built_in_api_providers,
    reset_api_providers, set_bedrock_provider_module, stream_simple_anthropic,
    stream_simple_anthropic_for, stream_simple_azure_openai_responses,
    stream_simple_azure_openai_responses_for, stream_simple_google, stream_simple_google_for,
    stream_simple_mistral, stream_simple_mistral_for, stream_simple_openai_codex_responses,
    stream_simple_openai_codex_responses_for, stream_simple_openai_completions,
    stream_simple_openai_completions_for, stream_simple_openai_responses,
    stream_simple_openai_responses_for,
};

pub mod cancellation;
/// Model descriptors and catalog lookup helpers.
mod catalog;
#[cfg(not(target_arch = "wasm32"))]
pub mod catalog_generation;
mod options;
pub use options::{
    AdjustedMaxTokens, ESTIMATED_IMAGE_CHARS, adjust_max_tokens_for_thinking, build_base_options,
    build_base_options_for, clamp_reasoning, is_context_overflow, reasoning_from_thinking_level,
    tokens_for_chars, utf16_units,
};
/// Conversation adaptation for the destination model.
mod projection;
pub mod records;
pub use projection::transform_messages;

pub use cancellation::Cancellation;
pub use catalog::models::{
    calculate_cost, clamp_thinking_level, get_model, get_models, get_providers,
    get_supported_thinking_levels, models_are_equal,
};

pub use providers::chat::openai_completions::{
    OpenAICompletionsOptions, stream_openai_completions, stream_openai_completions_for,
};
pub use providers::cloud::amazon_bedrock::{BedrockOptions, BedrockThinkingDisplay};
pub use providers::content::google::{GoogleOptions, stream_google, stream_google_for};
pub use providers::content::google_shared::GoogleThinkingLevel;
pub use providers::http::{
    Fetch, FetchError, HttpBody, HttpRequest, HttpResponse, default_fetch, fetch_optional_body,
};
pub use providers::messages::anthropic::{
    AnthropicClient, AnthropicEffort, AnthropicOptions, AnthropicRequestOptions,
    AnthropicThinkingDisplay, stream_anthropic, stream_anthropic_for,
};
pub use providers::reasoning::mistral::{
    MistralOptions, MistralPromptMode, MistralReasoningEffort, MistralToolChoice, stream_mistral,
    stream_mistral_for,
};
pub use providers::responses::azure_openai_responses::{
    AzureOpenAIResponsesOptions, stream_azure_openai_responses, stream_azure_openai_responses_for,
};
pub use providers::responses::openai_responses::{
    OpenAIResponsesOptions, stream_openai_responses, stream_openai_responses_for,
};
pub use records::api_registry::{
    ApiProvider, ApiStreamFunction, ApiStreamSimpleFunction, clear_api_providers, get_api_provider,
    get_api_providers, register_api_provider, unregister_api_providers,
};
pub use records::diagnostics::{
    AssistantMessageDiagnostic, DiagnosticCode, DiagnosticErrorInfo, DiagnosticInput,
    append_assistant_message_diagnostic, create_assistant_message_diagnostic,
    extract_diagnostic_error, format_thrown_value, timestamp_now,
};
pub use records::event_stream::create_assistant_message_event_stream;
pub use records::event_stream::{AssistantMessageEventStreamPort, EventStream};
pub use records::hash::short_hash;
pub use records::headers::headers_to_record;
pub use records::session_resources::{
    SessionResourceCleanup, SessionResourceCleanupError, SessionResourceRemoval,
    cleanup_session_resources, register_session_resource_cleanup,
};
pub use records::stream::{
    complete, complete_for, complete_simple, complete_simple_for, stream, stream_for,
    stream_simple, stream_simple_for,
};
pub use records::typebox_helpers::{StringEnumOptions, string_enum};
pub use records::types::*;

pub mod providers;
pub use providers::faux::*;

pub mod oauth;
pub use oauth::device::github_copilot::{
    GITHUB_COPILOT_OAUTH_PROVIDER, get_github_copilot_base_url, login_github_copilot,
    normalize_domain, refresh_github_copilot_token,
};
pub use oauth::oauth_page::{oauth_error_html, oauth_success_html};
pub use oauth::pkce::{Pkce, generate_pkce};
pub use oauth::responses::openai_codex::{
    OPENAI_CODEX_OAUTH_PROVIDER, login_openai_codex, refresh_openai_codex_token,
};
pub use oauth::subscription::anthropic::{
    ANTHROPIC_OAUTH_PROVIDER, login_anthropic, refresh_anthropic_token,
};
pub use oauth::types::{
    OAuthAuthInfo, OAuthCredentials, OAuthPrompt, OAuthProvider, OAuthProviderId,
    OAuthSelectOption, OAuthSelectPrompt,
};
pub use oauth::types::{OAuthCallbacks, OAuthError, OAuthLoginCallbacks, OAuthProviderInterface};
pub use oauth::{
    OAuthApiKey, OAuthProviderHandle, get_oauth_api_key, get_oauth_provider, get_oauth_providers,
    refresh_oauth_token, register_oauth_provider, reset_oauth_providers, unregister_oauth_provider,
};

pub use providers::response_sessions::openai_codex_responses::{
    OpenAICodexResponsesOptions, OpenAICodexWebSocketDebugStats, stream_openai_codex_responses,
    stream_openai_codex_responses_for,
};

pub use providers::json_text::compact_json;

#[cfg(test)]
pub mod tests;

pub use maestro_request::tool_info::ToolInfo;
#[cfg(not(target_arch = "wasm32"))]
pub use providers::websocket::default_web_socket;
pub use providers::websocket::{
    ConnectWebSocket, WebSocket, WebSocketConnection, WebSocketFuture, WebSocketRequest,
};

pub use providers::content::google_vertex::{
    GoogleVertexOptions, stream_google_vertex, stream_google_vertex_for,
    stream_simple_google_vertex, stream_simple_google_vertex_for,
};

/// Shared extension event inventory for concrete native adapters.
pub use maestro_request::extension_event_table;
/// Shared extension events and assembled prompt records.
pub use maestro_request::{extension_events, system_prompt};

/// Shared parameter-schema records and their declared node kinds.
pub use maestro_request::parameter_schema::{
    Base, ParameterSchema, Refinement, SchemaCallback, SchemaCode, SchemaCodeSource, SchemaKind,
    SchemaNode, transport as schema_transport,
};
