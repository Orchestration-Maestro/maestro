//! Data prepared for presentation, kept apart from any renderer.

pub mod status_line;

pub mod input_selectors;

pub mod bindings;

pub use maestro_models::{
    OAuthAuthInfo, OAuthLoginCallbacks, OAuthPrompt, OAuthProviderHandle, get_oauth_provider,
    get_oauth_providers,
};

pub mod slash_commands;
