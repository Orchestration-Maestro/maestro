//! Records and the base context handed to extension callbacks.
use crate::{EventError, runner::ContextBinding};
use maestro_models::{Cancellation, Model};
use maestro_models::{Execution, Local, UserContent};
use maestro_session::{
    SessionManager,
    messages::{CustomMessageInput, SendMessageOptions, SendUserMessageOptions},
    session_manager::{ForkPosition, NavigateTreeOptions},
};
use std::{cell::RefCell, future::ready, ops::Deref};

use maestro_session::CompactionResult;
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Reads the current model.
type ModelReader = Rc<dyn Fn() -> Result<Option<Arc<RwLock<Model>>>, EventError>>;
/// Receives the finished compaction.
type CompactionComplete = Rc<dyn Fn(CompactionResult) -> Result<(), EventError>>;
/// Receives the compaction failure.
type CompactionError = Rc<dyn Fn(EventError) -> Result<(), EventError>>;

/// Current context-window usage of the active model.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsage {
    /// Estimated context tokens, or `None` when unknown.
    pub tokens: Option<f64>,
    /// Context window of the active model.
    pub context_window: f64,
    /// Usage as a percentage of the window, or `None` when tokens are unknown.
    pub percent: Option<f64>,
}

/// A compaction request that returns before compaction finishes.
#[derive(Default)]
pub struct CompactOptions {
    /// Optional instructions for the summary.
    pub custom_instructions: Option<String>,
    /// Invoked with the finished compaction.
    pub on_complete: Option<CompactionComplete>,
    /// Invoked with the compaction failure.
    pub on_error: Option<CompactionError>,
}

/// Application operations behind every base context.
pub struct ExtensionContextActions {
    /// Reads the current model.
    pub get_model: ModelReader,
    /// Whether the agent is not streaming.
    pub is_idle: Rc<dyn Fn() -> Result<bool, EventError>>,
    /// Reads the current operation's cancellation, if streaming.
    pub get_signal: Rc<dyn Fn() -> Result<Option<Cancellation>, EventError>>,
    /// Aborts the current agent operation.
    pub abort: Rc<dyn Fn() -> Result<(), EventError>>,
    /// Whether queued messages are waiting.
    pub has_pending_messages: Rc<dyn Fn() -> Result<bool, EventError>>,
    /// Requests a graceful shutdown.
    pub shutdown: ShutdownHandler,
    /// Reads the current context usage, if known.
    pub get_context_usage: Rc<dyn Fn() -> Result<Option<ContextUsage>, EventError>>,
    /// Starts compaction without waiting for it.
    pub compact: Rc<dyn Fn(Option<CompactOptions>) -> Result<(), EventError>>,
    /// Reads the current effective system prompt.
    pub get_system_prompt: Rc<dyn Fn() -> Result<String, EventError>>,
}

impl Default for ExtensionContextActions {
    /// The results a runner gives before any operations are bound.
    fn default() -> Self {
        Self {
            get_model: Rc::new(|| Ok(None)),
            is_idle: Rc::new(|| Ok(true)),
            get_signal: Rc::new(|| Ok(None)),
            abort: Rc::new(|| Ok(())),
            has_pending_messages: Rc::new(|| Ok(false)),
            shutdown: Rc::new(|| Ok(())),
            get_context_usage: Rc::new(|| Ok(None)),
            compact: Rc::new(|_| Ok(())),
            get_system_prompt: Rc::new(|| Ok(String::new())),
        }
    }
}

/// The context passed to extension callbacks.
///
/// Each operation first fails with the stale error once its runner is invalidated,
/// then reads the runner's current state. Values it returned earlier stay usable.
/// It is a single-thread handle, so a thread-safe callback cannot keep it across a
/// suspension:
///
/// ```compile_fail,E0277
/// use maestro_extensions::{ExtensionContext, types::ShortcutHandler};
/// use std::sync::Arc;
///
/// fn threaded_shortcut_future_cannot_capture_context() -> ShortcutHandler {
///     Arc::new(|ctx: ExtensionContext| Box::pin(async move {
///         let _ = ctx.cwd();
///         Ok(())
///     }))
/// }
/// ```
#[derive(Clone)]
pub struct ExtensionContext {
    /// Runner state read at call time.
    pub(crate) binding: Rc<ContextBinding>,
    /// Model reader captured when the context was created.
    pub(crate) get_model: ModelReader,
    /// Invocation-owned prompt, absent for ordinary live-bound contexts.
    pub(crate) invocation_system_prompt: Option<Rc<RefCell<String>>>,
}

