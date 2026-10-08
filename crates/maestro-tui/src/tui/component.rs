//! What a renderable terminal component can do.

use std::cell::Cell;

/// Marker a focused component emits where the hardware cursor belongs.
///
/// It is an application-command escape that terminals ignore, so it occupies no cells.
pub const CURSOR_MARKER: &str = "\x1b_maestro:c\x07";

/// A component that accepts raw terminal input while it has focus.
pub trait InputHandler {
    /// Handles one chunk of terminal input, which may hold escape sequences.
    fn handle_input(&self, data: &str);
}

/// Whether a component has keyboard focus: the component owns the flag, the writer
/// sets it when focus moves, and the component reads it when it renders.
#[derive(Default)]
pub struct FocusFlag(Cell<bool>);

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
///
/// A component is a shared object: every method takes `&self`, so the writer, a listener
/// or another component can reach it while one of its own methods is running, for example
/// during a render that moves focus or invalidates the writer's components, and the call
/// takes effect at once. A component keeps the state it changes in `Cell`s and `RefCell`s,
/// borrows it only inside one method and never across a call into the writer, a callback
/// or another component, so a call that re-enters the component never finds it borrowed.
pub trait Component {
    /// Renders the component for the supplied viewport width.
    fn render(&self, width: usize) -> Vec<String>;

    /// Drops any cached rendering state so the next render starts from scratch.
    fn invalidate(&self) {}

    /// The input capability, when the component accepts input.
    fn input_handler(&self) -> Option<&dyn InputHandler> {
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
