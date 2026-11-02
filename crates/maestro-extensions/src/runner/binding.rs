//! Runner state shared with its contexts, and the operations that bind it.
use super::ExtensionRunner;
use crate::loader::runtime::BoundProviders;
use crate::types::{ExtensionActions, ProviderActions, write};
use crate::{
    EventError, ExtensionExecution,
    types::{
        ActivityGuard, ExtensionCommandContextActions, ExtensionContextActions, ExtensionUIContext,
        ExtensionUIContextBinding,
    },
};
use maestro_catalog::ModelRegistry;
use maestro_models::SharedHandle;
use maestro_session::session_manager::ReadonlySessionManager;
use std::{cell::RefCell, rc::Rc};

/// The session a runner serves, fixed when the runner is created.
pub struct ExtensionRunnerSession {
    /// Working directory reported to contexts.
    pub cwd: String,
    /// Live session history, read-only to extensions.
    pub session_manager: Rc<RefCell<dyn ReadonlySessionManager>>,
    /// Live model catalog.
    pub model_registry: SharedHandle<ModelRegistry>,
}

/// Runner state that its contexts read when used.
pub(crate) struct ContextBinding {
    /// This runner's own activity; a later runner over the same runtime keeps its own.
    pub(super) activity: ActivityGuard,
    /// The served session.
    pub(super) session: ExtensionRunnerSession,
    /// Currently bound application operations.
    pub(super) actions: RefCell<ExtensionContextActions>,
    /// Currently bound command operations.
    pub(super) command_actions: RefCell<ExtensionCommandContextActions>,
    /// The headless fallback restored by a reset.
    pub(super) no_ui: ExtensionUIContext,
    /// The current UI context.
    pub(super) ui: RefCell<ExtensionUIContextBinding>,
    /// The callback bound once to this runner's current UI acquisition.
    pub(super) ui_error_reporter: RefCell<Option<crate::types::ExtensionErrorListener>>,
}

impl ContextBinding {
    /// Returns the current UI context; it keeps its host after later changes.
    pub(super) fn get_ui_context(&self) -> ExtensionUIContext {
        let mut context = self.ui.borrow().get().unwrap_or_else(|| self.no_ui.clone());
        context
            .error_reporter
            .clone_from(&self.ui_error_reporter.borrow());
        context
    }
    /// Whether an installed context, rather than a headless fallback, is current.
    pub(super) fn has_ui(&self) -> bool {
        self.ui.borrow().has_ui()
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Binds the actions behind every API and context, then applies queued providers.
    ///
    /// Each failed queued registration is reported to the error listeners with its
    /// extension path, and the next one is applied. Afterwards registrations take
    /// effect immediately, through `provider_actions` or else the catalog.
    /// # Errors
    /// Returns a listener failure without replacing it.
    pub fn bind_core(
        &self,
        actions: ExtensionActions<E>,
        context_actions: ExtensionContextActions,
        provider_actions: ProviderActions<E>,
    ) -> Result<(), crate::EventError> {
        let replaced = std::mem::replace(&mut *write(&self.runtime.actions), actions);
        drop(replaced);
        drop(self.binding.actions.replace(context_actions));
        let bound = BoundProviders {
            actions: provider_actions,
            catalog: SharedHandle::downgrade(&self.binding.session.model_registry),
        };
        let mut cursor = 0;
        while let Some((id, pending)) = self.runtime.next_or_bind(cursor, &bound) {
            cursor = id + 1;
            if let Err(error) = bound.register_provider(pending.name, pending.config) {
                self.emit_error(&super::errors::queued_error(pending.extension_path, &error))?;
            } else {
                let retired = write(&self.runtime.pending_provider_registrations).remove(&id);
                drop(retired);
            }
        }
        Ok(())
    }
    /// Invalidates this runner's contexts and its runtime's registration, each keeping its
    /// first message.
    ///
    /// `None` uses the stale-context guidance.
    pub fn invalidate(&self, message: Option<String>) {
        self.binding.activity.invalidate(message.clone());
        self.runtime.invalidate(message);
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
                .replace(actions.unwrap_or_else(ExtensionCommandContextActions::unbound)),
        );
    }
}
