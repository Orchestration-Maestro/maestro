//! Mutable daemon state and workspace operations.

mod api;
mod runtime;
mod tab_bar_status;

/// Shared launch inputs and daemon construction.
pub mod construction;

use crate::terminal::runtime_registry::TerminalRuntimeRegistry;
use state::DaemonState;

/// Retained daemon state with the sole owners of its live terminal processes.
pub struct App<C> {
    /// Earliest time to remove the current clipboard acknowledgement.
    pub copy_feedback_deadline: Option<std::time::Instant>,
    /// Status producers retained across internal event passes.
    tab_bar_status: tab_bar_status::TabBarStatus,
    /// Accepted preferences shared by creation and replacement process launches.
    pub config: crate::settings::model::Config,
    /// Caller-supplied process entry and shared runtime adapters.
    pub launch: construction::PaneLaunchInputs,
    /// Mutable workspace metadata and notification decisions.
    pub state: DaemonState<C>,
    /// Live process owners keyed by durable terminal identity.
    pub terminal_runtimes: TerminalRuntimeRegistry,
    /// Shared publication history for state transitions and pane exits.
    pub event_hub: crate::api::event_hub::EventHub,
    /// Temporary panes awaiting exit and restoration of their previous focus.
    pub overlay_panes:
        std::collections::HashMap<maestro_workspace::layout::PaneId, OverlayPaneState>,
}

/// Focus and cleanup retained while a temporary overlay process owns a pane.
pub struct OverlayPaneState {
    /// Workspace display position containing the overlay.
    pub ws_idx: usize,
    /// Tab display position containing the overlay.
    pub tab_idx: usize,
    /// Pane to restore if the overlay still has focus when it exits.
    pub previous_focus: maestro_workspace::layout::PaneId,
    /// Zoom state before the overlay took focus.
    pub previous_zoomed: bool,
    /// Temporary files owned by the overlay process.
    pub temp_files: Vec<std::path::PathBuf>,
}

/// Retained daemon data independent of attached views.
pub mod state;

mod ids;

/// Workspace actions and notification context.
pub mod actions;
