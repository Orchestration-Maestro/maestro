//! Optional synchronous focus and the hardware-cursor marker.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::Component;
use crate::{CallbackEmitter, ExtensionResult};

/// Zero-width marker the host strips before positioning the hardware cursor.
pub const CURSOR_MARKER: &str = "\x1b_maestro:c\x07";

/// A component's synchronous focus getter and setter.
pub trait Focusable {
    /// Reads the current focus flag.
    /// # Errors
    /// Returns the component's failure.
    fn focused(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool>;
    /// Assigns the focus flag before returning.
    /// # Errors
    /// Returns the component's failure.
    fn set_focused(&self, focused: bool, emitter: CallbackEmitter<'_>) -> ExtensionResult<()>;
}

/// Whether a component has the focus capability, even while unfocused.
#[must_use]
pub fn is_focusable(component: Option<&Component>) -> bool {
    component.is_some_and(|component| component.view.focusable().is_some())
}

impl Component {
    /// Reads the focus flag, absent for a component without the capability.
    /// # Errors
    /// Returns the component's failure.
    pub fn focused(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<bool>> {
        self.view
            .focusable()
            .map(|focus| focus.focused(emitter))
            .transpose()
    }
    /// Assigns focus when supported; a nonfocusable component remains unchanged.
    /// # Errors
    /// Returns the component's failure.
    pub fn set_focused(&self, focused: bool, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.view
            .focusable()
            .map_or(Ok(()), |focus| focus.set_focused(focused, emitter))
    }
}