/// The local future returned by command and replacement operations.
pub(crate) type ContextFuture<T> = <Local as Execution>::Future<'static, Result<T, EventError>>;
/// Prepares a new session through its writable history.
pub type SetupSession = Rc<dyn Fn(Rc<RefCell<SessionManager>>) -> ContextFuture<()>>;
/// Continues against the replacement session.
pub type WithSession = Rc<dyn Fn(ReplacedSessionContext) -> ContextFuture<()>>;
/// Sends a custom message to the creating session.
type SendCustomMessage =
    Rc<dyn Fn(CustomMessageInput, Option<SendMessageOptions>) -> ContextFuture<()>>;
/// Sends a user message to the creating session.
type SendUserMessage = Rc<dyn Fn(UserContent, Option<SendUserMessageOptions>) -> ContextFuture<()>>;

/// Whether a session-changing command operation was cancelled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionChangeResult {
    /// The operation was cancelled.
    pub cancelled: bool,
}

/// Options of a command-started session.
#[derive(Default)]
pub struct NewSessionCommandOptions {
    /// Session file the new session starts from.
    pub parent_session: Option<String>,
    /// Prepares the replacement session.
    pub setup: Option<SetupSession>,
    /// Continues against the replacement session.
    pub with_session: Option<WithSession>,
}

/// Options of a command fork.
#[derive(Default)]
pub struct ForkOptions {
    /// Where the fork starts relative to its entry.
    pub position: Option<ForkPosition>,
    /// Continues against the replacement session.
    pub with_session: Option<WithSession>,
}

/// Options of a command session switch.
#[derive(Default)]
pub struct SwitchSessionOptions {
    /// Continues against the replacement session.
    pub with_session: Option<WithSession>,
}

/// Starts a new session.
pub type NewSessionHandler =
    Rc<dyn Fn(Option<NewSessionCommandOptions>) -> ContextFuture<SessionChangeResult>>;
/// Forks the session from an entry.
pub type ForkHandler =
    Rc<dyn Fn(String, Option<ForkOptions>) -> ContextFuture<SessionChangeResult>>;
/// Moves to another point of the session tree.
pub type NavigateTreeHandler =
    Rc<dyn Fn(String, Option<NavigateTreeOptions>) -> ContextFuture<SessionChangeResult>>;
/// Switches to another session file.
pub type SwitchSessionHandler =
    Rc<dyn Fn(String, Option<SwitchSessionOptions>) -> ContextFuture<SessionChangeResult>>;
/// Reloads extensions, skills, prompts and themes.
pub type ReloadHandler = Rc<dyn Fn() -> ContextFuture<()>>;
/// Requests a graceful shutdown.
pub type ShutdownHandler = Rc<dyn Fn() -> Result<(), EventError>>;

/// The application operations a command context runs.
#[derive(Clone)]
pub struct CommandContextOperations {
    /// Waits for the agent to finish streaming.
    pub wait_for_idle: Rc<dyn Fn() -> ContextFuture<()>>,
    /// Starts a new session.
    pub new_session: NewSessionHandler,
    /// Forks the session from an entry.
    pub fork: ForkHandler,
    /// Moves to another point of the session tree.
    pub navigate_tree: NavigateTreeHandler,
    /// Switches to another session file.
    pub switch_session: SwitchSessionHandler,
    /// Reloads extensions, skills, prompts and themes.
    pub reload: ReloadHandler,
}

