//! Builds the shared author extension as a native-async component.
//!
//! The extension lives in the crate's test sources so one body serves this component and the
//! tests; build it with `just extension-author-component`. Only the `wasm32` build exports
//! the component; a native build links an empty library so a workspace test run still compiles
//! every example.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(target_arch = "wasm32")]
#[path = "../src/tests/author.rs"]
mod author;

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(author::factory);
