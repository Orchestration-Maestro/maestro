//! Queries over current shared extension registrations.
mod agent_start;
pub use agent_start::BeforeAgentStartCombinedResult;
mod binding;
mod contexts;
mod dispatch;
pub use dispatch::{RunnerEmitEvent, emit_session_shutdown_event};
mod errors;
pub(crate) use errors::event_error;
pub use errors::retained_diagnostic;
mod input;
mod registration;
mod resources;
pub use resources::{ResourcePath, ResourcesDiscoverCombinedResult};
mod shortcuts;
mod tool_hooks;
mod transforms;
mod ui;
use crate::{
    ErrorReporter, ExtensionCommandContextActions, ExtensionExecution,
    types::{ActivityGuard, ExtensionCollection, ExtensionRuntime, ExtensionUIContext, read},
};
pub(crate) use binding::ContextBinding;
pub use binding::ExtensionRunnerSession;
use maestro_listeners::Listeners;
use maestro_models::Threaded;
use maestro_resources::ResourceDiagnostic;
use maestro_theme::LiveTheme;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Queries current owners while retaining registration and diagnostic identities.
pub struct ExtensionRunner<E: ExtensionExecution = Threaded> {
    /// Shared action runtime.
    runtime: E::Shared<ExtensionRuntime<E>>,
    /// Live attributed error observers.
    error_listeners: Rc<RefCell<Listeners<crate::types::ExtensionErrorListener>>>,
    /// Current extension load order.
    extensions: ExtensionCollection<E>,
    /// State shared with this runner's contexts.
    binding: Rc<ContextBinding>,
    /// Warning destination invoked outside locks.
    warning_reporter: ErrorReporter<E>,
    /// Most recent command query's diagnostics.
    command_diagnostics: RwLock<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
    /// Most recent shortcut query's diagnostics.
    shortcut_diagnostics: RwLock<Arc<RwLock<Vec<ResourceDiagnostic>>>>,
}
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Retains the supplied current collection, shared values, session and warning sink.
    ///
    /// Until a UI context is installed, the headless fallback supplies `theme`.
    #[must_use]
    pub fn new(
        extensions: ExtensionCollection<E>,
        runtime: E::Shared<ExtensionRuntime<E>>,
        session: ExtensionRunnerSession,
        warning_reporter: ErrorReporter<E>,
        theme: LiveTheme,
    ) -> Self {
        let no_ui = ExtensionUIContext::no_ui(theme);
        Self {
            extensions,
            runtime,
            error_listeners: Rc::default(),
            binding: Rc::new(ContextBinding {
                activity: ActivityGuard::default(),
                session,
                actions: RefCell::default(),
                command_actions: RefCell::new(ExtensionCommandContextActions::unbound()),
                ui: RefCell::new(no_ui.clone().into()),
                ui_error_reporter: RefCell::default(),
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
