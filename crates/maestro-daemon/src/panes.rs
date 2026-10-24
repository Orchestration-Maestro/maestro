//! Process pane terminal state.

mod input;

/// Screen scan and publication policy for pane detection.
pub mod agent_detection;

/// Cursor position settling.
pub mod settlement;

/// Retained pane terminal records.
pub mod display;

/// Pane capability query replies.
mod xtgettcap;

/// Pane OSC evidence.
mod osc;

/// Destination policy for pane wheel input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelRouting {
    /// Encode a mouse wheel report for the child.
    MouseReport,
    /// Encode cursor keys for the alternate screen.
    AlternateScroll,
    /// Navigate retained host scrollback.
    HostScroll,
}

/// Pane launch environment and shell selection.
#[cfg(target_os = "linux")]
pub mod commands;

/// Pane terminal identity and launch environment.
#[cfg(target_os = "linux")]
pub mod launch;

/// Foreground process probing policy.
#[cfg(target_os = "linux")]
pub mod inspection;

/// Asynchronous foreground and screen detection.
#[cfg(target_os = "linux")]
pub mod monitoring;

/// Owned native pane processes and terminal I/O.
#[cfg(target_os = "linux")]
pub mod lifecycle;

#[cfg(target_os = "linux")]
pub use lifecycle::{
    AgentDetection, FocusEvent, PaneRuntime, PaneRuntimeConfig, SpawnInitialState,
};
