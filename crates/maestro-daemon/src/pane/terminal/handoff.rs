//! Restore input negotiation through the engine's mode handlers.
use super::{InputState, PaneTerminal};
use crate::input::{MouseProtocolEncoding, MouseProtocolMode};

impl PaneTerminal {
    /// Restore negotiated input modes before resuming a handed-off pane.
    pub fn seed_handoff_input_state(&self, state: InputState) {
        let Ok(mut core) = self.core.lock() else {
            return;
        };
        if bool::from(state.alternate_screen) {
            core.terminal.write(b"\x1b[?1049h");
        }
        for (mode, enabled) in [
            (1, bool::from(state.application_cursor)),
            (2004, bool::from(state.bracketed_paste)),
            (1004, bool::from(state.focus_reporting)),
            (1007, bool::from(state.mouse_alternate_scroll)),
            (2031, bool::from(state.color_scheme_reporting)),
            (9, state.mouse_protocol_mode == MouseProtocolMode::Press),
            (
                1000,
                state.mouse_protocol_mode == MouseProtocolMode::PressRelease,
            ),
            (
                1002,
                state.mouse_protocol_mode == MouseProtocolMode::ButtonMotion,
            ),
            (
                1003,
                state.mouse_protocol_mode == MouseProtocolMode::AnyMotion,
            ),
            (
                1005,
                state.mouse_protocol_encoding == MouseProtocolEncoding::Utf8,
            ),
            (
                1006,
                state.mouse_protocol_encoding == MouseProtocolEncoding::Sgr,
            ),
            (
                1016,
                state.mouse_protocol_encoding == MouseProtocolEncoding::SgrPixels,
            ),
        ] {
            core.terminal
                .write(format!("\x1b[?{mode}{}", if enabled { 'h' } else { 'l' }).as_bytes());
        }
        if bool::from(state.modify_other_keys) {
            core.terminal.write(b"\x1b[>4;2m");
        }
    }

    /// Push nonzero keyboard flags onto the engine's active-screen stack.
    pub fn seed_keyboard_protocol_flags(&self, flags: u16) {
        if flags != 0 {
            self.seed_keyboard_protocol_ansi(&format!("\x1b[>{flags}u"));
        }
    }

    /// Restore a serialized keyboard stack using the existing parser.
    pub fn seed_keyboard_protocol_ansi(&self, ansi: &str) {
        self.seed_history_ansi(ansi);
    }
}
