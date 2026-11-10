//! Data prepared for presentation, kept apart from any renderer.

pub mod status_line;

pub mod input_selectors;

pub mod bindings;

pub use maestro_models::{
    OAuthAuthInfo, OAuthLoginCallbacks, OAuthPrompt, OAuthProviderHandle, get_oauth_provider,
    get_oauth_providers,
};

pub mod slash_commands;

/// Supplied native presentations for the final startup handoff.
#[cfg(not(target_arch = "wasm32"))]
pub mod mode_dispatch;
