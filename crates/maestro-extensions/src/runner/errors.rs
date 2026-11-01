//! Attributed error delivery in live registration order.
use super::ExtensionRunner;
use crate::{
    ExtensionExecution,
    types::{ExtensionError, ExtensionErrorListener},
};
use maestro_listeners::Listeners;
use std::{cell::RefCell, rc::Rc};
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Adds a failure listener; the returned operation removes it.
    pub fn on_error(&self, listener: ExtensionErrorListener) -> impl Fn() + use<E> {
        Listeners::subscribe(&self.error_listeners, listener)
    }
    /// Reports `error` to each listener in registration order, including ones added meanwhile.
    /// # Errors
    /// Returns the first listener failure.
    pub fn emit_error(&self, error: &ExtensionError) -> Result<(), crate::EventError> {
        deliver(&self.error_listeners, error)
    }
    /// Shares this runner's existing listener owner with retained UI capabilities.
    pub(super) fn ui_error_reporter(&self) -> ExtensionErrorListener {
        let listeners = Rc::clone(&self.error_listeners);
        Rc::new(move |error| deliver(&listeners, error))
    }
}

/// Delivers through the one live listener collection, outside collection borrows.
fn deliver(
    listeners: &RefCell<Listeners<ExtensionErrorListener>>,
    error: &ExtensionError,
) -> Result<(), crate::EventError> {
    Listeners::try_for_each_live(listeners, |listener| listener(error))
}

/// Copies authored diagnostic fields without synthesizing engine stack text.
pub(super) fn queued_error(extension_path: String, error: &crate::EventError) -> ExtensionError {
    event_error(extension_path, "register_provider", error)
}

/// The supplied diagnostic a callback or operation failure retains, absent for a failure
/// known only by its text. This is the single list of error types with retained
/// diagnostic fields.
#[must_use]
pub fn retained_diagnostic<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> Option<&'a maestro_models::DiagnosticErrorInfo> {
    error
        .downcast_ref::<maestro_catalog::CatalogError>()
        .map(maestro_catalog::CatalogError::diagnostic)
        .or_else(|| error.downcast_ref::<maestro_models::DiagnosticErrorInfo>())
        .or_else(|| {
            error
                .downcast_ref::<maestro_models::OAuthError>()
                .map(|error| error.diagnostic.as_ref())
        })
}

/// Projects callback diagnostics while preserving authored stack fields.
pub(crate) fn event_error(
    extension_path: String,
    event: &str,
    error: &crate::EventError,
) -> ExtensionError {
    let diagnostic = retained_diagnostic(error.as_ref());
    ExtensionError {
        extension_path,
        event: event.to_owned(),
        error: diagnostic.map_or_else(|| error.to_string(), |d| d.message.clone()),
        stack: diagnostic.and_then(|d| d.stack.clone()),
    }
}
