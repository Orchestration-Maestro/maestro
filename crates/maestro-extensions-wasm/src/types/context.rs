//! Contexts handlers receive, and the opaque capabilities they can retain.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use super::extension_result::{ExtensionFuture, ExtensionResult};
use crate::bindings::maestro::extension::host::{SendMessageOptions, SendUserMessageOptions};
use crate::bindings::maestro::extension::session::{
    ContextUsage, ForkData, NavigateTreeOptions, NewSessionCommandData, SessionChangeResult,
};
use crate::compaction::CompactionResult;
use crate::event_bus::CallbackEmitter;
use crate::messages::CustomMessageInput;
use crate::model_registry::ModelRegistry;
use crate::models::{Model, UserContent};
use crate::session_manager::{ReadonlySessionManager, SessionManager};

/// Host capability reporting whether an operation was cancelled.
pub trait SignalPort {
    /// Whether the operation was cancelled.
    fn aborted(&self) -> bool;
}

/// Cancellation flag of an operation. Clones share the host flag and may outlive the
/// callback that received them.
#[derive(Clone)]
pub struct AbortSignal(Rc<dyn SignalPort>);

impl AbortSignal {
    /// Wraps the host's flag.
    #[must_use]
    pub fn new(port: Rc<dyn SignalPort>) -> Self {
        Self(port)
    }

    /// Whether the operation was cancelled when this is called.
    #[must_use]
    pub fn aborted(&self) -> bool {
        self.0.aborted()
    }
}

impl fmt::Debug for AbortSignal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AbortSignal")
            .field("aborted", &self.aborted())
            .finish()
    }
}

