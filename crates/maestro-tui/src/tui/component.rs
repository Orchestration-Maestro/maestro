//! What a renderable terminal component can do.

use std::cell::Cell;
use std::rc::Rc;

/// Marker a focused component emits where the hardware cursor belongs.
///
/// It is an application-command escape that terminals ignore, so it occupies no cells.
pub const CURSOR_MARKER: &str = "\x1b_maestro:c\x07";

/// A component that accepts raw terminal input while it has focus.
pub trait InputHandler {
    /// Handles one chunk of terminal input, which may hold escape sequences.
    fn handle_input(&mut self, data: &str);
}

/// Whether a component has keyboard focus, shared between the component and the writer.
///
/// Clones observe the same value. A component running a callback is borrowed
/// exclusively, so the writer moves focus through this flag instead of through the
/// component: a change is visible to the component at once.
#[derive(Clone, Default)]
pub struct FocusFlag(Rc<Cell<bool>>);

impl FocusFlag {
    /// Whether the component has focus.
    #[must_use]
    pub fn get(&self) -> bool {
        self.0.get()
    }

    /// Records a focus change.
    pub fn set(&self, focused: bool) {
        self.0.set(focused);
    }
}

/// A component that can hold keyboard focus and show a text cursor.
pub trait Focusable {
    /// The component's flag, which the writer sets when focus moves to or from the
    /// component. A focused component is expected to emit [`CURSOR_MARKER`] when it
    /// renders.
    fn focus_flag(&self) -> &FocusFlag;
}

/// A unit of terminal output laid out for a viewport width.
pub trait Component {
    /// Renders the component for the supplied viewport width.
    fn render(&mut self, width: usize) -> Vec<String>;

    /// Drops any cached rendering state so the next render starts from scratch.
    fn invalidate(&mut self) {}

    /// The input capability, when the component accepts input.
    fn input_handler(&mut self) -> Option<&mut dyn InputHandler> {
        None
    }

    /// Whether the component wants key-release events; `false` unless overridden.
    fn wants_key_release(&self) -> bool {
        false
    }

    /// The focus capability, when the component can be focused.
    fn focusable(&self) -> Option<&dyn Focusable> {
        None
    }
}

/// Whether a component has the focus capability, regardless of its current focus.
#[must_use]
pub fn is_focusable(component: Option<&dyn Component>) -> bool {
    component.is_some_and(|component| component.focusable().is_some())
}
