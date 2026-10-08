//! Bindings generated from the canonical interface files.
// The generated list lifting passes the same expression as length and capacity.
#![allow(clippy::same_length_and_capacity)]

wit_bindgen::generate!({
    path: "../wit",
    world: "extension",
    pub_export_macro: true,
    default_bindings_module: "maestro_extensions_wasm::bindings",
});