/// Declares a capability facade the host owns and an author can only retain and clone.
macro_rules! opaque {
    ($($(#[$meta:meta])* $facade:ident($port:ident);)*) => {
        $(
            /// Marker for the host resource behind the matching opaque facade.
            pub trait $port {}

            $(#[$meta])*
            #[derive(Clone)]
            pub struct $facade {
                /// The retained host capability.
                _port: Rc<dyn $port>,
            }

            impl fmt::Debug for $facade {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.debug_struct(stringify!($facade)).finish_non_exhaustive()
                }
            }

            impl $facade {
                /// Wraps the capability.
                #[must_use]
                pub fn new(port: Rc<dyn $port>) -> Self {
                    Self { _port: port }
                }
            }
        )*
    };
}

opaque! {
    /// Opaque handle to the host's user interface. Clones share the capability.
    ExtensionUIContext(ExtensionUIContextPort);
    /// Opaque handle to the host's theme. Clones share the capability.
    Theme(ThemePort);
    /// Opaque handle to the operations a user bash replacement executes commands with.
    /// Clones share the capability.
    BashOperations(BashOperationsPort);
}

/// Callback told that a compaction finished, with the emitter it may use meanwhile.
pub type CompactionComplete =
    Rc<dyn for<'a> Fn(CompactionResult, CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// Callback told that a compaction failed, with the emitter it may use meanwhile.
pub type CompactionError = Rc<dyn for<'a> Fn(String, CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// Options of a compaction request.
#[derive(Default)]
pub struct CompactOptions {
    /// Instructions for the summarization.
    pub custom_instructions: Option<String>,
    /// Told when the compaction finished.
    pub on_complete: Option<CompactionComplete>,
    /// Told when the compaction failed.
    pub on_error: Option<CompactionError>,
}

port! {
    /// Live host capabilities behind an [`ExtensionContext`]; each method forwards one
    /// context operation.
    ContextPort for ExtensionContext via 0 {
        /// The user interface.
        fn ui() -> ExtensionUIContext;
        /// Whether a user interface is available.
        fn has_ui() -> bool;
        /// Current working directory.
        fn cwd() -> String;
        /// Read access to the session.
        fn session_manager() -> ReadonlySessionManager;
        /// The model catalog.
        fn model_registry() -> ModelRegistry;
        /// The current model, when there is one.
        fn model() -> Option<Model>;
        /// Whether the agent is idle.
        fn is_idle() -> bool;
        /// The current abort signal, when the agent is streaming.
        fn signal() -> Option<AbortSignal>;
        /// Aborts the current agent operation.
        fn abort() -> ();
        /// Whether messages are queued.
        fn has_pending_messages() -> bool;
        /// Gracefully shuts the application down.
        fn shutdown() -> ();
        /// The context usage of the active model.
        fn get_context_usage() -> Option<ContextUsage>;
        /// Triggers a compaction without awaiting its completion.
        fn compact(options: Option<CompactOptions>) -> ();
        /// The effective system prompt.
        fn get_system_prompt() -> String;
    }
}

/// Context passed to event handlers, tools and shortcuts. It has no session-replacing
/// operations, so a tool or shortcut cannot start, fork or switch a session:
///
/// ```compile_fail,E0599
/// fn tool(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.wait_for_idle();
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn tool(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.new_session(None);
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn tool(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.fork("entry", None);
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn tool(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.navigate_tree("entry", None);
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn tool(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.switch_session("path", None);
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn tool(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.reload();
/// }
/// ```
#[derive(Clone)]
pub struct ExtensionContext(Rc<dyn ContextPort>);

impl ExtensionContext {
    /// Wraps the host's context.
    #[must_use]
    pub fn new(port: Rc<dyn ContextPort>) -> Self {
        Self(port)
    }
}

/// Runs against the session manager of a new session before its continuation.
pub type SetupSession = Box<dyn FnOnce(SessionManager) -> ExtensionFuture<'static, ()>>;

/// Continuation run against the replacement session of an operation.
pub type WithSession = Box<dyn FnOnce(ReplacedSessionContext) -> ExtensionFuture<'static, ()>>;

/// Plain data and continuations of a new session started from a command.
pub struct NewSessionCommandOptions {
    /// Plain data of the new session.
    pub data: NewSessionCommandData,
    /// Prepares the new session before it becomes current.
    pub setup: Option<SetupSession>,
    /// Runs once the new session is current, against its own context.
    pub with_session: Option<WithSession>,
}

/// Plain data and continuation of a fork.
pub struct ForkOptions {
    /// Plain data of the fork.
    pub data: ForkData,
    /// Runs once the fork is current, against its own context.
    pub with_session: Option<WithSession>,
}

/// Continuation of a switch to another session.
pub struct SwitchSessionOptions {
    /// Runs once the other session is current, against its own context.
    pub with_session: Option<WithSession>,
}

/// Session-replacing operations of a [`CommandContextPort`]; each method forwards one
/// operation and resolves to the host's message when the host rejects it.
pub trait CommandContextPort: ContextPort {
    /// Waits for the agent to finish streaming.
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()>;
    /// Starts a new session.
    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult>;
    /// Forks from an entry, creating a new session file.
    fn fork<'a>(
        &'a self,
        entry_id: &'a str,
        options: Option<ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Navigates to another point of the session tree.
    fn navigate_tree<'a>(
        &'a self,
        target_id: &'a str,
        options: Option<NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Switches to another session file.
    fn switch_session<'a>(
        &'a self,
        path: &'a str,
        options: Option<SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Reloads extensions, skills, prompts and themes.
    fn reload(&self) -> ExtensionFuture<'_, ()>;
}

/// Context of commands: the ordinary context plus the session-replacing operations.
#[derive(Clone)]
pub struct ExtensionCommandContext {
    /// The host's command capabilities.
    port: Rc<dyn CommandContextPort>,
    /// The ordinary view over the same capabilities.
    ordinary: ExtensionContext,
}

impl ExtensionCommandContext {
    /// Wraps the host's command context.
    #[must_use]
    pub fn new(port: Rc<dyn CommandContextPort>) -> Self {
        let ordinary = ExtensionContext::new(Rc::clone(&port) as Rc<dyn ContextPort>);
        Self { port, ordinary }
    }

    /// Waits for the agent to finish streaming.
    #[must_use]
    pub fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        self.port.wait_for_idle()
    }

    /// Starts a new session; its `setup` runs before, and its `with_session` after, the
    /// session becomes current.
    #[must_use]
    pub fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        self.port.new_session(options)
    }

    /// Forks from an entry; `with_session` runs against the fork.
    #[must_use]
    pub fn fork<'a>(
        &'a self,
        entry_id: &'a str,
        options: Option<ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.port.fork(entry_id, options)
    }

    /// Navigates to another point of the session tree.
    #[must_use]
    pub fn navigate_tree<'a>(
        &'a self,
        target_id: &'a str,
        options: Option<NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.port.navigate_tree(target_id, options)
    }

    /// Switches to another session file; `with_session` runs against it.
    #[must_use]
    pub fn switch_session<'a>(
        &'a self,
        path: &'a str,
        options: Option<SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.port.switch_session(path, options)
    }

    /// Reloads extensions, skills, prompts and themes. No continuation runs afterwards.
    #[must_use]
    pub fn reload(&self) -> ExtensionFuture<'_, ()> {
        self.port.reload()
    }
}

impl Deref for ExtensionCommandContext {
    type Target = ExtensionContext;

    fn deref(&self) -> &ExtensionContext {
        &self.ordinary
    }
}

/// Operations of a [`ReplacedSessionContext`] beyond its command context.
pub trait ReplacedSessionContextPort {
    /// The command context bound to the replacement session.
    fn command(&self) -> Rc<dyn CommandContextPort>;
    /// Sends a custom message to the replacement session.
    fn send_message(
        &self,
        message: CustomMessageInput,
        options: Option<SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()>;
    /// Sends a user message to the replacement session.
    fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()>;
}

/// Fresh command-capable context bound to the replacement session of an operation.
#[derive(Clone)]
pub struct ReplacedSessionContext {
    /// The host's replacement capabilities.
    port: Rc<dyn ReplacedSessionContextPort>,
    /// The command view bound to the replacement session.
    command: ExtensionCommandContext,
}

impl ReplacedSessionContext {
    /// Wraps the host's replacement context.
    #[must_use]
    pub fn new(port: Rc<dyn ReplacedSessionContextPort>) -> Self {
        let command = ExtensionCommandContext::new(port.command());
        Self { port, command }
    }

    /// Sends a custom message to the replacement session.
    #[must_use]
    pub fn send_message(
        &self,
        message: CustomMessageInput,
        options: Option<SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        self.port.send_message(message, options)
    }

    /// Sends a user message to the replacement session.
    #[must_use]
    pub fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        self.port.send_user_message(content, options)
    }
}

impl Deref for ReplacedSessionContext {
    type Target = ExtensionCommandContext;

    fn deref(&self) -> &ExtensionCommandContext {
        &self.command
    }
}
