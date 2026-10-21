//! Mutable daemon state and workspace operations.

mod api;
mod runtime;

use crate::terminal::runtime_registry::TerminalRuntimeRegistry;
use state::DaemonState;

/// Retained daemon state with the sole owners of its live terminal processes.
pub struct App<C> {
    /// Mutable workspace metadata and notification decisions.
    pub state: DaemonState<C>,
    /// Live process owners keyed by durable terminal identity.
    pub terminal_runtimes: TerminalRuntimeRegistry,
}

/// Retained daemon data independent of attached views.
pub mod state;

mod ids;

/// Workspace actions and notification context.
pub mod actions;
