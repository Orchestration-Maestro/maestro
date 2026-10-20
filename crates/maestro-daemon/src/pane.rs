//! Process pane terminal state.

mod input;

/// Cursor position settling.
pub mod cursor;

/// Retained pane terminal records.
pub mod terminal;

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
