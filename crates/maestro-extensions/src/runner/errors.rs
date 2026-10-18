//! Attributed error delivery in live registration order.
use super::ExtensionRunner;
use crate::{
    ExtensionExecution,
    types::{ExtensionError, ExtensionErrorListener},
};
use std::rc::Rc;
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Adds a failure listener; the returned operation removes it.
    pub fn on_error(&self, listener: ExtensionErrorListener) -> impl Fn() + use<E> {
        let identity = Rc::downgrade(&listener);
        let listeners = Rc::downgrade(&self.error_listeners);
        self.error_listeners.borrow_mut().add(listener);
        move || {
            if let Some(listeners) = listeners.upgrade() {
                let removed = listeners.borrow_mut().remove(identity.as_ptr());
                drop(removed);
            }
        }
    }
    /// Reports `error` to each listener in registration order, including ones added meanwhile.
    /// # Errors
    /// Returns the first listener failure.
    pub fn emit_error(&self, error: &ExtensionError) -> Result<(), crate::EventError> {
        let mut cursor = 0;
        loop {
            let next = self
                .error_listeners
                .borrow()
                .next_from(cursor)
                .map(|(next, listener)| (next, Rc::clone(listener)));
            let Some((next, listener)) = next else {
                break;
            };
            cursor = next;
            listener(error)?;
        }
        Ok(())
    }
}

/// Copies authored diagnostic fields without synthesizing engine stack text.
pub(super) fn queued_error(extension_path: String, error: &crate::EventError) -> ExtensionError {
    // This projection is the single list of error types with retained diagnostic stacks.
    let diagnostic = error
        .downcast_ref::<maestro_catalog::CatalogError>()
        .map(maestro_catalog::CatalogError::diagnostic)
        .or_else(|| error.downcast_ref::<maestro_models::DiagnosticErrorInfo>())
        .or_else(|| {
            error
                .downcast_ref::<maestro_models::OAuthError>()
                .map(|error| error.diagnostic.as_ref())
        });
    ExtensionError {
        extension_path,
        event: "register_provider".to_owned(),
        error: diagnostic.map_or_else(|| error.to_string(), |d| d.message.clone()),
        stack: diagnostic.and_then(|d| d.stack.clone()),
    }
}
