//! Runner state shared with its contexts, and the operations that bind it.
use super::ExtensionRunner;
use crate::{
    EventError, ExtensionExecution,
    types::{
        ExtensionCommandContextActions, ExtensionContextActions, ExtensionRuntimeState,
        ExtensionUIContext,
    },
};
use maestro_catalog::ModelRegistry;
use maestro_session::session_manager::ReadonlySessionManager;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// The session a runner serves, fixed when the runner is created.
pub struct ExtensionRunnerSession {
    /// Working directory reported to contexts.
    pub cwd: String,
    /// Live session history, read-only to extensions.
    pub session_manager: Rc<RefCell<dyn ReadonlySessionManager>>,
    /// Live model catalog.
    pub model_registry: Rc<RefCell<ModelRegistry>>,
}

/// Runner state that its contexts read when used.
pub(crate) struct ContextBinding {
    /// Shared flag values and activity guard.
    pub(super) runtime: Arc<ExtensionRuntimeState>,
    /// The served session.
    pub(super) session: ExtensionRunnerSession,
    /// Currently bound application operations.
    pub(super) actions: RefCell<ExtensionContextActions>,
    /// Currently bound command operations.
    pub(super) command_actions: RefCell<ExtensionCommandContextActions>,
    /// The headless fallback restored by a reset.
    pub(super) no_ui: ExtensionUIContext,
    /// The current UI context.
    pub(super) ui: RefCell<ExtensionUIContext>,
}

impl ContextBinding {
    /// Returns the current UI context; it keeps its host after later changes.
    pub(super) fn get_ui_context(&self) -> ExtensionUIContext {
        self.ui.borrow().clone()
    }
    /// Whether an installed context, rather than a headless fallback, is current.
    pub(super) fn has_ui(&self) -> bool {
        !self.ui.borrow().fallback
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Binds the application operations read by current and later contexts.
    pub fn bind_core(&self, context_actions: ExtensionContextActions) {
        drop(self.binding.actions.replace(context_actions));
    }
    /// Invalidates this runner's contexts and registration, keeping the first message.
    ///
    /// `None` uses the stale-context guidance.
    pub fn invalidate(&self, message: Option<String>) {
        self.binding.runtime.invalidate(message);
    }
    /// Requests a graceful shutdown through the bound operation, even when stale.
    ///
    /// # Errors
    /// Returns the bound operation's error.
    pub fn shutdown(&self) -> Result<(), EventError> {
        let shutdown = Rc::clone(&self.binding.actions.borrow().shutdown);
        shutdown()
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Binds the command operations read by current and later command contexts.
    ///
    /// `None` restores the defaults: no-op waits and reloads, uncancelled results.
    pub fn bind_command_context(&self, actions: Option<ExtensionCommandContextActions>) {
        drop(
            self.binding
                .command_actions
                .replace(actions.unwrap_or_default()),
        );
    }
}
