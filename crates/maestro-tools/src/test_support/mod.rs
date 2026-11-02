//! Controlled test collaborators.
/// Shared display context helper.
pub mod display_context;
/// Shared display palette helper.
pub mod display_palette;
/// Shared immediate helper.
#[cfg(not(target_arch = "wasm32"))]
pub mod immediate;
