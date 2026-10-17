//! Queries over current shared extension registrations.
mod registration;
mod shortcuts;
mod ui;
use crate::{
    ErrorReporter, ExtensionExecution,
    types::{ExtensionCollection, ExtensionRuntimeState, ExtensionUIContext, read},
};
use maestro_models::Threaded;
use maestro_resources::ResourceDiagnostic;
use maestro_theme::LiveTheme;
use std::{
    cell::RefCell,
    sync::{Arc, RwLock},
};

/// Queries current owners while retaining registration and diagnostic identities.
pub struct ExtensionRunner<C, CC, E: ExtensionExecution = Threaded> {
    /// Current extension load order.
    extensions: ExtensionCollection<C, CC, E>,
    /// Shared flag values.
    runtime: Arc<ExtensionRuntimeState>,
    /// Warning destination invoked outside locks.
    warning_reporter: ErrorReporter<E>,
    /// Most recent command query's diagnostics.
    command_diagnostics: RwLock<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
    /// Most recent shortcut query's diagnostics.
    shortcut_diagnostics: RwLock<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
    /// The headless fallback restored by a reset.
    no_ui: ExtensionUIContext,
    /// The current UI context.
    ui: RefCell<ExtensionUIContext>,
}
impl<C, CC, E: ExtensionExecution> ExtensionRunner<C, CC, E> {
    /// Retains the supplied current collection, shared values and warning sink.
    ///
    /// Until a UI context is installed, the headless fallback supplies `theme`.
    #[must_use]
    pub fn new(
        extensions: ExtensionCollection<C, CC, E>,
        runtime: Arc<ExtensionRuntimeState>,
        warning_reporter: ErrorReporter<E>,
        theme: LiveTheme,
    ) -> Self {
        let no_ui = ExtensionUIContext::no_ui(theme);
        Self {
            extensions,
            runtime,
            warning_reporter,
            command_diagnostics: RwLock::default(),
            shortcut_diagnostics: RwLock::default(),
            ui: RefCell::new(no_ui.clone()),
            no_ui,
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
