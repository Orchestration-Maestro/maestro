//! Controlled test collaborators.
/// Shared immediate helper.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod immediate;
#[cfg(not(target_arch = "wasm32"))]
pub use immediate::{Immediate, ImmediateCallback};
/// Shared display context helper.
pub mod display_context;
/// Shared display palette helper.
pub mod display_palette;
