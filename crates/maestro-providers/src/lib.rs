//! Built-in model protocols, login flows and installation into the model interface's registries.
//!
//! `maestro-models` owns API and login registries and their records. This crate owns the
//! protocol implementations and login flows, installing both base layers through
//! [`install_built_in_providers`].

pub use providers::google::conversion::GoogleThinkingLevel;

pub mod oauth;

#[cfg(test)]
mod tests;

pub use oauth::device::browser::{
    GITHUB_COPILOT_OAUTH_PROVIDER, get_github_copilot_base_url, login_github_copilot,
    normalize_domain, refresh_github_copilot_token,
};
pub use oauth::pages::{oauth_error_html, oauth_success_html};
pub use oauth::responses::code_login::{
    OPENAI_CODEX_OAUTH_PROVIDER, login_openai_codex, refresh_openai_codex_token,
};
pub use oauth::subscription::browser::{
    ANTHROPIC_OAUTH_PROVIDER, login_anthropic, refresh_anthropic_token,
};

/// Installs the built-in providers as the base layer of the model interface's registries.
///
/// The login providers `anthropic`, `github-copilot` and `openai-codex` take the first
/// registry positions, in that order, under any registration already made: a registration
/// carrying one of those identifiers keeps shadowing its built-in until it is removed. The
/// rule is the registry's, stated at [`maestro_models::install_built_in_oauth_providers`].
///
/// The call is idempotent: a second call changes neither membership nor order. On browser
/// targets the registries are thread-local, so this installs on the current thread.
pub fn install_built_in_providers() {
    install_built_in_api_providers();
    maestro_models::install_built_in_oauth_providers(oauth::builtin_providers());
}

/// Built-in protocol registration and raw option decoding.
mod builtins;
pub mod native;
/// Built-in protocol adapters.
pub mod providers;
pub use builtins::{
    BedrockProviderModule, register_built_in_api_providers, reset_api_providers,
    set_bedrock_provider_module, stream_simple_anthropic, stream_simple_anthropic_for,
    stream_simple_azure_openai_responses, stream_simple_azure_openai_responses_for,
    stream_simple_google, stream_simple_google_for, stream_simple_mistral,
    stream_simple_mistral_for, stream_simple_openai_codex_responses,
    stream_simple_openai_codex_responses_for, stream_simple_openai_completions,
    stream_simple_openai_completions_for, stream_simple_openai_responses,
    stream_simple_openai_responses_for,
};
pub use providers::amazon_bedrock::{BedrockOptions, BedrockThinkingDisplay};
pub use providers::anthropic_messages::endpoint::{
    AnthropicClient, AnthropicEffort, AnthropicOptions, AnthropicRequestOptions,
    AnthropicThinkingDisplay, stream_anthropic, stream_anthropic_for,
};
pub use providers::google::generative::{GoogleOptions, stream_google, stream_google_for};
pub use providers::google::managed::{
    GoogleVertexOptions, stream_google_vertex, stream_google_vertex_for,
    stream_simple_google_vertex, stream_simple_google_vertex_for,
};
pub use providers::mistral_conversations::endpoint::{
    MistralOptions, MistralPromptMode, MistralReasoningEffort, MistralToolChoice, stream_mistral,
    stream_mistral_for,
};
pub use providers::openai_codex_responses::codex::{
    OpenAICodexResponsesOptions, OpenAICodexWebSocketDebugStats, stream_openai_codex_responses,
    stream_openai_codex_responses_for,
};
pub use providers::openai_completions::endpoint::{
    OpenAICompletionsOptions, stream_openai_completions, stream_openai_completions_for,
};
pub use providers::openai_responses::azure_transport::{
    AzureOpenAIResponsesOptions, stream_azure_openai_responses, stream_azure_openai_responses_for,
};
pub use providers::openai_responses::openai::{
    OpenAIResponsesOptions, stream_openai_responses, stream_openai_responses_for,
};

/// Installs the nine built-in API protocols beneath existing registrations, without login providers.
pub fn install_built_in_api_providers() {
    maestro_models::install_built_in_api_providers(builtins::built_in_providers());
}
