//! Author component whose hooks and registrations the driving test describes.
//!
//! Only the `wasm32` build exports the component; a native build links an empty library so a
//! workspace test run still compiles every example.
// Tests receive this component's path in MAESTRO_HOOK_COMPONENT.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(target_arch = "wasm32")]
#[path = "../src/tests/hook_author.rs"]
mod hook_author;

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(hook_author::factory);
