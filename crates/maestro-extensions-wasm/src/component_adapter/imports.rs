//! The host side of the extension world as the adapter logic sees it.

use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::types::{ExtensionFuture, ExtensionResult};

/// The host's imports and resources at the wire level: the generated bindings inside a
/// component, an in-process host where the same logic runs natively.
///
/// Owner handles are released by dropping them. Each method stands for one host call.
pub trait Imports: Clone + 'static {
    /// Owner handle of a host-owned callback identity.
    type Callback: 'static;
    /// Host-owned progress resource.
    type Update: 'static;
    /// Host-owned cancellation flag.
    type Signal: 'static;
    /// Context with the live capabilities every callback receives.
    type Context: 'static;
    /// Context that also has the session-replacing operations.
    type CommandContext: 'static;
    /// Context bound to the session an operation created.
    type ReplacedContext: 'static;

    /// Asks the host for a fresh identity: its key, which the owner handle and every borrow
    /// of it share, and the owner handle.
    fn new_callback(&self) -> (u32, Self::Callback);

    /// Registers a handler for an event name.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the registration.
    fn on(&self, event: &str, handler: &Self::Callback) -> ExtensionResult<()>;

    /// Registers a command.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the registration.
    fn register_command(
        &self,
        name: &str,
        description: Option<&str>,
        handler: &Self::Callback,
    ) -> ExtensionResult<()>;

    /// Registers authored metadata with optional preparation and required execution.
    ///
    /// # Errors
    /// Returns the host's rejection unchanged.
    fn register_tool(
        &self,
        metadata: &str,
        prepare: Option<&Self::Callback>,
        execute: &Self::Callback,
    ) -> ExtensionResult<()>;

    /// Delivers a partial output to a progress resource.
    ///
    /// # Errors
    /// Returns the host's failure unchanged.
    fn tool_update(&self, update: &Self::Update, partial: &str) -> ExtensionResult<()>;

    /// Appends a custom entry to the session; `data` is JSON text.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the entry.
    fn append_entry(&self, custom_type: &str, data: Option<&str>) -> ExtensionResult<()>;

    /// Whether the operation was cancelled.
    fn aborted(&self, signal: &Self::Signal) -> bool;

    /// The working directory of the session an ordinary context is bound to.
    ///
    /// # Errors
    /// Returns the host's message when the host rejects the request.
    fn cwd(&self, context: &Self::Context) -> ExtensionResult<String>;

    /// The working directory of the session a command context is bound to.
    ///
    /// # Errors
    /// Returns the host's message when the host rejects the request.
    fn command_cwd(&self, context: &Self::CommandContext) -> ExtensionResult<String>;

    /// Waits for the agent to finish streaming.
    fn wait_for_idle<'a>(&'a self, context: &'a Self::CommandContext) -> ExtensionFuture<'a, ()>;

    /// Starts a new session; the host runs `with_session` against the replacement session
    /// while the operation is pending.
    fn new_session<'a>(
        &'a self,
        context: &'a Self::CommandContext,
        data: NewSessionCommandData,
        with_session: Option<&'a Self::Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;

    /// The command context bound to the replacement session.
    fn command(&self, context: &Self::ReplacedContext) -> Self::CommandContext;
}
