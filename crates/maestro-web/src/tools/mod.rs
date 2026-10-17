//! Browser tool views.
pub mod artifacts;
/// Browser-local hidden JavaScript execution registration.
#[cfg(target_arch = "wasm32")]
pub mod javascript_repl;
