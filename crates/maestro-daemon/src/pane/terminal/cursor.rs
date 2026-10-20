//! Cursor presentation from the engine's retained shape observation.

use super::{PaneCore, PaneTerminal, TerminalCursorState};
use std::time::Instant;

impl PaneCore {
    /// Refresh the retained viewport once and read its current cursor.
    pub(super) fn current_cursor_state(&mut self) -> Option<TerminalCursorState> {
        self.refresh_render_state();
        let cursor = &self.render_state.cursor;
        let position = cursor.viewport?;
        Some(TerminalCursorState {
            x: position.x,
            y: u16::try_from(position.y).ok()?,
            visible: cursor.visible,
            shape: if self.terminal.cursor_style_is_default() {
                0
            } else {
                decscusr_cursor_shape(cursor.visual_style, cursor.blinking)
            },
        })
    }
}

impl PaneTerminal {
    /// Read cursor presentation at a supplied monotonic instant.
    /// Position settling is enabled on Windows; visibility and shape remain live.
    #[must_use]
    pub fn cursor_state(&self, now: Instant) -> Option<TerminalCursorState> {
        let mut core = self.core.lock().ok()?;
        let current = core.current_cursor_state();
        if cfg!(windows) {
            core.cursor_settle_state.reported_cursor(current, now)
        } else {
            current
        }
    }
}

/// Encode an explicit cursor appearance as its DECSCUSR parameter.
pub(super) fn decscusr_cursor_shape(style: maestro_terminal::CursorStyle, blinking: bool) -> u8 {
    use maestro_terminal::CursorStyle;
    match (style, blinking) {
        (CursorStyle::Block | CursorStyle::BlockHollow, true) => 1,
        (CursorStyle::Block | CursorStyle::BlockHollow, false) => 2,
        (CursorStyle::Underline, true) => 3,
        (CursorStyle::Underline, false) => 4,
        (CursorStyle::Bar, true) => 5,
        (CursorStyle::Bar, false) => 6,
    }
}
