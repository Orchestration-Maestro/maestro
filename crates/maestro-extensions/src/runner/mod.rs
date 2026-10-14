//! Queries over current shared extension registrations.
mod registration;
mod shortcuts;
use crate::{
    ErrorReporter,
    types::{ExtensionCollection, ExtensionRuntimeState, read},
};
use maestro_resources::ResourceDiagnostic;
use std::sync::{Arc, RwLock};

/// Queries current owners while retaining registration and diagnostic identities.
pub struct ExtensionRunner<C, CC> {
    /// Current extension load order.
    extensions: ExtensionCollection<C, CC>,
    /// Shared flag values.
    runtime: Arc<ExtensionRuntimeState>,
    /// Warning destination invoked outside locks.
    warning_reporter: ErrorReporter,
    /// Most recent command query's diagnostics.
    command_diagnostics: RwLock<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
    /// Most recent shortcut query's diagnostics.
    shortcut_diagnostics: RwLock<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
}
impl<C, CC> ExtensionRunner<C, CC> {
    /// Retains the supplied current collection, shared values and warning sink.
    #[must_use]
    pub fn new(
        extensions: ExtensionCollection<C, CC>,
        runtime: Arc<ExtensionRuntimeState>,
        warning_reporter: ErrorReporter,
    ) -> Self {
        Self {
            extensions,
            runtime,
            warning_reporter,
            command_diagnostics: RwLock::default(),
            shortcut_diagnostics: RwLock::default(),
        }
    }
    /// Retains the most recently published command diagnostics.
    #[must_use]
    pub fn get_command_diagnostics(&self) -> Arc<RwLock<Vec<ResourceDiagnostic>>> {
        Arc::clone(&read(&self.command_diagnostics))
    }
    /// Retains the most recently published shortcut diagnostics.
    #[must_use]
    pub fn get_shortcut_diagnostics(&self) -> Arc<RwLock<Vec<ResourceDiagnostic>>> {
        Arc::clone(&read(&self.shortcut_diagnostics))
    }
}
