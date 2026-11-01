//! Factory-time settings and package operations supplied by the application.
use crate::types::{read, write};
use crate::{ExtensionAPI, ExtensionExecution};
use serde_json::Value;

/// Scope of an installed source or raw preference document.
#[derive(Clone, Copy, Debug)]
pub enum OperationScope {
    /// User configuration.
    User,
    /// Project configuration.
    Project,
}

/// One operation on the loader's original settings and package owners.
#[derive(Clone, Debug)]
pub enum HostOperation {
    /// Reads a raw preference document, without effective overrides.
    ReadSettings(OperationScope),
    /// Resolves configured packages, or the supplied sources in one scope, including
    /// packages without resources. Supplied sources are never written to settings.
    ResolvePackages(Option<(Vec<String>, OperationScope)>),
    /// Acquires contents without editing preferences.
    Acquire(String, OperationScope),
    /// Observes a managed pinned Git checkout without changing it.
    VerifyPin(String, OperationScope),
    /// Restores a managed pinned Git checkout through the package owner.
    RestorePin(String, OperationScope),
}

impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Calls the application operation installed before factory loading.
    ///
    /// # Errors
    /// Returns missing-service, owner or native-operation failures.
    pub fn host_operation(
        &self,
        operation: HostOperation,
    ) -> E::Future<'static, Result<Value, String>> {
        let selected = self
            .owner()
            .map_err(|error| error.to_string())
            .and_then(|_| {
                read(&self.runtime.host_operations)
                    .clone()
                    .ok_or_else(|| "Extension host operations are not configured".to_owned())
            });
        match selected {
            Ok(call) => call(operation),
            Err(error) => E::failed_host_operation(error),
        }
    }
}

impl<E: ExtensionExecution> crate::ExtensionRuntime<E> {
    /// Installs factory-time operations without binding session actions.
    pub fn set_host_operations(&self, operations: E::Shared<E::HostOperation>) {
        let previous = write(&self.host_operations).replace(operations);
        drop(previous);
    }
}
