use std::{future::Future, pin::Pin};

pub use crate::bindings::maestro::extension::types::Error;

/// An asynchronous extension operation retaining its captures until completion.
pub type ExtensionFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + 'a>>;

use crate::{
    AbortSignal, AgentToolResult, AutocompleteItem, CustomMessage, EventBus,
    ExtensionCommandContext, ExtensionContext, ExtensionEvent, ExtensionEventResult,
    ExtensionFlagOptions, FlagValue, Model, RenderShell, SendMessageOptions,
    SendUserMessageOptions, SlashCommandInfo, ThinkingLevel, ToolExecutionMode, ToolInfo,
    UserContent,
};
use serde_json::Value;
use std::rc::Rc;

/// Extension Return.
pub enum ExtensionReturn<'a, T> {
    /// Immediate.
    Immediate(Result<T, Error>),
    /// Future.
    Future(ExtensionFuture<'a, T>),
}
/// Extension Factory.
pub type ExtensionFactory = Rc<dyn Fn(Rc<dyn ExtensionAPI>) -> ExtensionReturn<'static, ()>>;
/// Extension Handler.
pub type ExtensionHandler<E, R = ()> =
    Rc<dyn for<'a> Fn(&'a mut E, Rc<dyn ExtensionContext>) -> ExtensionReturn<'a, Option<R>>>;
/// Agent Tool Update Callback.
pub type AgentToolUpdateCallback = Rc<dyn Fn(AgentToolResult) -> Result<(), Error>>;
/// Extension API.
pub trait ExtensionAPI {
    /// On.
    fn on(
        &self,
        event: &str,
        handler: ExtensionHandler<ExtensionEvent, ExtensionEventResult>,
    ) -> Result<(), Error>;
    /// On named.
    fn on_named(&self, event: &str, handler: ExtensionHandler<Value, Value>) -> Result<(), Error>;
    /// Register tool.
    fn register_tool(&self, tool: ToolDefinition) -> Result<(), Error>;
    /// Register command.
    fn register_command(&self, name: &str, options: RegisteredCommandOptions) -> Result<(), Error>;
    /// Register shortcut.
    fn register_shortcut(
        &self,
        shortcut: &str,
        options: ExtensionShortcutOptions,
    ) -> Result<(), Error>;
    /// Register flag.
    fn register_flag(&self, name: &str, options: ExtensionFlagOptions) -> Result<(), Error>;
    /// Get flag.
    fn get_flag(&self, name: &str) -> Result<Option<FlagValue>, Error>;
    /// Send message.
    fn send_message(
        &self,
        message: CustomMessage,
        options: Option<SendMessageOptions>,
    ) -> Result<(), Error>;
    /// Send user message.
    fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> Result<(), Error>;
    /// Append entry.
    fn append_entry(&self, custom_type: &str, data: Option<Value>) -> Result<(), Error>;
    /// Set session name.
    fn set_session_name(&self, name: &str) -> Result<(), Error>;
    /// Get session name.
    fn get_session_name(&self) -> Result<Option<String>, Error>;
    /// Set label.
    fn set_label(&self, entry_id: &str, label: Option<&str>) -> Result<(), Error>;
    /// Get active tools.
    fn get_active_tools(&self) -> Result<Vec<String>, Error>;
    /// Get all tools.
    fn get_all_tools(&self) -> Result<Vec<ToolInfo>, Error>;
    /// Set active tools.
    fn set_active_tools(&self, names: &[String]) -> Result<(), Error>;
    /// Get commands.
    fn get_commands(&self) -> Result<Vec<SlashCommandInfo>, Error>;
    /// Set model.
    fn set_model(&self, model: Model) -> ExtensionFuture<'_, bool>;
    /// Get thinking level.
    fn get_thinking_level(&self) -> Result<ThinkingLevel, Error>;
    /// Set thinking level.
    fn set_thinking_level(&self, level: ThinkingLevel) -> Result<(), Error>;
    /// Events.
    fn events(&self) -> Rc<dyn EventBus>;
}
type ArgumentCompletions =
    Rc<dyn Fn(String) -> ExtensionReturn<'static, Option<Vec<AutocompleteItem>>>>;
type CommandHandler =
    Rc<dyn Fn(String, Rc<dyn ExtensionCommandContext>) -> ExtensionFuture<'static, ()>>;
type ShortcutHandler = Rc<dyn Fn(Rc<dyn ExtensionContext>) -> ExtensionReturn<'static, ()>>;
type ArgumentPreparation = Rc<dyn Fn(Value) -> Result<Value, Error>>;
type ToolExecute = Rc<
    dyn Fn(
        String,
        Value,
        Option<Rc<dyn AbortSignal>>,
        Option<AgentToolUpdateCallback>,
        Rc<dyn ExtensionContext>,
    ) -> ExtensionFuture<'static, AgentToolResult>,
>;

/// Registered Command Options.
pub struct RegisteredCommandOptions {
    /// Description.
    pub description: Option<String>,
    /// Get argument completions.
    pub get_argument_completions: Option<ArgumentCompletions>,
    /// Handler.
    pub handler: CommandHandler,
}
/// Extension Shortcut Options.
pub struct ExtensionShortcutOptions {
    /// Description.
    pub description: Option<String>,
    /// Handler.
    pub handler: ShortcutHandler,
}
/// Tool Definition.
pub struct ToolDefinition {
    /// Name.
    pub name: String,
    /// Label.
    pub label: String,
    /// Description.
    pub description: String,
    /// Prompt snippet.
    pub prompt_snippet: Option<String>,
    /// Prompt guidelines.
    pub prompt_guidelines: Option<Vec<String>>,
    /// Parameters.
    pub parameters: Value,
    /// Render shell.
    pub render_shell: Option<RenderShell>,
    /// Prepare arguments.
    pub prepare_arguments: Option<ArgumentPreparation>,
    /// Execution mode.
    pub execution_mode: Option<ToolExecutionMode>,
    /// Execute.
    pub execute: ToolExecute,
}

/// Return a tool definition unchanged, without validating its arguments.
pub fn define_tool(tool: ToolDefinition) -> ToolDefinition {
    tool
}
