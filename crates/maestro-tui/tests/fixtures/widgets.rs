//! Controlled components for the component contracts.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_tui::tui::InputHandler;
use maestro_tui::{CURSOR_MARKER, Component, FocusFlag, Focusable};

/// Names of the components invalidated so far, in invalidation order.
pub type InvalidationTrace = Rc<RefCell<Vec<&'static str>>>;

/// Renders a fixed set of lines and adds its name to a shared trace when invalidated.
pub struct Block {
    /// Name written to the trace.
    pub name: &'static str,
    /// Lines returned by every render.
    pub lines: RefCell<Vec<String>>,
    /// Trace shared with the other components of a test.
    pub trace: InvalidationTrace,
}

impl Component for Block {
    fn render(&self, _width: usize) -> Vec<String> {
        self.lines.borrow().clone()
    }

    fn invalidate(&self) {
        self.trace.borrow_mut().push(self.name);
    }
}

/// Keeps no cached state, so it relies on the default invalidation.
pub struct Passive(pub &'static str);

impl Component for Passive {
    fn render(&self, width: usize) -> Vec<String> {
        vec![format!("{}:{width}", self.0)]
    }
}

/// A component that records input and exposes focus.
#[derive(Default)]
pub struct Field {
    /// Whether the field has focus.
    pub focus: FocusFlag,
    /// Input chunks received.
    pub received: RefCell<Vec<String>>,
    /// Whether the field wants key-release events.
    pub release: Cell<bool>,
}

impl Component for Field {
    fn render(&self, _width: usize) -> Vec<String> {
        let marker = if self.focus.get() { CURSOR_MARKER } else { "" };
        vec![format!("> {marker}")]
    }

    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }

    fn wants_key_release(&self) -> bool {
        self.release.get()
    }

    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

impl InputHandler for Field {
    fn handle_input(&self, data: &str) {
        self.received.borrow_mut().push(data.to_owned());
    }
}

impl Focusable for Field {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
