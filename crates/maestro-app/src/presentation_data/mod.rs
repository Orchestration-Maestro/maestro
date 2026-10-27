//! Data prepared for presentation, kept apart from any renderer.

pub mod footer_data_provider;

pub mod input_selectors;

pub mod keybindings;

pub use maestro_models::{
    OAuthAuthInfo, OAuthLoginCallbacks, OAuthPrompt, OAuthProviderHandle, get_oauth_provider,
    get_oauth_providers,
};

pub mod slash_commands;

/// Supplied native presentations for the final startup handoff.
#[cfg(not(target_arch = "wasm32"))]
pub mod mode_dispatch;
