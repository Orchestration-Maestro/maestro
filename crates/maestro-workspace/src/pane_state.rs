//! Per-pane viewport state, without a terminal runtime.
use crate::terminal_id::TerminalId;

/// A pane's terminal attachment and viewport flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneState {
    /// Attached terminal identity.
    pub attached_terminal_id: TerminalId,
    /// Whether the pane has been seen since its last idle transition.
    pub seen: bool,
    /// Whether unmodified right-clicks reach the terminal application.
    pub right_click_passthrough: bool,
}
impl PaneState {
    /// Attach a terminal with initially seen, non-passthrough viewport state.
    #[must_use]
    pub fn new(attached_terminal_id: TerminalId) -> Self {
        Self {
            attached_terminal_id,
            seen: true,
            right_click_passthrough: false,
        }
    }
}
