//! Retained pane state and presentation.

/// Terminal title presentation normalization.
pub mod title;

/// Retained pane identity and effective state.
pub mod state;

/// Per-source presentation metadata.
pub mod metadata;

/// Native live runtime ownership separate from retained workspace state.
#[cfg(target_os = "linux")]
pub mod runtime_registry;
