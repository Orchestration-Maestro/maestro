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
    /// Independently owned session reader.
    type Reader: 'static;
    /// Independently owned tree node.
    type Node: 'static;
    /// Acquires the current reader or returns the host rejection.
    fn session_manager(&self, context: &Self::Context) -> ExtensionResult<Self::Reader>;
    /// Acquires the current reader or returns the host rejection.
    fn command_session_manager(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<Self::Reader>;
    /// The current host working directory.
    fn reader_get_cwd(&self, resource: &Self::Reader) -> ExtensionResult<String>;
    /// The host session directory.
    fn reader_get_session_dir(&self, resource: &Self::Reader) -> ExtensionResult<String>;
    /// The host session identity.
    fn reader_get_session_id(&self, resource: &Self::Reader) -> ExtensionResult<String>;
    /// The host session file, when supplied.
    fn reader_get_session_file(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>>;
    /// The selected leaf identity.
    fn reader_get_leaf_id(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>>;
    /// The selected leaf entry.
    fn reader_get_leaf_entry(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>>;
    /// The entry for the literal identity.
    fn reader_get_entry(
        &self,
        resource: &Self::Reader,
        id: &str,
    ) -> ExtensionResult<Option<String>>;
    /// The label for the literal identity.
    fn reader_get_label(
        &self,
        resource: &Self::Reader,
        id: &str,
    ) -> ExtensionResult<Option<String>>;
    /// The supplied branch, in host order.
    fn reader_get_branch(
        &self,
        resource: &Self::Reader,
        from_id: Option<&str>,
    ) -> ExtensionResult<String>;
    /// The selected host header.
    fn reader_get_header(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>>;
    /// The supplied entries, in host order.
    fn reader_get_entries(&self, resource: &Self::Reader) -> ExtensionResult<String>;
    /// The supplied roots as owned node handles.
    fn reader_get_tree(&self, resource: &Self::Reader) -> ExtensionResult<Vec<Self::Node>>;
    /// The host-resolved session name.
    fn reader_get_session_name(&self, resource: &Self::Reader) -> ExtensionResult<Option<String>>;
    /// The entry retained by this node.
    fn node_entry(&self, resource: &Self::Node) -> ExtensionResult<String>;
    /// The supplied children as owned node handles.
    fn node_children(&self, resource: &Self::Node) -> ExtensionResult<Vec<Self::Node>>;
    /// The label retained by this node.
    fn node_label(&self, resource: &Self::Node) -> ExtensionResult<Option<String>>;
    /// The label timestamp retained by this node.
    fn node_label_timestamp(&self, resource: &Self::Node) -> ExtensionResult<Option<String>>;
}
