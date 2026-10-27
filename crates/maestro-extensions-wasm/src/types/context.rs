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
    ContextUsage, ExtensionWidgetOptions, NotificationType, TerminalInputResult, WidgetPlacement,
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
        /// Shows the component `factory` builds under one key; the host disposes the widget it
        /// replaces first, and removing the key goes through `set_widget`.
        fn set_widget_factory(
            key: &str,
            factory: crate::ComponentFactory,
            options: Option<ExtensionWidgetOptions>,
        ) -> ();
        /// Replaces the footer with the component `factory` builds, or restores the host's
        /// footer; the host disposes the footer it replaces first.
        fn set_footer(factory: Option<crate::FooterFactory>) -> ();
        /// Replaces the header with the component `factory` builds, or restores the host's
        /// header; the host disposes the header it replaces first.
        fn set_header(factory: Option<crate::ComponentFactory>) -> ();
        /// Sets the terminal title.
        fn set_title(title: &str) -> ();
        /// Pastes literal text into the host editor through its paste handling.
        fn paste_to_editor(text: &str) -> ();
        /// Replaces the host editor's text.
        fn set_editor_text(text: &str) -> ();
        /// Reads the host editor's current text.
        fn get_editor_text() -> String;
        /// Replaces the host editor with the editor `factory` builds, or restores the host's
        /// editor; the factory runs before the request returns.
        fn set_editor_component(factory: Option<crate::EditorFactory>) -> ();
        /// The current editor factory, or `None` while the host's own editor is in use.
        fn get_editor_component() -> Option<crate::EditorFactory>;
        /// Stacks `factory` on the current completion provider; the host keeps it for later
        /// rebuilds and runs it before the request returns.
        fn add_autocomplete_provider(factory: crate::AutocompleteProviderFactory) -> ();
        /// Adds a raw terminal input listener; dropping the subscription keeps it.
        fn on_terminal_input(handler: TerminalInputHandler) -> crate::Subscription;
        /// The UI's live theme; it observes later selections.
        fn theme() -> crate::Theme;
        /// Every available theme, in the host's order.
        fn get_all_themes() -> Vec<crate::ThemeInfo>;
        /// Loads a theme by name without selecting it.
        fn get_theme(name: &str) -> Option<crate::Theme>;
        /// Selects a theme by name or a retained host theme.
        fn set_theme(theme: crate::ThemeSelection) -> crate::ThemeSetResult;
        /// Reads whether tool output is expanded.
        fn get_tools_expanded() -> bool;
        /// Expands or collapses tool output.
        fn set_tools_expanded(expanded: bool) -> ();
    }
    owned {
        /// Shows a custom component until its host accepts closing it.
        fn custom_component(factory: crate::CustomComponentFactory, options: Option<crate::CustomUIOptions>) -> ();
        /// Shows a selector and resolves to the chosen option, or to `None` on dismissal.
        fn select(
            title: String,
            options: Vec<String>,
            opts: Option<ExtensionUIDialogOptions>
        ) -> Option<String>;
        /// Asks for confirmation and resolves to `false` on dismissal.
        fn confirm(
            title: String,
            message: String,
            opts: Option<ExtensionUIDialogOptions>
        ) -> bool;
        /// Reads one line and resolves to `None` on dismissal.
        fn input(
            title: String,
            placeholder: Option<String>,
            opts: Option<ExtensionUIDialogOptions>
        ) -> Option<String>;
        /// Reads several lines and resolves to `None` on dismissal.
        fn editor(title: String, prefill: Option<String>) -> Option<String>;
    }
}

/// A raw terminal input listener with callback-scoped emission; `None` leaves the input as it
/// is.
pub type TerminalInputHandler = Rc<
    dyn for<'a> Fn(
        &str,
        crate::CallbackEmitter<'a>,
    ) -> ExtensionResult<Option<TerminalInputResult>>,
>;

/// Options of a dialog, forwarded without choosing host defaults.
#[derive(Clone, Default)]
pub struct ExtensionUIDialogOptions {
    /// Dismisses the dialog when aborted; it stays retained after the dialog returns.
    pub signal: Option<AbortSignal>,
    /// Milliseconds before the host dismisses the dialog, passed to it unchanged.
    pub timeout: Option<f64>,
}
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use super::extension_result::{ExtensionFuture, ExtensionResult};
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};

/// A synchronous cancellation notification with callback-scoped emission.
pub type AbortListener = Rc<dyn for<'a> Fn(crate::CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// Host capability reporting whether an operation was cancelled.
pub trait SignalPort {
    /// Whether the operation was cancelled.
    fn aborted(&self) -> bool;
    /// Registers `listener` to run once when the operation is cancelled.
    ///
    /// # Errors
    /// Returns the host rejection; a rejected listener is not retained.
    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<crate::Subscription>;
}

/// Cancellation flag of an operation. Clones share the host flag and may outlive the
/// callback that received them.
#[derive(Clone)]
pub struct AbortSignal {
    /// Cancellation observations supplied by the signal's owner.
    port: Rc<dyn SignalPort>,
    /// Original host resource, retained when the host supplied this signal.
    #[cfg(any(test, target_arch = "wasm32"))]
    host: Option<Rc<dyn std::any::Any>>,
}

impl AbortSignal {
    /// Wraps the host's flag.
    #[must_use]
    pub fn new(port: Rc<dyn SignalPort>) -> Self {
        Self {
            port,
            #[cfg(any(test, target_arch = "wasm32"))]
            host: None,
        }
    }

    /// Keeps a supplied host resource available for later operations.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn supplied<P: SignalPort + 'static>(port: Rc<P>) -> Self {
        Self {
            port: port.clone(),
            host: Some(port),
        }
    }

    /// Recovers the retained host resource without creating another signal.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn host<P: 'static>(&self) -> Option<Rc<P>> {
        self.host.clone()?.downcast().ok()
    }

    /// Whether the operation was cancelled when this is called.
    #[must_use]
    pub fn aborted(&self) -> bool {
        self.port.aborted()
    }

    /// Registers `listener` to run once when the operation is cancelled.
    ///
    /// Host signals invoke the listener synchronously on their local owner; an abort
    /// from another thread wakes that owner. A listener registered after cancellation
    /// never runs; read [`AbortSignal::aborted`] first.
    /// Dropping the returned subscription keeps the listener; unsubscribe to remove it.
    ///
    /// # Errors
    /// Returns the host rejection; a rejected listener is not retained.
    pub fn subscribe(&self, listener: AbortListener) -> ExtensionResult<crate::Subscription> {
        self.port.subscribe(listener)
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
