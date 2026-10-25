//! The terminal a toolkit renders to; implementations live in adapter crates.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

/// Receives a native output failure.
pub type OutputErrorHandler = Rc<dyn Fn(&io::Error)>;

/// A terminal that components render to and that delivers input.
pub trait Terminal {
    /// Starts the terminal, delivering input chunks and resize notices to the callbacks.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn start(
        &mut self,
        on_input: Box<dyn FnMut(&str)>,
        on_resize: Box<dyn FnMut()>,
    ) -> io::Result<()>;

    /// Stops the terminal and restores its previous state.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn stop(&mut self) -> io::Result<()>;

    /// Reads and discards pending input before exit so key releases do not leak to a shell.
    ///
    /// `None` leaves the adapter's defaults: a 1000 ms limit and a 50 ms idle exit. The
    /// returned future owns its state, so a scheduled frame may borrow and render to the
    /// same terminal while the drain is pending.
    fn drain_input(
        &mut self,
        max: Option<Duration>,
        idle: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<()>> + 'static>>;

    /// Writes output to the terminal.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn write(&mut self, data: &str) -> io::Result<()>;

    /// Bytes accepted by the terminal but not yet written; zero without backlog support.
    fn pending_output_bytes(&self) -> usize {
        0
    }

    /// Width in columns.
    fn columns(&self) -> usize;

    /// Height in rows.
    fn rows(&self) -> usize;

    /// Whether the Kitty keyboard protocol is active.
    fn kitty_protocol_active(&self) -> bool;

    /// Moves the cursor up for negative and down for positive `lines`.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn move_by(&mut self, lines: isize) -> io::Result<()>;

    /// Hides the cursor.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn hide_cursor(&mut self) -> io::Result<()>;

    /// Shows the cursor.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn show_cursor(&mut self) -> io::Result<()>;

    /// Clears the current line.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn clear_line(&mut self) -> io::Result<()>;

    /// Clears from the cursor to the end of the screen.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn clear_from_cursor(&mut self) -> io::Result<()>;

    /// Clears the screen and moves the cursor to the top-left corner.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn clear_screen(&mut self) -> io::Result<()>;

    /// Sets the window title.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn set_title(&mut self, title: &str) -> io::Result<()>;

    /// Turns the terminal progress indicator on or off.
    ///
    /// # Errors
    ///
    /// Returns the device error when the effect cannot be carried out.
    fn set_progress(&mut self, active: bool) -> io::Result<()>;

    /// Registers `handler` for native output failures; the returned operation removes it and
    /// does nothing when repeated.
    fn on_output_error(&self, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        let _ = handler;
        Box::new(|| ())
    }
}
