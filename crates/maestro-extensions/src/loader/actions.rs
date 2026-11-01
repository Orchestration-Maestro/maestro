//! Action methods of the extension API, delegating to the shared runtime.
use crate::{
    EventError, ExtensionExecution,
    types::{ExtensionAPI, ExtensionActions, ProviderConfig, read},
};
use maestro_models::{
    Model, ModelThinkingLevel as ThinkingLevel, SharedHandle, ToolInfo, UserContent,
};
use maestro_resources::SlashCommandInfo;
use maestro_session::messages::{CustomMessageInput, SendMessageOptions, SendUserMessageOptions};
#[cfg(not(target_arch = "wasm32"))]
use maestro_tools::{ExecOptions, ExecResult, exec_command};
use serde_json::Value;
use std::sync::{Arc, RwLock};

impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Checks activity, then returns one currently bound operation.
    fn action<F: ?Sized>(
        &self,
        select: fn(&ExtensionActions<E>) -> &E::Shared<F>,
    ) -> Result<E::Shared<F>, EventError> {
        self.owner()?;
        Ok(self.runtime.action(select))
    }
    /// Sends a custom message through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn send_message(
        &self,
        message: CustomMessageInput,
        options: Option<SendMessageOptions>,
    ) -> Result<(), EventError> {
        self.action(|actions| &actions.send_message)?(message, options)
    }
    /// Sends a user message through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> Result<(), EventError> {
        self.action(|actions| &actions.send_user_message)?(content, options)
    }
    /// Appends a custom entry through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn append_entry(&self, custom_type: String, data: Option<Value>) -> Result<(), EventError> {
        self.action(|actions| &actions.append_entry)?(custom_type, data)
    }
    /// Sets the session name through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn set_session_name(&self, name: String) -> Result<(), EventError> {
        self.action(|actions| &actions.set_session_name)?(name)
    }
    /// Reads the session name through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn get_session_name(&self) -> Result<Option<String>, EventError> {
        self.action(|actions| &actions.get_session_name)?()
    }
    /// Sets or clears an entry label through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn set_label(&self, entry_id: String, label: Option<String>) -> Result<(), EventError> {
        self.action(|actions| &actions.set_label)?(entry_id, label)
    }
    /// Runs `command` with `args` in `cwd`, or in the creation directory when `None`.
    ///
    /// # Errors
    /// Returns the stale error before starting.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn exec<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        cwd: Option<&'a str>,
        options: ExecOptions<'a>,
    ) -> Result<impl Future<Output = std::io::Result<ExecResult>> + 'a, EventError> {
        self.owner()?;
        Ok(exec_command(
            command,
            args,
            cwd.unwrap_or(&self.cwd),
            options,
        ))
    }
    /// Reads the active tool names through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn get_active_tools(&self) -> Result<Vec<String>, EventError> {
        self.action(|actions| &actions.get_active_tools)?()
    }
    /// Reads every configured tool through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn get_all_tools(
        &self,
    ) -> Result<
        Vec<ToolInfo<maestro_models::ParameterSchema<maestro_models::SchemaCode>>>,
        EventError,
    > {
        self.action(|actions| &actions.get_all_tools)?()
    }
    /// Selects the active tools through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn set_active_tools(&self, tool_names: Vec<String>) -> Result<(), EventError> {
        self.action(|actions| &actions.set_active_tools)?(tool_names)
    }
    /// Reads the available commands through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn get_commands(&self) -> Result<Vec<SlashCommandInfo>, EventError> {
        self.action(|actions| &actions.get_commands)?()
    }
    /// Starts the bound model selection; it resolves whether the model was accepted.
    ///
    /// # Errors
    /// Returns the stale error before starting.
    pub fn set_model(
        &self,
        model: Arc<RwLock<Model>>,
    ) -> Result<E::Future<'static, Result<bool, EventError>>, EventError> {
        Ok(self.action(|actions| &actions.set_model)?(model))
    }
    /// Reads the thinking level through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn get_thinking_level(&self) -> Result<ThinkingLevel, EventError> {
        self.action(|actions| &actions.get_thinking_level)?()
    }
    /// Sets the thinking level through the bound operation.
    ///
    /// # Errors
    /// Returns the stale error, the not-initialized error or the operation's error.
    pub fn set_thinking_level(&self, level: ThinkingLevel) -> Result<(), EventError> {
        self.action(|actions| &actions.set_thinking_level)?(level)
    }
    /// Registers a provider attributed to this extension.
    ///
    /// The configuration is the platform handle, retained as the browser's local owner or
    /// the native shared one.
    ///
    /// # Errors
    /// Returns the stale error, or the bound route's error after binding.
    pub fn register_provider(
        &self,
        name: String,
        config: SharedHandle<RwLock<ProviderConfig>>,
    ) -> Result<(), EventError> {
        let owner = self.owner()?;
        let path = read(&owner).path.clone();
        self.runtime.register_provider(name, config, Some(path))
    }
    /// Unregisters a provider.
    ///
    /// # Errors
    /// Returns the stale error, or the bound route's error after binding.
    pub fn unregister_provider(&self, name: &str) -> Result<(), EventError> {
        let owner = self.owner()?;
        let path = read(&owner).path.clone();
        self.runtime.unregister_provider_from(name, Some(&path))
    }
}
