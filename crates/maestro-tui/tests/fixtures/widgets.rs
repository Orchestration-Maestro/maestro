//! Controlled components and an overlay handle for the component contracts.

use std::cell::RefCell;
use std::rc::Rc;

use maestro_tui::tui::InputHandler;
use maestro_tui::{CURSOR_MARKER, Component, Focusable, OverlayHandle};

/// Names of the components invalidated so far, in invalidation order.
pub type InvalidationTrace = Rc<RefCell<Vec<&'static str>>>;

/// Renders a fixed set of lines and adds its name to a shared trace when invalidated.
pub struct Block {
    /// Name written to the trace.
    pub name: &'static str,
    /// Lines returned by every render.
    pub lines: Vec<String>,
    /// Trace shared with the other components of a test.
    pub trace: InvalidationTrace,
}

impl Component for Block {
    fn render(&mut self, _width: usize) -> Vec<String> {
        self.lines.clone()
    }

    fn invalidate(&mut self) {
        self.trace.borrow_mut().push(self.name);
    }
}

/// Keeps no cached state, so it relies on the default invalidation.
pub struct Passive(pub &'static str);

impl Component for Passive {
    fn render(&mut self, width: usize) -> Vec<String> {
        vec![format!("{}:{width}", self.0)]
    }
}

/// A component that records input and exposes focus.
#[derive(Default)]
pub struct Field {
    /// Whether the field has focus.
    pub focused: bool,
    /// Input chunks received.
    pub received: Vec<String>,
    /// Whether the field wants key-release events.
    pub release: bool,
}

impl Component for Field {
    fn render(&mut self, _width: usize) -> Vec<String> {
        let marker = if self.focused { CURSOR_MARKER } else { "" };
        vec![format!("> {marker}")]
    }

    fn input_handler(&mut self) -> Option<&mut dyn InputHandler> {
        Some(self)
    }

    fn wants_key_release(&self) -> bool {
        self.release
    }

    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }

    fn focusable_mut(&mut self) -> Option<&mut dyn Focusable> {
        Some(self)
    }
}

impl InputHandler for Field {
    fn handle_input(&mut self, data: &str) {
        self.received.push(data.to_owned());
    }
}

impl Focusable for Field {
    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

/// Records the operations an overlay manager would perform.
#[derive(Default)]
pub struct Handle {
    /// Whether the overlay is temporarily hidden.
    pub hidden: bool,
    /// Whether the overlay has focus.
    pub focused: bool,
    /// Whether the overlay was removed.
    pub removed: bool,
    /// Operations in the order they were requested.
    pub log: Vec<&'static str>,
}

impl OverlayHandle for Handle {
    fn hide(&mut self) {
        self.removed = true;
        self.log.push("hide");
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        self.log.push("set_hidden");
    }

    fn is_hidden(&self) -> bool {
        self.hidden
    }

    fn focus(&mut self) {
        self.focused = true;
        self.log.push("focus");
    }

    fn unfocus(&mut self) {
        self.focused = false;
        self.log.push("unfocus");
    }

    fn is_focused(&self) -> bool {
        self.focused
    }
}
