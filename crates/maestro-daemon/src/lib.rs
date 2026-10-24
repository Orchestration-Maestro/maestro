//! Retained pane agent identity and presentation for the workspace daemon.

/// Validated session references and literal resume plans.
pub mod resume;

/// Owned terminal input and supplied host geometry.
pub mod input;

/// Workspace preference values and validation.
pub mod settings;

/// Pane state and presentation operations.
pub mod terminal;

/// Host theme state and colour sequence emission.
pub mod terminal_theme;

/// Supplied agent roots and daemon resource paths.
pub mod locations;

/// Owned named-daemon selection and lifecycle information.
pub mod instances;

/// Structured workspace events and bounded file logging.
pub mod diagnostics;

/// Live pane directory observations over topology-owned tabs.
#[cfg(target_os = "linux")]
pub mod tab;

/// Live process observations over topology-owned workspaces.
#[cfg(target_os = "linux")]
pub mod workspace;

/// Daemon snapshot save and clear operations.
pub mod persist;

/// Environment names grouped by their runtime consumers.
pub mod variables;

/// Process pane terminal state.
pub mod panes;

/// Absolute-row text selections within a pane.
pub mod selection;

/// Opt-in render profiling through structured tracing.
pub mod render_prof;

/// Coalesced pane render and metadata requests.
pub mod render_signal;

/// Lossless background pane event delivery.
pub mod events;

/// Shared replay history for daemon API consumers.
pub mod api;

/// Native hostname and local calendar observations.
pub mod platform;

/// Workspace key-combination grammar.
pub mod keys;

/// Retained daemon state and workspace operations.
#[cfg(target_os = "linux")]
pub mod app;

#[cfg(test)]
mod tests;

/// Native terminal state transferred between workspace servers.
#[cfg(unix)]
pub mod transfer;

/// Native repository discovery and caller-owned Git process fallbacks.
pub mod git;
