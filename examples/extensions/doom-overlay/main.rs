//! Play DOOM in a retained terminal overlay.
mod engine_bindings;
engine_bindings::generate!("player");
mod factory;
pub use factory::factory;

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
