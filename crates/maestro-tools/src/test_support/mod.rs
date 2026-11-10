//! Controlled test collaborators.
/// Shared immediate helper.
#[cfg(not(target_arch = "wasm32"))]
pub mod callbacks;
/// Shared display context helper.
pub mod display_context;
/// Shared display palette helper.
pub mod display_palette;
