//! An opt-in filesystem and network sandbox for bash commands.
// maestro-component: MAESTRO_SANDBOX_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(target_arch = "wasm32")]
mod author;
#[cfg(target_arch = "wasm32")]
mod config;
#[cfg(target_arch = "wasm32")]
mod config_data;
#[cfg(target_arch = "wasm32")]
mod linux;
#[cfg(target_arch = "wasm32")]
mod macos;
#[cfg(target_arch = "wasm32")]
mod macos_profile;
#[cfg(target_arch = "wasm32")]
mod network;
#[cfg(target_arch = "wasm32")]
mod platform;
#[cfg(target_arch = "wasm32")]
mod platform_commands;
#[cfg(target_arch = "wasm32")]
mod proxies;
#[cfg(target_arch = "wasm32")]
mod proxy;
#[cfg(target_arch = "wasm32")]
mod proxy_io;
#[cfg(target_arch = "wasm32")]
mod runtime;
#[cfg(target_arch = "wasm32")]
mod seccomp;

#[cfg(target_arch = "wasm32")]
pub use author::factory;

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
