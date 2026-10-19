//! Retained stream ownership across synchronous terminal writes.

use super::Handler;
use crate::{Terminal, stream::Stream};

/// Cursor configuration retained independently from the borrowed handler.
pub(crate) struct WriteState {
    /// Parser and incomplete Unicode input.
    stream: Stream,
    /// Whether the current cursor uses configured defaults.
    default_cursor: bool,
    /// Configured cursor shape.
    style: crate::CursorStyle,
    /// Configured blinking mode.
    blink: bool,
}

impl Default for WriteState {
    fn default() -> Self {
        Self {
            stream: Stream::new(true),
            default_cursor: true,
            style: crate::CursorStyle::Block,
            blink: false,
        }
    }
}

impl Terminal {
    /// Consume terminal bytes synchronously, retaining incomplete sequences between writes.
    ///
    /// The stream is temporarily taken out of the terminal while its handler borrows
    /// the screen state, then restored without cloning parser or cursor state.
    /// APC image commands use `apc_handler` limits and enablement; execution is
    /// disabled, so completed image commands cause no screen changes or replies.
    pub fn write(&mut self, bytes: &[u8]) {
        let mut state = self.write_state.take().unwrap_or_default();
        let mut handler = Handler {
            terminal: self,
            default_cursor: state.default_cursor,
            default_cursor_style: state.style,
            default_cursor_blink: state.blink,
        };
        state.stream.next_slice(bytes, &mut handler);
        state.default_cursor = handler.default_cursor;
        state.style = handler.default_cursor_style;
        state.blink = handler.default_cursor_blink;
        self.write_state = Some(state);
    }
}
