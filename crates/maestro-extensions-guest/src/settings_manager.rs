//! Raw settings read through the host's original settings manager.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::bindings::maestro::extension::host::{HostOperation, OperationScope};
use crate::{ExtensionAPI, ExtensionFuture};
use serde_json::Value;

/// A reader of the user and project preference documents, not effective settings.
#[derive(Clone)]
pub struct SettingsManager {
    /// The factory's original host.
    api: ExtensionAPI,
}
impl SettingsManager {
    /// Retains the supplied host without opening settings files in the guest.
    #[must_use]
    pub fn new(api: ExtensionAPI) -> Self {
        Self { api }
    }
    /// Reads the raw user document, including unknown and wrong-typed values.
    ///
    /// # Errors
    /// Returns the host's error if the settings manager cannot be borrowed.
    #[must_use]
    pub fn get_global_settings(&self) -> ExtensionFuture<'_, Value> {
        self.api
            .call_host_operation(HostOperation::ReadSettings(OperationScope::User))
    }
    /// Reads the raw project document without merging the user document.
    ///
    /// # Errors
    /// Returns the host's error if the settings manager cannot be borrowed.
    #[must_use]
    pub fn get_project_settings(&self) -> ExtensionFuture<'_, Value> {
        self.api
            .call_host_operation(HostOperation::ReadSettings(OperationScope::Project))
    }
}
