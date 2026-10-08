//! Terminal-facing records and components.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod autocomplete;

use std::rc::Rc;

pub use autocomplete::AutocompleteItem;

use crate::event_bus::CallbackEmitter;
use crate::types::ExtensionResult;

/// What an extension implements to draw itself. Every method runs synchronously and may emit
/// on the bus through the borrowed emitter.
pub trait ComponentView {
    /// The lines to draw for a viewport width.
    ///
    /// # Errors
    /// Returns a message when the view cannot render.
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>>;

    /// Handles keyboard input while the component has focus; ignores it by default.
    ///
    /// # Errors
    /// Returns a message when the input cannot be handled.
    fn handle_input(&self, _data: &str, _emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }

    /// Whether the component receives key release events; false by default.
    fn wants_key_release(&self) -> bool {
        false
    }

    /// Drops cached rendering state.
    ///
    /// # Errors
    /// Returns a message when the view cannot invalidate.
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()>;
}

/// A drawable component an extension hands to the host.
#[derive(Clone)]
pub struct Component(Rc<dyn ComponentView>);

impl Component {
    /// Wraps a view; the host owns the component until it drops it.
    #[must_use]
    pub fn new(view: impl ComponentView + 'static) -> Self {
        Self(Rc::new(view))
    }

    /// The lines to draw for a viewport width.
    ///
    /// # Errors
    /// Returns the view's message when it cannot render.
    pub fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.render(width, emitter)
    }

    /// Handles keyboard input.
    ///
    /// # Errors
    /// Returns the view's message when the input cannot be handled.
    pub fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.handle_input(data, emitter)
    }

    /// Whether the component receives key release events.
    #[must_use]
    pub fn wants_key_release(&self) -> bool {
        self.0.wants_key_release()
    }

    /// Drops cached rendering state.
    ///
    /// # Errors
    /// Returns the view's message when it cannot invalidate.
    pub fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.invalidate(emitter)
    }
}
