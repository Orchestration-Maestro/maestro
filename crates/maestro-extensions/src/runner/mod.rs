//! Queries over current shared extension registrations.
mod binding;
mod contexts;
mod registration;
mod shortcuts;
mod ui;
use crate::{
    ErrorReporter, ExtensionExecution,
    types::{ExtensionCollection, ExtensionRuntimeState, ExtensionUIContext, read},
};
pub(crate) use binding::ContextBinding;
pub use binding::ExtensionRunnerSession;
use maestro_models::Threaded;
use maestro_resources::ResourceDiagnostic;
use maestro_theme::LiveTheme;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Queries current owners while retaining registration and diagnostic identities.
pub struct ExtensionRunner<CC, E: ExtensionExecution = Threaded> {
    /// Current extension load order.
    extensions: ExtensionCollection<CC, E>,
    /// State shared with this runner's contexts.
    binding: Rc<ContextBinding>,
    /// Warning destination invoked outside locks.
    warning_reporter: ErrorReporter<E>,
    /// Most recent command query's diagnostics.
    command_diagnostics: RwLock<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
    /// Most recent shortcut query's diagnostics.
    shortcut_diagnostics: RwLock<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
}
impl<CC, E: ExtensionExecution> ExtensionRunner<CC, E> {
    /// Retains the supplied current collection, shared values, session and warning sink.
    ///
    /// Until a UI context is installed, the headless fallback supplies `theme`.
    #[must_use]
    pub fn new(
        extensions: ExtensionCollection<CC, E>,
        runtime: Arc<ExtensionRuntimeState>,
        session: ExtensionRunnerSession,
        warning_reporter: ErrorReporter<E>,
        theme: LiveTheme,
    ) -> Self {
        let no_ui = ExtensionUIContext::no_ui(theme);
        Self {
            extensions,
            binding: Rc::new(ContextBinding {
                runtime,
                session,
                actions: RefCell::default(),
                ui: RefCell::new(no_ui.clone()),
                no_ui,
            }),
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
