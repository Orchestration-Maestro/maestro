//! Contexts handlers receive, and the cancellation signals they can retain.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// A synchronous completion notification with callback-scoped emission.
///
/// The emitter cannot be retained beyond this invocation:
/// ```compile_fail,E0521
/// use std::{cell::RefCell, rc::Rc};
/// use maestro_extensions_wasm::{CallbackEmitter, CompactionComplete};
/// let retained: Rc<RefCell<Option<CallbackEmitter<'static>>>> = Rc::default();
/// let callback: CompactionComplete = Rc::new(move |_, emitter| {
///     retained.replace(Some(emitter));
///     Ok(())
/// });
/// ```
pub type CompactionComplete =
    Rc<dyn for<'a> Fn(crate::CompactionResult, crate::CallbackEmitter<'a>) -> ExtensionResult<()>>;
/// A synchronous error notification with callback-scoped emission.
pub type CompactionError =
    Rc<dyn for<'a> Fn(String, crate::CallbackEmitter<'a>) -> ExtensionResult<()>>;
/// A fire-and-return compaction request.
#[derive(Default)]
pub struct CompactOptions {
    /// Optional literal instructions forwarded to the host.
    pub custom_instructions: Option<String>,
    /// Optional completion callback supplied to the host.
    pub on_complete: Option<CompactionComplete>,
    /// Optional error callback supplied to the host.
    pub on_error: Option<CompactionError>,
}
pub use crate::bindings::maestro::extension::host::{
    ContextUsage, ExtensionWidgetOptions, NotificationType, WidgetPlacement,
    WorkingIndicatorOptions,
};
use crate::{Model, ModelRegistry, ReadonlySessionManager};
/// UI capability; clones share its host owner.
#[derive(Clone)]
pub struct ExtensionUIContext {
    /// Independently retained host capability.
    port: Rc<dyn ExtensionUIContextPort>,
}
impl ExtensionUIContext {
    /// Wraps the supplied UI capability.
    #[must_use]
    pub fn new(port: Rc<dyn ExtensionUIContextPort>) -> Self {
        Self { port }
    }
}

port! {
    /// Requests to the independently retained host UI; each method forwards one request and
    /// resolves to the host's message when the host rejects it.
    ExtensionUIContextPort for ExtensionUIContext via port {
        /// Shows a notification with an optional severity.
        fn notify(message: &str, kind: Option<NotificationType>) -> ();
        /// Sets the text of one status key, or clears that key.
        fn set_status(key: &str, text: Option<&str>) -> ();
        /// Sets the working message, or restores the host default.
        fn set_working_message(message: Option<&str>) -> ();
        /// Shows or hides the working loader row.
        fn set_working_visible(visible: bool) -> ();
        /// Configures the working indicator, or restores the host default.
        fn set_working_indicator(options: Option<WorkingIndicatorOptions>) -> ();
        /// Sets the hidden-thinking label, or restores the host default.
        fn set_hidden_thinking_label(label: Option<&str>) -> ();
        /// Sets the fixed lines of one widget, or removes it.
        fn set_widget(
            key: &str,
            content: Option<&[String]>,
            options: Option<ExtensionWidgetOptions>,
        ) -> ();
        /// Sets the terminal title.
        fn set_title(title: &str) -> ();
    }
}
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use super::extension_result::{ExtensionFuture, ExtensionResult};
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};

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

port! {
    /// Live capabilities of the host available to every callback.
    ContextPort for ExtensionContext via 0 {
        /// The working directory of the current session.
        fn cwd() -> String;
        /// Requests compaction without waiting for notification.
        fn compact(options: Option<CompactOptions>) -> ();
        /// Acquires the currently bound UI capability.
        fn ui() -> ExtensionUIContext;
        /// Reads the host UI availability flag.
        fn has_ui() -> bool;
        /// Acquires the currently bound catalog.
        fn model_registry() -> ModelRegistry;
        /// Reads the current model through the bound host context.
        fn model() -> Option<Model>;
        /// Reads the host idle flag.
        fn is_idle() -> bool;
        /// Acquires the current operation cancellation flag.
        fn signal() -> Option<AbortSignal>;
        /// Forwards abort to the bound host context.
        fn abort() -> ();
        /// Reads the host queued-message flag.
        fn has_pending_messages() -> bool;
        /// Forwards shutdown to the bound host context.
        fn shutdown() -> ();
        /// Reads the supplied context usage estimate.
        fn get_context_usage() -> Option<ContextUsage>;
        /// Reads the literal effective system prompt.
        fn get_system_prompt() -> String;

        /// Acquires the reader currently bound to this context.
        fn session_manager() -> ReadonlySessionManager;
    }
}

