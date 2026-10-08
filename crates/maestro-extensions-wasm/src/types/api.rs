//! The extension API facade and the port both adapters implement.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use serde_json::Value;

use super::context::{ExtensionCommandContext, ExtensionContext, Theme};
use super::events::ExtensionHandler;
use super::extension_result::{ExtensionFuture, ExtensionResult};
use super::tools::ToolDefinition;
use crate::agent::ThinkingLevel;
use crate::bindings::exports::maestro::extension::guest::MessageRenderOptions;
use crate::bindings::maestro::extension::host::{
    FlagOptions, FlagValue, SendMessageOptions, SendUserMessageOptions,
};
use crate::bindings::maestro::extension::session::ToolInfo;
use crate::event_bus::{CallbackEmitter, EventBus};
use crate::messages::{CustomMessage, CustomMessageInput};
use crate::models::{Model, UserContent};
use crate::slash_commands::SlashCommandInfo;
use crate::tui::{AutocompleteItem, Component};

/// Handler of a registered command: receives the argument text and a command context.
pub type CommandHandler =
    Rc<dyn Fn(String, ExtensionCommandContext) -> ExtensionFuture<'static, ()>>;

/// Completes the argument text of a command; `None` means there are no suggestions.
pub type ArgumentCompletions =
    Rc<dyn Fn(String) -> ExtensionFuture<'static, Option<Vec<AutocompleteItem>>>>;

/// Registration data of a command.
pub struct CommandOptions {
    /// Description shown to users.
    pub description: Option<String>,
    /// Completes the arguments of the command.
    pub get_argument_completions: Option<ArgumentCompletions>,
    /// Runs when the command is invoked.
    pub handler: CommandHandler,
}

/// Handler of a keyboard shortcut.
pub type ShortcutHandler = Rc<dyn Fn(ExtensionContext) -> ExtensionFuture<'static, ()>>;

/// Registration data of a keyboard shortcut.
pub struct ShortcutOptions {
    /// Description shown to users.
    pub description: Option<String>,
    /// Runs when the shortcut is pressed.
    pub handler: ShortcutHandler,
}

/// Renders a custom message; `None` keeps the default rendering. It runs synchronously and
/// may emit on the bus through the borrowed emitter.
pub type MessageRenderer = Rc<
    dyn for<'a> Fn(
        CustomMessage,
        MessageRenderOptions,
        Theme,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<Option<Component>>,
>;

port! {
    /// Registrations and ordinary actions a host provides; each method forwards one request.
    ExtensionHost for ExtensionAPI via 0 {
        /// Registers a handler for an event name. Names the host does not define are
        /// forwarded unchanged, and nothing is retained when the host rejects the handler.
        fn on(event: &str, handler: ExtensionHandler) -> ();
        /// Registers a tool the model can call; nothing is retained when the host rejects it.
        fn register_tool(tool: ToolDefinition) -> ();
        /// Registers a command; nothing is retained when the host rejects it.
        fn register_command(name: &str, options: CommandOptions) -> ();
        /// Registers a keyboard shortcut; nothing is retained when the host rejects it.
        fn register_shortcut(shortcut: &str, options: ShortcutOptions) -> ();
        /// Registers a command-line flag.
        fn register_flag(name: &str, options: FlagOptions) -> ();
        /// The value of a flag: unset flags are absent, and `false` or an empty string are
        /// values like any other.
        fn get_flag(name: &str) -> Option<FlagValue>;
        /// Registers the renderer of a custom message type; nothing is retained when the host
        /// rejects it.
        fn register_message_renderer(custom_type: &str, renderer: MessageRenderer) -> ();
        /// Sends a custom message to the session.
        fn send_message(message: CustomMessageInput, options: Option<SendMessageOptions>) -> ();
        /// Sends a user message to the agent.
        fn send_user_message(content: UserContent, options: Option<SendUserMessageOptions>) -> ();
        /// Appends a custom entry to the session for state persistence; absent data is
        /// distinct from JSON `null`.
        fn append_entry(custom_type: &str, data: Option<Value>) -> ();
        /// Sets the session display name.
        fn set_session_name(name: &str) -> ();
        /// The session display name, when one is set.
        fn get_session_name() -> Option<String>;
        /// Sets or clears the label of an entry.
        fn set_label(entry_id: &str, label: Option<&str>) -> ();
        /// The names of the active tools.
        fn get_active_tools() -> Vec<String>;
        /// Every configured tool.
        fn get_all_tools() -> Vec<ToolInfo>;
        /// Sets the active tools by name.
        fn set_active_tools(names: &[String]) -> ();
        /// The available slash commands.
        fn get_commands() -> Vec<SlashCommandInfo>;
        /// The thinking level.
        fn get_thinking_level() -> ThinkingLevel;
        /// Sets the thinking level.
        fn set_thinking_level(level: ThinkingLevel) -> ();
    }
    extra {
        /// Selects a model; resolves to `false` when no credentials are available.
        fn set_model(&self, model: Model) -> ExtensionFuture<'_, bool>;
        /// The event bus shared by extensions.
        fn events(&self) -> EventBus;
    }
}

/// The handle an extension factory registers through.
#[derive(Clone)]
pub struct ExtensionAPI(Rc<dyn ExtensionHost>);

impl ExtensionAPI {
    /// Wraps a host adapter.
    #[must_use]
    pub fn new(host: Rc<dyn ExtensionHost>) -> Self {
        Self(host)
    }

    /// Selects a model; resolves to `false` when no credentials are available.
    #[must_use]
    pub fn set_model(&self, model: Model) -> ExtensionFuture<'_, bool> {
        self.0.set_model(model)
    }

    /// The event bus shared by extensions.
    #[must_use]
    pub fn events(&self) -> EventBus {
        self.0.events()
    }
}