impl CommandContextOperations {
    /// The results a runner gives before operations are bound or after they are unbound.
    pub(crate) fn unbound() -> Self {
        let uncancelled = || -> ContextFuture<SessionChangeResult> {
            Box::pin(ready(Ok(SessionChangeResult::default())))
        };
        Self {
            wait_for_idle: Rc::new(|| Box::pin(ready(Ok(())))),
            new_session: Rc::new(move |_| uncancelled()),
            fork: Rc::new(move |_, _| uncancelled()),
            navigate_tree: Rc::new(move |_, _| uncancelled()),
            switch_session: Rc::new(move |_, _| uncancelled()),
            reload: Rc::new(|| Box::pin(ready(Ok(())))),
        }
    }
}

/// The command-context operations bound to a session.
///
/// A runner serves the operations the application runtime built, so a caller asks the
/// runtime for them instead of supplying its own.
///
/// ```compile_fail,E0451
/// use maestro_extensions::ExtensionCommandContextActions as Actions;
///
/// let _ = Actions { operations: todo!() };
/// ```
///
/// ```compile_fail,E0277
/// use maestro_extensions::ExtensionCommandContextActions as Actions;
///
/// let _ = <Actions as Default>::default();
/// ```
#[derive(Clone)]
pub struct ExtensionCommandContextActions {
    /// The operations a session command runs.
    operations: CommandContextOperations,
}

impl ExtensionCommandContextActions {
    /// Wraps the operations an application supplies for its sessions.
    #[must_use]
    pub fn new(operations: CommandContextOperations) -> Self {
        Self { operations }
    }

    /// The operations a session command runs.
    #[must_use]
    pub fn operations(&self) -> &CommandContextOperations {
        &self.operations
    }

    /// The results a runner gives before operations are bound or after they are unbound.
    pub(crate) fn unbound() -> Self {
        Self::new(CommandContextOperations::unbound())
    }
}

/// The context passed to command handlers: the base context plus session control.
///
/// ```compile_fail,E0277
/// use maestro_extensions::{ExtensionCommandContext, types::CommandHandler};
/// use std::sync::Arc;
///
/// fn threaded_command_future_cannot_capture_context() -> CommandHandler {
///     Arc::new(|_, ctx: ExtensionCommandContext| Box::pin(async move {
///         let _ = ctx.cwd();
///         Ok(())
///     }))
/// }
/// ```
#[derive(Clone)]
pub struct ExtensionCommandContext {
    /// The base view with its own captured model reader.
    pub(crate) context: ExtensionContext,
}

impl Deref for ExtensionCommandContext {
    type Target = ExtensionContext;
    fn deref(&self) -> &ExtensionContext {
        &self.context
    }
}

/// A command context bound to a replacement session, with that session's message operations.
#[derive(Clone)]
pub struct ReplacedSessionContext {
    /// The command view of the replacement runner.
    command: ExtensionCommandContext,
    /// Sends a custom message to the creating session.
    send_message: SendCustomMessage,
    /// Sends a user message to the creating session.
    send_user_message: SendUserMessage,
}

impl Deref for ReplacedSessionContext {
    type Target = ExtensionCommandContext;
    fn deref(&self) -> &ExtensionCommandContext {
        &self.command
    }
}

impl ReplacedSessionContext {
    /// Combines a replacement runner's command context with its session's message operations.
    #[must_use]
    pub fn new(
        command: ExtensionCommandContext,
        send_message: SendCustomMessage,
        send_user_message: SendUserMessage,
    ) -> Self {
        Self {
            command,
            send_message,
            send_user_message,
        }
    }
    /// Sends a custom message to the creating session, even after invalidation.
    #[must_use]
    pub fn send_message(
        &self,
        message: CustomMessageInput,
        options: Option<SendMessageOptions>,
    ) -> ContextFuture<()> {
        (self.send_message)(message, options)
    }
    /// Sends a user message to the creating session, even after invalidation.
    #[must_use]
    pub fn send_user_message(
        &self,
        content: UserContent,
        options: Option<SendUserMessageOptions>,
    ) -> ContextFuture<()> {
        (self.send_user_message)(content, options)
    }
}
