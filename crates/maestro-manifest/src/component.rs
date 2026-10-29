//! Component entry for the manifest package.

#[cfg(target_arch = "wasm32")]
use maestro_manifest::factory;

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
