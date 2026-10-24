//! Retained pane state and presentation.

/// Terminal title presentation normalization.
pub mod titles;

/// Retained pane identity and effective state.
pub mod model;

/// Per-source presentation metadata.
pub mod agents;

/// Native live runtime ownership separate from retained workspace state.
#[cfg(target_os = "linux")]
pub mod owners;
