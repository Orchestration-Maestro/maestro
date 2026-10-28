//! Play DOOM in a retained terminal overlay.
mod engine_bindings;
engine_bindings::generate!("player");
pub mod doom_component;
pub mod doom_engine;
pub mod doom_keys;
mod factory;
pub mod wad_finder;
pub use factory::factory;

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
