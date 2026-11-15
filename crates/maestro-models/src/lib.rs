#![doc = include_str!("../../../docs/records.md")]

pub mod parameters;
pub use parameters::*;
/// Built-in provider environment utilities.
mod builtins;
pub use builtins::{find_env_keys, get_env_api_key};

pub mod cancellation;
/// Model descriptors and catalog lookup helpers.
mod catalog;
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

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_http::configure_http_dispatcher;
pub use maestro_http::{
    Fetch, FetchError, HttpBody, HttpRequest, HttpResponse, default_fetch, fetch_optional_body,
    unit_random,
};

pub use records::api_registry::{
    ApiProvider, ApiStreamFunction, ApiStreamSimpleFunction, clear_api_providers, get_api_provider,
    get_api_providers, install_built_in_api_providers, register_api_provider,
    unregister_api_providers,
};
pub use records::diagnostics::{
    AssistantMessageDiagnostic, DiagnosticCode, DiagnosticErrorInfo, DiagnosticInput,
    append_assistant_message_diagnostic, create_assistant_message_diagnostic,
    extract_diagnostic_error, format_thrown_value, timestamp_now,
};
pub use records::digest::short_hash;
pub use records::events::create_assistant_message_event_stream;
pub use records::events::{AssistantMessageEventStreamPort, EventStream};
pub use records::header_values::headers_to_record;
pub use records::invocation::{
    complete, complete_for, complete_simple, complete_simple_for, stream, stream_for,
    stream_simple, stream_simple_for, stream_simple_via_for,
};
pub use records::sessions::{
    SessionResourceCleanup, SessionResourceCleanupError, SessionResourceRemoval,
    cleanup_session_resources, register_session_resource_cleanup,
};
pub use records::string_enum::{EnumOptions, string_enum};
pub use records::types::*;

pub mod providers;
pub use providers::faux::{
    FauxAssistantContent, FauxAssistantMessageOptions, FauxContentBlock, FauxModelDefinition,
    FauxProviderRegistration, FauxProviderState, FauxResponseCallback, FauxResponseFactory,
    FauxResponseInput, FauxResponseOptions, FauxResponseStep, FauxTokenSize, FauxToolCallOptions,
    RegisterFauxProviderOptions, faux_assistant_message, faux_text, faux_thinking, faux_tool_call,
    register_faux_provider,
};

pub mod oauth;
pub use oauth::proof_key::{Pkce, generate_pkce};
pub use oauth::types::{
    OAuthAuthInfo, OAuthCredentials, OAuthPrompt, OAuthProvider, OAuthProviderId,
    OAuthSelectOption, OAuthSelectPrompt,
};
pub use oauth::types::{OAuthCallbacks, OAuthError, OAuthLoginCallbacks, OAuthProviderInterface};
pub use oauth::{
    OAuthApiKey, OAuthProviderHandle, get_oauth_api_key, get_oauth_provider, get_oauth_providers,
    install_built_in_oauth_providers, refresh_oauth_token, register_oauth_provider,
    reset_oauth_providers, unregister_oauth_provider,
};

pub use providers::json_text::compact_json;

#[cfg(feature = "test-support")]
pub mod test_support;

#[cfg(test)]
extern crate self as maestro_models;

#[cfg(test)]
pub mod tests;

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_http::websocket::default_web_socket;
pub use maestro_http::websocket::{
    ConnectWebSocket, WebSocket, WebSocketConnection, WebSocketFuture, WebSocketRequest,
};
pub use maestro_interchange::tool_info::ToolInfo;

/// Shared extension event inventory for concrete native adapters.
pub use maestro_interchange::extension_event_table;
/// Shared extension events and assembled prompt records.
pub use maestro_interchange::{extension_events, system_prompt};

/// Shared parameter-schema records and their declared node kinds.
pub use maestro_interchange::parameter_schema::{
    Base, ParameterSchema, Refinement, SchemaCallback, SchemaCode, SchemaCodeSource, SchemaKind,
    SchemaNode, transport as schema_transport,
};

#[cfg(not(target_arch = "wasm32"))]
pub use builtins::{env_value, raw_env_value};
#[cfg(not(target_arch = "wasm32"))]
pub use options::thinking_budget;
pub use options::{fit_thinking, resolve_api_key, resolve_cache_retention};
pub use projection::{IMAGE_PLACEHOLDER, normalize_tool_call_id, tool_call_id_characters};
