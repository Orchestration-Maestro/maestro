#![doc = include_str!("../../../../docs/models/oauth.md")]
pub mod device;
pub mod pages;
pub mod responses;
pub mod subscription;

mod callback;
mod native;
pub use device::browser::{
    GITHUB_COPILOT_OAUTH_PROVIDER, get_github_copilot_base_url, login_github_copilot,
    normalize_domain, refresh_github_copilot_token,
};

pub(crate) use maestro_text::is_whitespace_scalar as authorization_whitespace;

use maestro_models::{OAuthProviderHandle, SharedHandle};
pub use responses::code_login::{
    OPENAI_CODEX_OAUTH_PROVIDER, login_openai_codex, refresh_openai_codex_token,
};
pub use subscription::browser::{
    ANTHROPIC_OAUTH_PROVIDER, login_anthropic, refresh_anthropic_token,
};

/// The built-in login providers in their registry positions.
pub(crate) fn builtin_providers() -> [OAuthProviderHandle; 3] {
    [
        SharedHandle::new(subscription::browser::AnthropicOAuthProvider),
        SharedHandle::new(device::browser::GitHubCopilotOAuthProvider),
        SharedHandle::new(responses::code_login::OpenAICodexOAuthProvider),
    ]
}
