//! Load-time inputs offered by base package factories.
use crate::{EventError, ExtensionAPI, ExtensionExecution};
use serde_json::Value;

impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Offers package sources, base prompts, model files or base settings during base loading.
    ///
    /// # Errors
    /// Returns an error when the registration owner is no longer active.
    pub fn contribute_load_time(&self, _contributions: Value) -> Result<(), EventError> {
        self.assert_active()
    }
}
