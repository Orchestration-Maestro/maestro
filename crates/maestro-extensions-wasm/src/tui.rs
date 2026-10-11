//! Retained components supplied by extension authors.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::{CallbackEmitter, ExtensionResult};
use std::rc::Rc;

/// An author's synchronous component operations.
pub trait ComponentView {
    /// Renders lines at the supplied viewport width.
    /// # Errors
    /// Returns the author's failure.
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>>;
    /// Handles literal input; the default does nothing.
    /// # Errors
    /// Returns the author's failure.
    fn handle_input(&self, _data: &str, _emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
    /// Reads the current key-release preference; the default is false.
    /// # Errors
    /// Returns the author's failure.
    fn wants_key_release(&self, _emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Ok(false)
    }
    /// Invalidates retained rendering state.
    /// # Errors
    /// Returns the author's failure.
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()>;
}

/// Independently retained component; clones share the authored view.
#[derive(Clone)]
pub struct Component(Rc<dyn ComponentView>);
impl Component {
    /// Retains the supplied author view.
    #[must_use]
    pub fn new(view: impl ComponentView + 'static) -> Self {
        Self(Rc::new(view))
    }
    /// Renders at the supplied width.
    /// # Errors
    /// Returns the author's failure.
    pub fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.render(width, emitter)
    }
    /// Forwards input to the retained view.
    /// # Errors
    /// Returns the author's failure.
    pub fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.handle_input(data, emitter)
    }
    /// Reads the retained view's current release preference.
    /// # Errors
    /// Returns the author's failure.
    pub fn wants_key_release(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        self.0.wants_key_release(emitter)
    }
    /// Invalidates the retained view.
    /// # Errors
    /// Returns the author's failure.
    pub fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.invalidate(emitter)
    }
}
