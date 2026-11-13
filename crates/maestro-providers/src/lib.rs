//! The built-in model providers: their login flows and their installation into the model
//! interface's registries.
//!
//! `maestro-models` owns the login provider registry and its records. This crate owns the
//! flows behind the built-in providers and hands them to that registry through
//! [`install_built_in_providers`].

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
    maestro_models::install_built_in_oauth_providers(oauth::builtin_providers());
}
