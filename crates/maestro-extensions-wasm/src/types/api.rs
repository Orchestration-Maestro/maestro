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
use crate::AutocompleteItem;
pub use crate::bindings::maestro::extension::host::{FlagOptions, FlagType, FlagValue};

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
        /// Forwards a custom entry to the host's session.
        fn append_entry(custom_type: &str, data: Option<Value>) -> ();
    }
    infallible {
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
