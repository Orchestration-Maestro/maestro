//! Mutable daemon state and workspace operations.

/// Retained daemon data independent of attached views.
pub mod state;

mod ids;

/// Workspace actions and notification context.
pub mod actions;
