#![doc = include_str!("../../../docs/records.md")]

pub mod arguments;
pub use arguments::*;
/// Built-in provider environment utilities.
mod builtins;
pub use builtins::{find_env_keys, get_env_api_key};

pub mod cancellation;
/// Model descriptors and catalog lookup helpers.
mod catalog;
#[cfg(not(target_arch = "wasm32"))]
pub mod catalog_generation;
mod options;
pub use options::{
    AdjustedMaxTokens, adjust_max_tokens_for_thinking, build_base_options, clamp_reasoning,
    is_context_overflow,
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
    OpenAICompletionsOptions, stream_openai_completions, stream_simple_openai_completions,
};
pub use providers::http::{Fetch, FetchError, HttpBody, HttpRequest, HttpResponse, default_fetch};
pub use providers::messages::anthropic::{
    AnthropicClient, AnthropicEffort, AnthropicOptions, AnthropicRequestOptions,
    AnthropicThinkingDisplay, stream_anthropic, stream_simple_anthropic,
};
pub use records::api_registry::{
    ApiProvider, ApiStreamFunction, ApiStreamSimpleFunction, clear_api_providers, get_api_provider,
    get_api_providers, register_api_provider, unregister_api_providers,
};
pub use records::diagnostics::{
    AssistantMessageDiagnostic, DiagnosticCode, DiagnosticErrorInfo, DiagnosticInput,
    append_assistant_message_diagnostic, create_assistant_message_diagnostic,
    extract_diagnostic_error, format_thrown_value,
};
pub use records::event_stream::EventStream;
pub use records::event_stream::create_assistant_message_event_stream;
pub use records::hash::short_hash;
pub use records::headers::headers_to_record;
pub use records::session_resources::{
    SessionResourceCleanup, SessionResourceCleanupError, SessionResourceRemoval,
    cleanup_session_resources, register_session_resource_cleanup,
};
pub use records::stream::{complete, complete_simple, stream, stream_simple};
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
