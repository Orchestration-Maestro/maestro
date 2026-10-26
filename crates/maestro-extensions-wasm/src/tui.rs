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

port! {
    /// Requests to the terminal a factory received.
    TuiPort for TUI via _port {}
}

/// The terminal a component factory received; clones share its host owner.
#[derive(Clone)]
pub struct TUI {
    /// The owner of this capability, kept while the author holds the terminal.
    _port: Rc<dyn TuiPort>,
}
impl TUI {
    /// Retains the supplied host capability.
    #[must_use]
    pub fn new(port: Rc<dyn TuiPort>) -> Self {
        Self { _port: port }
    }
}

/// Disposal the host runs once when it removes a component, with callback-scoped emission.
pub type ComponentDisposer = Rc<dyn for<'a> Fn(CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// A component a factory built, with its optional disposal.
#[derive(Clone)]
pub struct ExtensionComponent {
    /// The component the host shows.
    pub component: Component,
    /// Runs once when the host removes the component.
    pub dispose: Option<ComponentDisposer>,
}

/// Builds a component from the terminal and the live theme, synchronously.
pub type ComponentFactory = Rc<
    dyn for<'a> Fn(TUI, crate::Theme, CallbackEmitter<'a>) -> ExtensionResult<ExtensionComponent>,
>;
