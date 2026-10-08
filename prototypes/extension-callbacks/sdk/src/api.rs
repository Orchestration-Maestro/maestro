//! The extension API facade and the port both adapters implement.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use serde_json::Value;

use crate::bindings::maestro::extension::types::{CustomMessage, InputEvent, InputResult};
use crate::component::{Component, MessageRenderer};
use crate::context::{CommandHandler, ExtensionCommandContext, ExtensionContext};
use crate::tool::ToolDefinition;

/// Result of an operation that fails with the message the host supplied.
pub type ExtensionResult<T> = Result<T, String>;

/// Future of a fallible operation, local to the extension instance.
pub type ExtensionFuture<'a, T> = Pin<Box<dyn Future<Output = ExtensionResult<T>> + 'a>>;

/// Handler of an input; it may edit the event and returns an optional decision.
pub type InputHandler = Rc<
    dyn for<'a> Fn(
        &'a mut InputEvent,
        ExtensionContext,
    ) -> ExtensionFuture<'a, Option<InputResult>>,
>;

/// One typed event handler.
pub enum ExtensionHandler {
    /// Handles inputs.
    Input(InputHandler),
}

impl ExtensionHandler {
    /// Builds an input handler from a closure that borrows the event.
    pub fn input<F>(handler: F) -> Self
    where
        F: for<'a> Fn(
                &'a mut InputEvent,
                ExtensionContext,
            ) -> ExtensionFuture<'a, Option<InputResult>>
            + 'static,
    {
        Self::Input(Rc::new(handler))
    }
}

/// An extension: a factory the host runs once, awaiting its completion.
pub trait Extension {
    /// Registers callbacks through `api`; registrations become observable once this completes.
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()>;
}

/// Registrations and ordinary actions a host provides.
pub trait ExtensionHost {
    /// Forwards an input handler.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the request.
    fn on_input(&self, handler: InputHandler) -> ExtensionResult<()>;
    /// Forwards a tool with its callbacks.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the request.
    fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()>;
    /// Forwards a command handler.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the request.
    fn register_command(&self, name: &str, handler: CommandHandler) -> ExtensionResult<()>;
    /// Forwards a custom message renderer.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the request.
    fn register_message_renderer(
        &self,
        custom_type: &str,
        renderer: MessageRenderer,
    ) -> ExtensionResult<()>;
    /// Appends a custom session entry.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the request.
    fn append_entry(&self, custom_type: &str, data: Option<&Value>) -> ExtensionResult<()>;
}

/// The handle an extension factory registers through.
#[derive(Clone)]
pub struct ExtensionAPI {
    /// The adapter registrations are forwarded to.
    host: Rc<dyn ExtensionHost>,
}

impl ExtensionAPI {
    /// Wraps a host adapter.
    #[must_use]
    pub fn new(host: Rc<dyn ExtensionHost>) -> Self {
        Self { host }
    }

    /// Registers a typed event handler.
    ///
    /// # Errors
    /// Returns the host's message when registration fails.
    pub fn on(&self, handler: ExtensionHandler) -> ExtensionResult<()> {
        match handler {
            ExtensionHandler::Input(handler) => self.host.on_input(handler),
        }
    }

    /// Registers a tool.
    ///
    /// # Errors
    /// Returns the host's message when registration fails.
    pub fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()> {
        self.host.register_tool(tool)
    }

    /// Registers a command whose handler receives a command context.
    ///
    /// # Errors
    /// Returns the host's message when registration fails.
    pub fn register_command<F>(&self, name: &str, handler: F) -> ExtensionResult<()>
    where
        F: Fn(String, ExtensionCommandContext) -> ExtensionFuture<'static, ()> + 'static,
    {
        self.host.register_command(name, Rc::new(handler))
    }

    /// Registers a renderer for a custom message type.
    ///
    /// # Errors
    /// Returns the host's message when registration fails.
    pub fn register_message_renderer<F>(
        &self,
        custom_type: &str,
        renderer: F,
    ) -> ExtensionResult<()>
    where
        F: Fn(CustomMessage, bool) -> ExtensionResult<Option<Component>> + 'static,
    {
        self.host
            .register_message_renderer(custom_type, Rc::new(renderer))
    }

    /// Appends a custom entry to the session.
    ///
    /// # Errors
    /// Returns the host's message when the append fails.
    pub fn append_entry(&self, custom_type: &str, data: Option<&Value>) -> ExtensionResult<()> {
        self.host.append_entry(custom_type, data)
    }
}
