//! The extension API facade and the port both adapters implement.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use serde_json::Value;

use super::context::{ExtensionCommandContext, ExtensionContext};
use super::events::ExtensionHandler;
use super::extension_result::{ExtensionFuture, ExtensionResult};
use super::tools::ToolDefinition;
pub use crate::bindings::maestro::extension::host::{FlagOptions, FlagType, FlagValue};
use crate::{
    AutocompleteItem, CustomMessageInput, Model, SendMessageOptions, SendUserMessageOptions,
    SlashCommandInfo, ThinkingLevel, ToolInfo, UserContent,
};

/// Handler of a registered command: receives the argument text and a command context.
pub type CommandHandler =
    Rc<dyn Fn(String, ExtensionCommandContext) -> ExtensionFuture<'static, ()>>;

/// Supplies optional completion candidates for literal argument text.
pub type ArgumentCompletions =
    Rc<dyn Fn(String) -> ExtensionFuture<'static, Option<Vec<AutocompleteItem>>>>;
/// Runs a shortcut with an ordinary context.
pub type ShortcutHandler = Rc<dyn Fn(ExtensionContext) -> ExtensionFuture<'static, ()>>;
/// Shortcut registration options.
pub struct ShortcutOptions {
    /// Optional description.
    pub description: Option<String>,
    /// The shortcut callback.
    pub handler: ShortcutHandler,
}
/// Registration data of a command.
pub struct CommandOptions {
    /// Description shown to users.
    pub description: Option<String>,
    /// Runs when the command is invoked.
    pub handler: CommandHandler,
    /// Optional argument-completion callback.
    pub get_argument_completions: Option<ArgumentCompletions>,
}

port! {
    /// Registrations and ordinary actions a host provides; each method forwards one request.
    ExtensionHost for ExtensionAPI via 0 {
        /// Registers a handler for an event name. Names the host does not define are
        /// forwarded unchanged, and nothing is retained when the host rejects the handler.
        fn on(event: &str, handler: ExtensionHandler) -> ();
        /// Registers a command; rejection adds neither callback to the table.
        fn register_command(name: &str, options: CommandOptions) -> ();
        /// Registers a shortcut without parsing its key.
        fn register_shortcut(shortcut: &str, options: ShortcutOptions) -> ();
        /// Registers a flag without resolving its value.
        fn register_flag(name: &str, options: FlagOptions) -> ();
        /// Reads the host-resolved value of a flag.
        fn get_flag(name: &str) -> Option<FlagValue>;
        /// Registers one tool; rejected registration retains no callbacks.
        fn register_tool(tool: ToolDefinition) -> ();
        /// Registers a custom-message renderer.
        fn register_message_renderer(custom_type: &str, renderer: crate::MessageRenderer) -> ();
        /// Forwards a custom entry to the host's session.
        fn append_entry(custom_type: &str, data: Option<Value>) -> ();
        /// Sends the supplied custom message and optional delivery choices.
        fn send_message(message: CustomMessageInput, options: Option<SendMessageOptions>) -> ();
        /// Sends the supplied user content and optional delivery choice.
        fn send_user_message(content: UserContent, options: Option<SendUserMessageOptions>) -> ();
        /// Sets the literal session name through the host.
        fn set_session_name(name: &str) -> ();
        /// Reads the host's current optional session name.
        fn get_session_name() -> Option<String>;
        /// Sets or clears a label through the host.
        fn set_label(entry_id: &str, label: Option<&str>) -> ();
        /// Reads the host's ordered active tool names.
        fn get_active_tools() -> Vec<String>;
        /// Reads the host's ordered tool inventory.
        fn get_all_tools() -> Vec<ToolInfo>;
        /// Selects the supplied tool names through the host.
        fn set_active_tools(tool_names: &[String]) -> ();
        /// Reads the host's ordered command inventory.
        fn get_commands() -> Vec<SlashCommandInfo>;
        /// Reads the host's current thinking level.
        fn get_thinking_level() -> ThinkingLevel;
        /// Sets the requested thinking level through the host.
        fn set_thinking_level(level: ThinkingLevel) -> ();

    }
    asynchronous {
        /// Selects the supplied model through the host.
        fn set_model<'a>(model: Model) -> bool;
        /// Executes a literal program and argument vector through the host.
        fn exec<'a>(command: &'a str, args: &'a [String], options: Option<crate::ExecOptions>) -> crate::ExecResult;
    }
    infallible {
        /// Returns independently retained native operations.
        fn native_io() -> Rc<dyn crate::io::NativeIo>;
        /// Returns the supplied host bus.
        fn events() -> crate::EventBus;
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
}

impl ExtensionAPI {
    /// Executes a literal program and argument vector through the host.
    ///
    /// # Errors
    /// Returns encoding, host or result-decoding errors unchanged.
    #[must_use]
    pub fn exec<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: Option<crate::ExecOptions>,
    ) -> ExtensionFuture<'a, crate::ExecResult> {
        self.0.exec(command, args, options)
    }

    /// Selects the supplied model through the host and awaits its boolean reply.
    #[must_use]
    pub fn set_model(&self, model: Model) -> ExtensionFuture<'_, bool> {
        self.0.set_model(model)
    }
}
