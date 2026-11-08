//! Owned native terminal state transferred during workspace server replacement.
use crate::panes::display::InputState;
use maestro_workspace::tiling::PaneId;
use serde::{Deserialize, Serialize};
use std::os::fd::OwnedFd;

/// Retained terminal session state, excluding transient client coordination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffRuntimeState {
    /// Destination pane identity.
    pub pane_id: u32,
    /// Direct child session identity.
    pub child_pid: u32,
    /// Terminal rows.
    pub rows: u16,
    /// Terminal columns.
    pub cols: u16,
    /// Width of one cell in pixels.
    pub cell_width_px: u32,
    /// Height of one cell in pixels.
    pub cell_height_px: u32,
    /// Active keyboard protocol flags.
    #[serde(default)]
    pub keyboard_protocol_flags: u16,
    /// Replayable keyboard negotiation stack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard_protocol_ansi: Option<String>,
    /// Negotiated terminal input modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_state: Option<InputState>,
    /// Retained display title, independent of agent evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_title: Option<String>,
    /// Recent primary-screen ANSI history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_history_ansi: Option<String>,
}

impl HandoffRuntimeState {
    /// Retargets a transferred terminal to its destination pane.
    #[must_use]
    pub fn with_pane_id(mut self, pane_id: PaneId) -> Self {
        self.pane_id = pane_id.raw();
        self
    }
}

/// Received terminal master with its retained state and owned descriptor lifetime.
#[derive(Debug)]
pub struct ImportedHandoffRuntime {
    /// Received terminal master, closed if construction fails.
    pub master_fd: OwnedFd,
    /// Retained session state.
    pub state: HandoffRuntimeState,
}
