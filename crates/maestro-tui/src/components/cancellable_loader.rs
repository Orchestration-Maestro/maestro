//! An input-cancellable wrapper around the existing loader.

use std::{cell::RefCell, rc::Rc};

use maestro_cancellation::Cancellation;

use super::Loader;
use crate::{Component, get_keybindings, tui::InputHandler};

/// A loader with a retained cancellation signal and an optional per-match callback.
pub struct CancellableLoader {
    /// Embedded animation and message owner.
    loader: Loader,
    /// Shared cooperative cancellation state.
    signal: Cancellation,
    /// Callback consulted for each matched input.
    on_abort: RefCell<Option<Rc<dyn Fn()>>>,
}
impl CancellableLoader {
    /// Wraps a loader with a fresh signal and no callback.
    #[must_use]
    pub fn new(loader: Loader) -> Self {
        Self {
            loader,
            signal: Cancellation::new(),
            on_abort: RefCell::new(None),
        }
    }
    /// Returns the embedded animation and message owner.
    #[must_use]
    pub fn loader(&self) -> &Loader {
        &self.loader
    }
    /// Borrows the shared cancellation signal.
    #[must_use]
    pub fn signal(&self) -> &Cancellation {
        &self.signal
    }
    /// Reads the signal's aborted flag.
    #[must_use]
    pub fn aborted(&self) -> bool {
        self.signal.is_aborted()
    }
    /// Retains the currently installed callback, if any.
    #[must_use]
    pub fn on_abort(&self) -> Option<Rc<dyn Fn()>> {
        self.on_abort.borrow().clone()
    }
    /// Replaces or removes the callback, dropping the old one outside the borrow.
    pub fn set_on_abort(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.on_abort.replace(callback);
        drop(old);
    }
    /// Stops the embedded loader without aborting its signal.
    pub fn dispose(&self) {
        self.loader.stop();
    }
}
impl InputHandler for CancellableLoader {
    fn handle_input(&self, data: &str) {
        if get_keybindings().matches(data, "tui.select.cancel") {
            self.signal.abort();
            if let Some(callback) = self.on_abort() {
                callback();
            }
        }
    }
}
impl Component for CancellableLoader {
    fn render(&self, width: usize) -> Vec<String> {
        self.loader.render(width)
    }
    fn invalidate(&self) {
        self.loader.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
}
