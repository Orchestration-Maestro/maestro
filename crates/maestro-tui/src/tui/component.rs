//! What a renderable terminal component can do.

use std::{cell::Cell, rc::Rc};

/// Marker a focused component emits where the hardware cursor belongs.
///
/// It is an application-command escape that terminals ignore, so it occupies no cells.
pub const CURSOR_MARKER: &str = "\x1b_maestro:c\x07";

/// A component that accepts raw terminal input while it has focus.
pub trait InputHandler {
    /// Handles one chunk of terminal input, which may hold escape sequences.
    fn handle_input(&self, data: &str);

    /// Handles input that can return an ordinary failure.
    ///
    /// # Errors
    /// Returns the handler's original input error.
    fn try_handle_input(&self, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.handle_input(data);
        Ok(())
    }
}

/// Whether a component has keyboard focus: the component owns the flag, the writer
/// sets it when focus moves, and the component reads it when it renders.
#[derive(Default)]
pub struct FocusFlag {
    /// This component's own focus state.
    focused: Cell<bool>,
    /// Targets receiving each focus change immediately, in order.
    targets: Vec<Rc<dyn Focusable>>,
}

impl FocusFlag {
    /// Creates an independent flag that forwards focus changes to a child.
    #[must_use]
    pub fn forwarding(target: Rc<dyn Focusable>) -> Self {
        Self::forwarding_all(vec![target])
    }

    /// Creates an independent flag forwarding updates to each target in order.
    #[must_use]
    pub fn forwarding_all(targets: Vec<Rc<dyn Focusable>>) -> Self {
        Self {
            focused: Cell::new(false),
            targets,
        }
    }

    /// Whether the component has focus.
    #[must_use]
    pub fn get(&self) -> bool {
        self.focused.get()
    }

    /// Records own state, then synchronously forwards to every target in order.
    pub fn set(&self, focused: bool) {
        self.focused.set(focused);
        for target in &self.targets {
            target.set_focused(focused);
        }
    }
}

/// A component that can hold keyboard focus and show a text cursor.
pub trait Focusable {
    /// The component's flag, which the writer sets when focus moves to or from the
    /// component. A focused component is expected to emit [`CURSOR_MARKER`] when it
    /// renders.
    fn focus_flag(&self) -> &FocusFlag;

    /// Assigns focus synchronously; components may also forward the change to their owner.
    fn set_focused(&self, focused: bool) {
        self.focus_flag().set(focused);
    }
}

/// A unit of terminal output laid out for a viewport width.
///
/// A component is a shared object: every method takes `&self`, so the writer, a listener
/// or another component can reach it while one of its own methods is running, for example
/// during a render that moves focus or invalidates the writer's components, and the call
/// takes effect at once. A component keeps the state it changes in `Cell`s and `RefCell`s,
/// borrows it only inside one method and never across a call into the writer, a callback
/// or another component, so a call that re-enters the component never finds it borrowed.
pub trait Component: std::any::Any {
    /// Renders the component for the supplied viewport width.
    fn render(&self, width: usize) -> Vec<String>;

    /// Render a component that may report an ordinary rendering failure.
    ///
    /// # Errors
    /// Returns the component's original rendering error.
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Ok(self.render(width))
    }

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

/// Runs composed rendering with the ordinary, non-failing child operation.
pub(crate) fn render_infallible<C: Component + ?Sized>(
    render: impl FnOnce(
        fn(&C, usize) -> Result<Vec<String>, std::convert::Infallible>,
    ) -> Result<Vec<String>, std::convert::Infallible>,
) -> Vec<String> {
    render(|child, width| Ok(child.render(width))).unwrap_or_else(|never| match never {})
}
