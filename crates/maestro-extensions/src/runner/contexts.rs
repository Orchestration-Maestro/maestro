//! Base context creation and its guarded operations.
use super::{ContextBinding, ExtensionRunner};
use crate::{
    EventError, ExtensionExecution,
    types::{
        CompactOptions, ContextFuture, ContextUsage, ExtensionCommandContext,
        ExtensionCommandContextActions, ExtensionContext, ExtensionUIContext, ForkOptions,
        NewSessionCommandOptions, SessionChangeResult, SwitchSessionOptions,
    },
};
use maestro_catalog::ModelRegistry;
use maestro_models::{Cancellation, Model, SharedHandle};
use maestro_session::session_manager::{NavigateTreeOptions, ReadonlySessionManager};
use std::future::ready;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Creates a context that reads this runner's state when used.
    ///
    /// The model reader is the one bound now; later bindings do not replace it.
    #[must_use]
    pub fn create_context(&self) -> ExtensionContext {
        ExtensionContext {
            get_model: Rc::clone(&self.binding.actions.borrow().get_model),
            binding: Rc::clone(&self.binding),
            invocation_system_prompt: None,
        }
    }
}

impl ContextBinding {
    /// Returns the selected bound operation after the stale check.
    fn action<A, F: ?Sized>(
        &self,
        cell: &RefCell<A>,
        select: fn(&A) -> &Rc<F>,
    ) -> Result<Rc<F>, EventError> {
        self.activity.assert_active()?;
        Ok(Rc::clone(select(&cell.borrow())))
    }
}

impl ExtensionContext {
    /// Returns the current UI context.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn ui(&self) -> Result<ExtensionUIContext, EventError> {
        self.binding.activity.assert_active()?;
        Ok(self.binding.get_ui_context())
    }
    /// Whether an installed UI context is current.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn has_ui(&self) -> Result<bool, EventError> {
        self.binding.activity.assert_active()?;
        Ok(self.binding.has_ui())
    }
    /// Returns the runner's working directory.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn cwd(&self) -> Result<&str, EventError> {
        self.binding.activity.assert_active()?;
        Ok(&self.binding.session.cwd)
    }
    /// Shares the runner's read-only session history.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn session_manager(&self) -> Result<Rc<RefCell<dyn ReadonlySessionManager>>, EventError> {
        self.binding.activity.assert_active()?;
        Ok(Rc::clone(&self.binding.session.session_manager))
    }
    /// Shares the runner's model catalog.
    ///
    /// # Errors
    /// Returns the stale error.
    pub fn model_registry(&self) -> Result<SharedHandle<ModelRegistry>, EventError> {
        self.binding.activity.assert_active()?;
        Ok(SharedHandle::clone(&self.binding.session.model_registry))
    }
    /// Reads the current model through the reader captured at creation.
    ///
    /// # Errors
    /// Returns the stale error or the reader's error.
    pub fn model(&self) -> Result<Option<Arc<RwLock<Model>>>, EventError> {
        self.binding.activity.assert_active()?;
        (self.get_model)()
    }
    /// Whether the agent is not streaming.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn is_idle(&self) -> Result<bool, EventError> {
        (self
            .binding
            .action(&self.binding.actions, |actions| &actions.is_idle)?)()
    }
    /// Returns the current operation's cancellation, if streaming.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn signal(&self) -> Result<Option<Cancellation>, EventError> {
        (self
            .binding
            .action(&self.binding.actions, |actions| &actions.get_signal)?)()
    }
    /// Aborts the current agent operation.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn abort(&self) -> Result<(), EventError> {
        (self
            .binding
            .action(&self.binding.actions, |actions| &actions.abort)?)()
    }
    /// Whether queued messages are waiting.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn has_pending_messages(&self) -> Result<bool, EventError> {
        (self.binding.action(&self.binding.actions, |actions| {
            &actions.has_pending_messages
        })?)()
    }
    /// Requests a graceful shutdown.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn shutdown(&self) -> Result<(), EventError> {
        (self
            .binding
            .action(&self.binding.actions, |actions| &actions.shutdown)?)()
    }
    /// Returns the current context usage, if known.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn get_context_usage(&self) -> Result<Option<ContextUsage>, EventError> {
        (self
            .binding
            .action(&self.binding.actions, |actions| &actions.get_context_usage)?)()
    }
    /// Starts compaction and returns without waiting for it.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn compact(&self, options: Option<CompactOptions>) -> Result<(), EventError> {
        (self
            .binding
            .action(&self.binding.actions, |actions| &actions.compact)?)(options)
    }
    /// Returns the current effective system prompt.
    ///
    /// # Errors
    /// Returns the stale error or the bound operation's error.
    pub fn get_system_prompt(&self) -> Result<String, EventError> {
        if let Some(prompt) = &self.invocation_system_prompt {
            self.binding.activity.assert_active()?;
            return Ok(prompt.borrow().clone());
        }
        (self
            .binding
            .action(&self.binding.actions, |actions| &actions.get_system_prompt)?)()
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Creates a command context that reads this runner's state when used.
    ///
    /// Its model reader is the one bound now, as for [`Self::create_context`].
    #[must_use]
    pub fn create_command_context(&self) -> ExtensionCommandContext {
        ExtensionCommandContext {
            context: self.create_context(),
        }
    }
}

