//! Component entry for the authored-agent package.

#[cfg(target_arch = "wasm32")]
use maestro_subagents::factory;

#[cfg(target_arch = "wasm32")]
maestro_extensions_guest::export_extension!(factory);
