//! Attributed error delivery in live registration order.
use super::ExtensionRunner;
use crate::{
    ExtensionExecution,
    types::{ExtensionError, ExtensionErrorListener},
};
use maestro_models::records::listeners::Listeners;
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Adds a failure listener; the returned operation removes it.
    pub fn on_error(&self, listener: ExtensionErrorListener) -> impl Fn() + use<E> {
        Listeners::subscribe(&self.error_listeners, listener)
    }
    /// Reports `error` to each listener in registration order, including ones added meanwhile.
    /// # Errors
    /// Returns the first listener failure.
    pub fn emit_error(&self, error: &ExtensionError) -> Result<(), crate::EventError> {
        Listeners::try_for_each_live(&self.error_listeners, |listener| listener(error))
    }
}

/// Copies authored diagnostic fields without synthesizing engine stack text.
pub(super) fn queued_error(extension_path: String, error: &crate::EventError) -> ExtensionError {
    event_error(extension_path, "register_provider", error)
}

/// Projects callback diagnostics while preserving authored stack fields.
pub(super) fn event_error(
    extension_path: String,
    event: &str,
    error: &crate::EventError,
) -> ExtensionError {
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
        event: event.to_owned(),
        error: diagnostic.map_or_else(|| error.to_string(), |d| d.message.clone()),
        stack: diagnostic.and_then(|d| d.stack.clone()),
    }
}
