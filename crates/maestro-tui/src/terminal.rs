//! Caller-supplied terminal contracts.
use std::{future::Future, pin::Pin};
/// Terminal.
pub trait Terminal {
    /// Install input and resize callbacks.
    fn start(&mut self, on_input: Box<dyn FnMut(String)>, on_resize: Box<dyn FnMut()>);
    /// Stop.
    fn stop(&mut self);
    /// Drain input; absent arguments represent 1000/50 millisecond defaults.
    fn drain_input<'a>(
        &'a mut self,
        max_ms: Option<usize>,
        idle_ms: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>>;
    /// Write.
    fn write(&mut self, data: &str);
    /// Columns.
    fn columns(&self) -> usize;
    /// Rows.
    fn rows(&self) -> usize;
    /// Kitty protocol active.
    fn kitty_protocol_active(&self) -> bool;
    /// Move relative to the cursor, with signed line displacement.
    fn move_by(&mut self, lines: isize);
    /// Hide cursor.
    fn hide_cursor(&mut self);
    /// Show cursor.
    fn show_cursor(&mut self);
    /// Clear line.
    fn clear_line(&mut self);
    /// Clear from cursor.
    fn clear_from_cursor(&mut self);
    /// Clear screen.
    fn clear_screen(&mut self);
    /// Set title.
    fn set_title(&mut self, title: &str);
    /// Set progress.
    fn set_progress(&mut self, active: bool);
}
