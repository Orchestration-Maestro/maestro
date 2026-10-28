//! A real authored extension exercising standard interfaces alongside the canonical host.
// maestro-component: MAESTRO_WASI_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
#[cfg(target_arch = "wasm32")]
#[path = "wasi_capabilities/author.rs"]
mod author;

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(author::factory);
