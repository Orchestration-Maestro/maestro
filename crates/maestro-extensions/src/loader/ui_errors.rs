//! Retained UI failures use their original extension identity and runner listener owner.
use crate::{
    EventError, ExtensionAPI, ExtensionExecution, ExtensionUIContext,
    types::{UIErrorReporter, read},
};
use std::rc::Rc;

impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// The reporter of retained UI callback failures, attributed to this facade's extension.
    ///
    /// It keeps the runner's listener owner and the extension path, not `ui` itself.
    ///
    /// # Errors
    /// Returns a stale facade or a UI without a bound error reporter.
    pub fn ui_error_reporter(
        &self,
        ui: &ExtensionUIContext,
    ) -> Result<UIErrorReporter, EventError> {
        let owner = self.owner()?;
        let path = read(&owner).path.clone();
        let report = ui
            .error_reporter
            .clone()
            .ok_or("extension UI error reporting is not bound")?;
        Ok(Rc::new(move |event, error| {
            report(&crate::runner::event_error(path.clone(), event, error))
        }))
    }
}
