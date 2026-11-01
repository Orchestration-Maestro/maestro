//! The terminal a frame writer draws on, kept by the host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{TextHandler, WidgetNotice};
use crate::{ExtensionFuture, ExtensionResult};
use std::rc::Rc;

port! {
    /// Requests to the host terminal behind a frame writer.
    TerminalPort for Terminal via port {
        /// Starts the terminal, delivering input chunks and resize notices to the callbacks.
        fn start(on_input: TextHandler, on_resize: WidgetNotice) -> ();
        /// Stops the terminal and restores its previous state.
        fn stop() -> ();
        /// Writes output to the terminal.
        fn write(data: &str) -> ();
        /// Width in columns.
        fn columns() -> u32;
        /// Height in rows.
        fn rows() -> u32;
        /// Whether the Kitty keyboard protocol is active.
        fn kitty_protocol_active() -> bool;
        /// Moves the cursor up for negative and down for positive `lines`.
        fn move_by(lines: i64) -> ();
        /// Hides the cursor.
        fn hide_cursor() -> ();
        /// Shows the cursor.
        fn show_cursor() -> ();
        /// Clears the current line.
        fn clear_line() -> ();
        /// Clears from the cursor to the end of the screen.
        fn clear_from_cursor() -> ();
        /// Clears the screen and moves the cursor to the top-left corner.
        fn clear_screen() -> ();
        /// Sets the window title.
        fn set_title(title: &str) -> ();
        /// Turns the progress indicator on or off.
        fn set_progress(active: bool) -> ();
    }
    owned {
        /// Reads and discards pending input before exit, within `max_ms` and stopping after
        /// `idle_ms` without input; `None` keeps the terminal's default.
        fn drain_input(max_ms: Option<f64>, idle_ms: Option<f64>) -> ();
    }
}

/// The terminal a frame writer draws on; clones share the host's terminal.
#[derive(Clone)]
pub struct Terminal {
    /// The host's terminal.
    port: Rc<dyn TerminalPort>,
}
impl Terminal {
    /// Retains the supplied host capability.
    #[must_use]
    pub fn new(port: Rc<dyn TerminalPort>) -> Self {
        Self { port }
    }
}
