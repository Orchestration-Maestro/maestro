//! Base context creation and its guarded operations.
use super::{ContextBinding, ExtensionRunner};
use crate::{
    EventError, ExtensionExecution,
    types::{
        CompactOptions, ContextUsage, ExtensionContext, ExtensionContextActions, ExtensionUIContext,
    },
};
use maestro_catalog::ModelRegistry;
use maestro_models::{Cancellation, Model};
use maestro_session::session_manager::ReadonlySessionManager;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

impl<CC, E: ExtensionExecution> ExtensionRunner<CC, E> {
    /// Creates a context that reads this runner's state when used.
    ///
    /// The model reader is the one bound now; later bindings do not replace it.
    #[must_use]
    pub fn create_context(&self) -> ExtensionContext {
        ExtensionContext {
            get_model: Rc::clone(&self.binding.actions.borrow().get_model),
            binding: Rc::clone(&self.binding),
        }
    }
}

impl ContextBinding {
    /// Returns the selected bound operation after the stale check.
    fn action<F: ?Sized>(
        &self,
        select: fn(&ExtensionContextActions) -> &Rc<F>,
    ) -> Result<Rc<F>, EventError> {
        self.runtime.assert_active()?;
        Ok(Rc::clone(select(&self.actions.borrow())))
    }
}

impl ExtensionContext {
    /// Returns the current UI context.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn ui(&self) -> Result<ExtensionUIContext, EventError> {
        self.binding.runtime.assert_active()?;
        Ok(self.binding.get_ui_context())
    }
    /// Whether an installed UI context is current.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn has_ui(&self) -> Result<bool, EventError> {
        self.binding.runtime.assert_active()?;
        Ok(self.binding.has_ui())
    }
    /// Returns the runner's working directory.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn cwd(&self) -> Result<&str, EventError> {
        self.binding.runtime.assert_active()?;
        Ok(&self.binding.session.cwd)
    }
    /// Shares the runner's read-only session history.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn session_manager(&self) -> Result<Rc<RefCell<dyn ReadonlySessionManager>>, EventError> {
        self.binding.runtime.assert_active()?;
        Ok(Rc::clone(&self.binding.session.session_manager))
    }
    /// Shares the runner's model catalog.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn model_registry(&self) -> Result<Rc<RefCell<ModelRegistry>>, EventError> {
        self.binding.runtime.assert_active()?;
        Ok(Rc::clone(&self.binding.session.model_registry))
    }
    /// Reads the current model through the reader captured at creation.
    ///
    /// # Errors
    /// Returns the stale error or the reader's error.
    pub fn model(&self) -> Result<Option<Arc<RwLock<Model>>>, EventError> {
        self.binding.runtime.assert_active()?;
        (self.get_model)()
    }
    /// Whether the agent is not streaming.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn is_idle(&self) -> Result<bool, EventError> {
        (self.binding.action(|actions| &actions.is_idle)?)()
    }
    /// Returns the current operation's cancellation, if streaming.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn signal(&self) -> Result<Option<Cancellation>, EventError> {
        (self.binding.action(|actions| &actions.get_signal)?)()
    }
    /// Aborts the current agent operation.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn abort(&self) -> Result<(), EventError> {
        (self.binding.action(|actions| &actions.abort)?)()
    }
    /// Whether queued messages are waiting.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn has_pending_messages(&self) -> Result<bool, EventError> {
        (self
            .binding
            .action(|actions| &actions.has_pending_messages)?)()
    }
    /// Requests a graceful shutdown.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn shutdown(&self) -> Result<(), EventError> {
        (self.binding.action(|actions| &actions.shutdown)?)()
    }
    /// Returns the current context usage, if known.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn get_context_usage(&self) -> Result<Option<ContextUsage>, EventError> {
        (self.binding.action(|actions| &actions.get_context_usage)?)()
    }
    /// Starts compaction and returns without waiting for it.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn compact(&self, options: Option<CompactOptions>) -> Result<(), EventError> {
        (self.binding.action(|actions| &actions.compact)?)(options)
    }
    /// Returns the current effective system prompt.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn get_system_prompt(&self) -> Result<String, EventError> {
        (self.binding.action(|actions| &actions.get_system_prompt)?)()
    }
}
