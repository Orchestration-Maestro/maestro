//! Process pane terminal state.

/// Cursor position settling.
pub mod cursor;

/// Retained pane terminal records.
pub mod terminal;

/// Pane capability query replies.
mod xtgettcap;

/// Pane OSC evidence.
mod osc;
