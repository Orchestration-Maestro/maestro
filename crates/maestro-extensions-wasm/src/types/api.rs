//! The extension API facade and the port both adapters implement.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use serde_json::Value;

use super::context::ExtensionCommandContext;
use super::events::ExtensionHandler;
use super::extension_result::{ExtensionFuture, ExtensionResult};

/// Handler of a registered command: receives the argument text and a command context.
pub type CommandHandler =
    Rc<dyn Fn(String, ExtensionCommandContext) -> ExtensionFuture<'static, ()>>;

/// Registration data of a command.
pub struct CommandOptions {
    /// Description shown to users.
    pub description: Option<String>,
    /// Runs when the command is invoked.
    pub handler: CommandHandler,
}

port! {
    /// Registrations and ordinary actions a host provides; each method forwards one request.
    ExtensionHost for ExtensionAPI via 0 {
        /// Registers a handler for an event name. Names the host does not define are
        /// forwarded unchanged, and nothing is retained when the host rejects the handler.
        fn on(event: &str, handler: ExtensionHandler) -> ();
        /// Registers a command; nothing is retained when the host rejects it.
        fn register_command(name: &str, options: CommandOptions) -> ();
        /// Appends a custom entry to the session for state persistence; absent data is
        /// distinct from JSON `null`.
        fn append_entry(custom_type: &str, data: Option<Value>) -> ();
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
