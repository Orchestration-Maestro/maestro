//! Mutable daemon state and workspace operations.

mod agents;
mod api;
mod api_helpers;
mod config;
mod config_io;
#[cfg(unix)]
mod creation;
mod git_refresh;
mod pane_info;
mod runtime;
mod session;
mod tab_bar_status;
/// Raw and normalized terminal title synchronization.
pub mod terminal_titles;
mod theme_sync;
mod window_title;

/// Stable terminal targeting and unambiguous agent-name resolution.
pub mod terminal_targets;

/// Shared launch inputs and daemon construction.
pub mod construction;

use crate::terminal::runtime_registry::TerminalRuntimeRegistry;
use state::DaemonState;

/// Retained daemon state with the sole owners of its live terminal processes.
pub struct App<C> {
    /// Parsed outer-title format with the server hostname resolved at configuration time.
    window_title_template: Option<(crate::settings::window_title::WindowTitleTemplate, String)>,
    /// Earliest capture time for the next debounced session save.
    pub session_save_deadline: Option<std::time::Instant>,
    /// The sole writer of an already captured session job.
    pub session_save_thread: Option<std::thread::JoinHandle<()>>,
    /// Next terminal-size observation for an interactive caller.
    pub next_resize_poll: std::time::Instant,
    /// Last attempted render, whether or not presented.
    last_render_at: Option<std::time::Instant>,
    /// Last frame actually presented to an interactive caller.
    last_presentation_at: Option<std::time::Instant>,
    /// Earliest time to remove the current workspace notification.
    pub toast_deadline: Option<std::time::Instant>,
    /// When the last API notification was delivered, for its one-second rate limit.
    pub last_api_notification_at: Option<std::time::Instant>,
    /// Next automatic product check, absent when disabled or already requested.
    pub next_auto_update_check: Option<std::time::Instant>,
    /// Next automatic detection catalogue check.
    pub next_agent_manifest_update_check: Option<std::time::Instant>,
    /// Earliest time to remove the current settings diagnostic.
    pub config_diagnostic_deadline: Option<std::time::Instant>,
    /// Whether a disk reload awaits acknowledgement by the loop.
    config_reloaded_from_disk: bool,
    /// Accepted history disable awaiting the supplied persistence paths.
    clear_history_after_reload: bool,
    /// Single internal observation queue drained by the daemon loop.
    pub event_rx: tokio::sync::mpsc::Receiver<crate::events::AppEvent>,
    /// Earliest time to remove the current clipboard acknowledgement.
    pub copy_feedback_deadline: Option<std::time::Instant>,
    /// Status producers retained across internal event passes.
    tab_bar_status: tab_bar_status::TabBarStatus,
    /// Repository status refresh scheduling and its cache.
    git_refresh: git_refresh::GitRefresh,
    /// Accepted preferences shared by creation and replacement process launches.
    pub config: crate::settings::model::Config,
    /// Last accepted loader documents, independent of typed runtime preferences.
    accepted_config_areas: crate::settings::io::AcceptedAreas,
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