impl ExtensionCommandContext {
    /// Selects and starts the bound operation now, or yields the stale error.
    fn start<F: ?Sized, T: 'static>(
        &self,
        select: fn(&ExtensionCommandContextActions) -> &Rc<F>,
        run: impl FnOnce(&F) -> ContextFuture<T>,
    ) -> ContextFuture<T> {
        let binding = &self.context.binding;
        match binding.action(&binding.command_actions, select) {
            Ok(operation) => run(&operation),
            Err(error) => Box::pin(ready(Err(error))),
        }
    }
    /// Waits for the agent to finish streaming.
    ///
    /// The returned future yields the stale error or the bound operation's result.
    #[must_use]
    pub fn wait_for_idle(&self) -> ContextFuture<()> {
        self.start(|actions| &actions.operations().wait_for_idle, |wait| wait())
    }
    /// Starts a new session.
    #[must_use]
    pub fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ContextFuture<SessionChangeResult> {
        self.start(
            |actions| &actions.operations().new_session,
            |start| start(options),
        )
    }
    /// Forks the session from an entry.
    #[must_use]
    pub fn fork(
        &self,
        entry_id: String,
        options: Option<ForkOptions>,
    ) -> ContextFuture<SessionChangeResult> {
        self.start(
            |actions| &actions.operations().fork,
            |fork| fork(entry_id, options),
        )
    }
    /// Moves to another point of the session tree.
    #[must_use]
    pub fn navigate_tree(
        &self,
        target_id: String,
        options: Option<NavigateTreeOptions>,
    ) -> ContextFuture<SessionChangeResult> {
        self.start(
            |actions| &actions.operations().navigate_tree,
            |navigate| navigate(target_id, options),
        )
    }
    /// Switches to another session file.
    #[must_use]
    pub fn switch_session(
        &self,
        session_path: String,
        options: Option<SwitchSessionOptions>,
    ) -> ContextFuture<SessionChangeResult> {
        self.start(
            |actions| &actions.operations().switch_session,
            |switch| switch(session_path, options),
        )
    }
    /// Reloads extensions, skills, prompts and themes.
    #[must_use]
    pub fn reload(&self) -> ContextFuture<()> {
        self.start(|actions| &actions.operations().reload, |reload| reload())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl maestro_tools::ReadContext for ExtensionContext {
    /// The current model read through the captured reader; a stale context knows none.
    fn model(&self) -> Option<Arc<Model>> {
        let model = ExtensionContext::model(self).ok().flatten()?;
        let model = crate::types::read(&model).clone();
        Some(Arc::new(model))
    }
}
