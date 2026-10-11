//! The host side of the extension world as the adapter logic sees it.

use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::types::{ExtensionFuture, ExtensionResult};
use crate::{FlagOptions, FlagValue};

/// The host's imports and resources at the wire level: the generated bindings inside a
/// component, an in-process host where the same logic runs natively.
///
/// Owner handles are released by dropping them. Each method stands for one host call.
pub trait Imports: Clone + 'static {
    /// Forwards fork through the owned host capability.
    fn fork<'a>(
        &'a self,
        context: &'a Self::CommandContext,
        entry_id: &'a str,
        data: Option<String>,
        with_session: Option<&'a Self::Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Forwards navigate tree through the owned host capability.
    fn navigate_tree<'a>(
        &'a self,
        context: &'a Self::CommandContext,
        target_id: &'a str,
        options: Option<String>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Forwards switch session through the owned host capability.
    fn switch_session<'a>(
        &'a self,
        context: &'a Self::CommandContext,
        path: &'a str,
        with_session: Option<&'a Self::Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Forwards reload through the owned host capability.
    fn reload<'a>(&'a self, context: &'a Self::CommandContext) -> ExtensionFuture<'a, ()>;
    /// Forwards replaced send message through the owned host capability.
    fn replaced_send_message<'a>(
        &'a self,
        context: &'a Self::ReplacedContext,
        message: String,
        options: Option<String>,
    ) -> ExtensionFuture<'a, ()>;
    /// Forwards replaced send user message through the owned host capability.
    fn replaced_send_user_message<'a>(
        &'a self,
        context: &'a Self::ReplacedContext,
        content: String,
        options: Option<String>,
    ) -> ExtensionFuture<'a, ()>;

    /// The Writer value.
    type Writer: 'static;
    /// Forwards writer reader through the owned host capability.
    fn writer_reader(&self, resource: &Self::Writer) -> Self::Reader;
    /// Forwards writer set session file through the owned host capability.
    fn writer_set_session_file(&self, resource: &Self::Writer, path: &str) -> ExtensionResult<()>;
    /// Forwards writer new session through the owned host capability.
    fn writer_new_session(
        &self,
        resource: &Self::Writer,
        options: Option<&str>,
    ) -> ExtensionResult<Option<String>>;
    /// Forwards writer is persisted through the owned host capability.
    fn writer_is_persisted(&self, resource: &Self::Writer) -> ExtensionResult<bool>;
    /// Forwards writer get children through the owned host capability.
    fn writer_get_children(
        &self,
        resource: &Self::Writer,
        parent_id: &str,
    ) -> ExtensionResult<String>;
    /// Forwards writer build session context through the owned host capability.
    fn writer_build_session_context(&self, resource: &Self::Writer) -> ExtensionResult<String>;
    /// Forwards writer append message through the owned host capability.
    fn writer_append_message(
        &self,
        resource: &Self::Writer,
        message: &str,
    ) -> ExtensionResult<String>;
    /// Forwards writer append thinking level change through the owned host capability.
    fn writer_append_thinking_level_change(
        &self,
        resource: &Self::Writer,
        level: &str,
    ) -> ExtensionResult<String>;
    /// Forwards writer append model change through the owned host capability.
    fn writer_append_model_change(
        &self,
        resource: &Self::Writer,
        provider: &str,
        model_id: &str,
    ) -> ExtensionResult<String>;
    /// Forwards writer append compaction through the owned host capability.
    fn writer_append_compaction(
        &self,
        resource: &Self::Writer,
        compaction: &str,
        from_hook: Option<bool>,
    ) -> ExtensionResult<String>;
    /// Forwards writer append custom entry through the owned host capability.
    fn writer_append_custom_entry(
        &self,
        resource: &Self::Writer,
        custom_type: &str,
        data: Option<&str>,
    ) -> ExtensionResult<String>;
    /// Forwards writer append session info through the owned host capability.
    fn writer_append_session_info(
        &self,
        resource: &Self::Writer,
        name: &str,
    ) -> ExtensionResult<String>;
    /// Forwards writer append custom message entry through the owned host capability.
    fn writer_append_custom_message_entry(
        &self,
        resource: &Self::Writer,
        message: &str,
    ) -> ExtensionResult<String>;
    /// Forwards writer append label change through the owned host capability.
    fn writer_append_label_change(
        &self,
        resource: &Self::Writer,
        target_id: &str,
        label: Option<&str>,
    ) -> ExtensionResult<String>;
    /// Forwards writer branch through the owned host capability.
    fn writer_branch(&self, resource: &Self::Writer, from_id: &str) -> ExtensionResult<()>;
    /// Forwards writer reset leaf through the owned host capability.
    fn writer_reset_leaf(&self, resource: &Self::Writer) -> ExtensionResult<()>;
    /// Forwards writer branch with summary through the owned host capability.
    fn writer_branch_with_summary(
        &self,
        resource: &Self::Writer,
        from_id: Option<&str>,
        summary: &str,
        metadata: (Option<&str>, Option<bool>),
    ) -> ExtensionResult<String>;
    /// Forwards writer create branched session through the owned host capability.
    fn writer_create_branched_session(
        &self,
        resource: &Self::Writer,
        leaf_id: &str,
    ) -> ExtensionResult<Option<String>>;

    /// Forwards the ordinary action.
    fn send_message(&self, message: &str, options: Option<&str>) -> ExtensionResult<()>;
    /// Forwards the ordinary action.
    fn send_user_message(&self, content: &str, options: Option<&str>) -> ExtensionResult<()>;
    /// Forwards the ordinary action.
    fn set_session_name(&self, name: &str) -> ExtensionResult<()>;
    /// Forwards the ordinary action.
    fn get_session_name(&self) -> ExtensionResult<Option<String>>;
    /// Forwards the ordinary action.
    fn set_label(&self, entry_id: &str, label: Option<&str>) -> ExtensionResult<()>;
    /// Forwards the ordinary action.
    fn get_active_tools(&self) -> ExtensionResult<Vec<String>>;
    /// Forwards the ordinary action.
    fn get_all_tools(&self) -> ExtensionResult<String>;
    /// Forwards the ordinary action.
    fn set_active_tools(&self, tool_names: &[String]) -> ExtensionResult<()>;
    /// Forwards the ordinary action.
    fn get_commands(&self) -> ExtensionResult<String>;
    /// Forwards the ordinary action.
    fn get_thinking_level(&self) -> ExtensionResult<String>;
    /// Forwards the ordinary action.
    fn set_thinking_level(&self, level: &str) -> ExtensionResult<()>;
    /// Selects a supplied model.
    fn set_model(&self, model: String) -> ExtensionFuture<'_, bool>;
    /// Starts a compaction on the supplied context.
    fn compact(
        &self,
        context: &Self::Context,
        instructions: Option<&str>,
        complete: Option<&Self::Callback>,
        error: Option<&Self::Callback>,
    ) -> ExtensionResult<()>;
    /// Starts a compaction on the supplied context.
    fn command_compact(
        &self,
        context: &Self::CommandContext,
        instructions: Option<&str>,
        complete: Option<&Self::Callback>,
        error: Option<&Self::Callback>,
    ) -> ExtensionResult<()>;

    /// Explicit remover resource.
    type Subscription: 'static;
    /// Callback-scoped emission resource.
    type Emitter: 'static;
    /// Registers a bus listener or returns the host rejection.
    fn event_on(
        &self,
        channel: &str,
        handler: &Self::Callback,
    ) -> ExtensionResult<Self::Subscription>;
    /// Explicitly removes a host subscription.
    fn unsubscribe(&self, subscription: &Self::Subscription) -> ExtensionResult<()>;
    /// Emits ordinary data asynchronously.
    fn event_emit(&self, channel: String, data: String) -> ExtensionFuture<'_, ()>;
    /// Emits from a borrowed callback capability.
    fn callback_emit(
        &self,
        emitter: &Self::Emitter,
        channel: &str,
        data: &str,
    ) -> ExtensionResult<()>;
    /// Announces a tail without invoking it.
    fn start_event_tail(
        &self,
        emitter: &Self::Emitter,
        tail: &Self::Callback,
    ) -> ExtensionResult<()>;

    /// Owner handle of a host-owned callback identity.
    type Callback: 'static;
    /// Independently retained theme owner.
    type Theme: 'static;
    /// Registers a renderer against its host callback identity.
    fn register_message_renderer(
        &self,
        custom_type: &str,
        handler: &Self::Callback,
    ) -> ExtensionResult<()>;
    /// Host-owned progress resource.
    type Update: 'static;
    /// Owned receiver of streamed command bytes.
    type BashOutput: 'static;
    /// Delivers one uninterpreted output chunk.
    fn bash_output(&self, output: &Self::BashOutput, bytes: &[u8]) -> ExtensionResult<()>;
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
        completions: Option<&Self::Callback>,
        handler: &Self::Callback,
    ) -> ExtensionResult<()>;
    /// Registers a shortcut with its literal key.
    fn register_shortcut(
        &self,
        shortcut: &str,
        description: Option<&str>,
        handler: &Self::Callback,
    ) -> ExtensionResult<()>;
    /// Forwards flag options unchanged.
    fn register_flag(&self, name: &str, options: &FlagOptions) -> ExtensionResult<()>;
    /// Returns the host-resolved value.
    fn get_flag(&self, name: &str) -> ExtensionResult<Option<FlagValue>>;

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
        setup: Option<&'a Self::Callback>,
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

    /// Independently owned host capability.
    type Ui: 'static;
    /// Independently owned host capability.
    type Registry: 'static;
    /// Forwards the corresponding host capability call.
    fn registry_get_all(&self, resource: &Self::Registry) -> ExtensionResult<String>;
    /// Forwards the corresponding host capability call.
    fn registry_get_available(&self, resource: &Self::Registry) -> ExtensionResult<String>;
    /// Forwards the corresponding host capability call.
    fn registry_find(
        &self,
        resource: &Self::Registry,
        provider: &str,
        model_id: &str,
    ) -> ExtensionResult<Option<String>>;
    /// Forwards the corresponding host capability call.
    fn registry_get_api_key_and_headers<'a>(
        &'a self,
        resource: &'a Self::Registry,
        model: String,
    ) -> ExtensionFuture<'a, crate::ResolvedRequestAuth>;
    /// Forwards the corresponding host capability call.
    fn ui(&self, context: &Self::Context) -> ExtensionResult<Self::Ui>;
    /// Forwards the corresponding host capability call.
    fn has_ui(&self, context: &Self::Context) -> ExtensionResult<bool>;
    /// Forwards the corresponding host capability call.
    fn model_registry(&self, context: &Self::Context) -> ExtensionResult<Self::Registry>;
    /// Forwards the corresponding host capability call.
    fn model(&self, context: &Self::Context) -> ExtensionResult<Option<String>>;
    /// Forwards the corresponding host capability call.
    fn is_idle(&self, context: &Self::Context) -> ExtensionResult<bool>;
    /// Forwards the corresponding host capability call.
    fn signal(&self, context: &Self::Context) -> ExtensionResult<Option<Self::Signal>>;
    /// Forwards the corresponding host capability call.
    fn abort(&self, context: &Self::Context) -> ExtensionResult<()>;
    /// Forwards the corresponding host capability call.
    fn has_pending_messages(&self, context: &Self::Context) -> ExtensionResult<bool>;
    /// Forwards the corresponding host capability call.
    fn shutdown(&self, context: &Self::Context) -> ExtensionResult<()>;
    /// Forwards the corresponding host capability call.
    fn get_context_usage(
        &self,
        context: &Self::Context,
    ) -> ExtensionResult<Option<crate::ContextUsage>>;
    /// Forwards the corresponding host capability call.
    fn get_system_prompt(&self, context: &Self::Context) -> ExtensionResult<String>;
    /// Forwards the corresponding host capability call.
    fn command_ui(&self, context: &Self::CommandContext) -> ExtensionResult<Self::Ui>;
    /// Forwards the corresponding host capability call.
    fn command_has_ui(&self, context: &Self::CommandContext) -> ExtensionResult<bool>;
    /// Forwards the corresponding host capability call.
    fn command_model_registry(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<Self::Registry>;
    /// Forwards the corresponding host capability call.
    fn command_model(&self, context: &Self::CommandContext) -> ExtensionResult<Option<String>>;
    /// Forwards the corresponding host capability call.
    fn command_is_idle(&self, context: &Self::CommandContext) -> ExtensionResult<bool>;
    /// Forwards the corresponding host capability call.
    fn command_signal(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<Option<Self::Signal>>;
    /// Forwards the corresponding host capability call.
    fn command_abort(&self, context: &Self::CommandContext) -> ExtensionResult<()>;
    /// Forwards the corresponding host capability call.
    fn command_has_pending_messages(&self, context: &Self::CommandContext)
    -> ExtensionResult<bool>;
    /// Forwards the corresponding host capability call.
    fn command_shutdown(&self, context: &Self::CommandContext) -> ExtensionResult<()>;
    /// Forwards the corresponding host capability call.
    fn command_get_context_usage(
        &self,
        context: &Self::CommandContext,
    ) -> ExtensionResult<Option<crate::ContextUsage>>;
    /// Forwards the corresponding host capability call.
    fn command_get_system_prompt(&self, context: &Self::CommandContext) -> ExtensionResult<String>;
}
