//! Data prepared for presentation, kept apart from any renderer.

pub mod footer_data_provider;

#[cfg(not(target_arch = "wasm32"))]
pub mod input_selectors;

pub mod keybindings;

pub use maestro_models::get_oauth_providers;

pub mod slash_commands;