/// Context passed to event handlers. It has no session-replacing operations, so a handler
/// cannot start a session or wait for the agent:
///
/// ```compile_fail,E0599
/// fn handler(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.wait_for_idle();
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn handler(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.new_session(None);
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn fork(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.fork("entry", None);
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn navigate_tree(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.navigate_tree("entry", None);
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn switch_session(ctx: maestro_extensions_wasm::ExtensionContext) {
///     let _ = ctx.switch_session("session", None);
/// }
/// ```
///
/// ```compile_fail,E0599
/// fn reload(ctx: maestro_extensions_wasm::ExtensionContext) {
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

/// Continuation run against the replacement session of an operation.
pub type WithSession = Box<dyn FnOnce(ReplacedSessionContext) -> ExtensionFuture<'static, ()>>;

/// One-shot setup receiving the writable session capability.
pub type SetupSession = Box<dyn FnOnce(crate::SessionManager) -> ExtensionFuture<'static, ()>>;
/// Plain data and callbacks of a command-created session.
pub struct NewSessionCommandOptions {
    /// One-shot setup invoked with the operation's writable session.
    pub setup: Option<SetupSession>,
    /// Plain data of the new session.
    pub data: NewSessionCommandData,
    /// The host runs it against a context bound to the replacement session.
    pub with_session: Option<WithSession>,
}

/// Session-replacing operations of a command context; each method forwards one operation and
/// resolves to the host's message when the host rejects it.
pub trait CommandContextPort: ContextPort {
    /// Forwards fork to the bound host.
    fn fork<'a>(
        &'a self,
        entry_id: &'a str,
        options: Option<crate::ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Forwards navigate tree to the bound host.
    fn navigate_tree<'a>(
        &'a self,
        target_id: &'a str,
        options: Option<crate::NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Forwards switch session to the bound host.
    fn switch_session<'a>(
        &'a self,
        path: &'a str,
        options: Option<crate::SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult>;
    /// Forwards reload to the bound host.
    fn reload(&self) -> ExtensionFuture<'_, ()>;

    /// Waits for the agent to finish streaming.
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()>;
    /// Starts a new session.
    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult>;
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
    #[must_use]
    /// Forwards fork to the bound host.
    pub fn fork<'a>(
        &'a self,
        entry_id: &'a str,
        options: Option<crate::ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.port.fork(entry_id, options)
    }
    #[must_use]
    /// Forwards navigate tree to the bound host.
    pub fn navigate_tree<'a>(
        &'a self,
        target_id: &'a str,
        options: Option<crate::NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.port.navigate_tree(target_id, options)
    }
    #[must_use]
    /// Forwards switch session to the bound host.
    pub fn switch_session<'a>(
        &'a self,
        path: &'a str,
        options: Option<crate::SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        self.port.switch_session(path, options)
    }
    #[must_use]
    /// Forwards reload to the bound host.
    pub fn reload(&self) -> ExtensionFuture<'_, ()> {
        self.port.reload()
    }

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

    /// Starts a new session; the host runs its `with_session`, if any, against the
    /// replacement session.
    #[must_use]
    pub fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        self.port.new_session(options)
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
    /// Forwards send message to the bound host.
    fn send_message(
        &self,
        message: crate::CustomMessageInput,
        options: Option<crate::SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()>;
    /// Forwards send user message to the bound host.
    fn send_user_message(
        &self,
        content: crate::UserContent,
        options: Option<crate::SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()>;

    /// The command context bound to the replacement session.
    fn command(&self) -> Rc<dyn CommandContextPort>;
}

/// Fresh command-capable context bound to the replacement session of an operation.
#[derive(Clone)]
pub struct ReplacedSessionContext {
    /// The command view bound to the replacement session.
    command: ExtensionCommandContext,
    /// Retained host replacement capability.
    port: Rc<dyn ReplacedSessionContextPort>,
}

impl ReplacedSessionContext {
    #[must_use]
    /// Forwards send message to the bound host.
    pub fn send_message(
        &self,
        message: crate::CustomMessageInput,
        options: Option<crate::SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        self.port.send_message(message, options)
    }
    #[must_use]
    /// Forwards send user message to the bound host.
    pub fn send_user_message(
        &self,
        content: crate::UserContent,
        options: Option<crate::SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        self.port.send_user_message(content, options)
    }

    /// Wraps the host's replacement context.
    #[must_use]
    pub fn new(port: Rc<dyn ReplacedSessionContextPort>) -> Self {
        Self {
            command: ExtensionCommandContext::new(port.command()),
            port,
        }
    }
}

impl Deref for ReplacedSessionContext {
    type Target = ExtensionCommandContext;

    fn deref(&self) -> &ExtensionCommandContext {
        &self.command
    }
}

#[derive(Default, serde::Serialize)]
/// Optional plain fork data forwarded to the host.
pub struct ForkData {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Position requested from the host; absence selects no guest default.
    pub position: Option<crate::ForkPosition>,
}
#[derive(Default)]
/// Fork data and its independently optional continuation.
pub struct ForkOptions {
    /// Plain fork data.
    pub data: ForkData,
    /// One-shot continuation bound by the host to this operation.
    pub with_session: Option<WithSession>,
}
#[derive(Default)]
/// Optional continuation of a session switch.
pub struct SwitchSessionOptions {
    /// One-shot continuation bound by the host to this operation.
    pub with_session: Option<WithSession>,
}
pub use maestro_entry::session_manager::NavigateTreeOptions;
