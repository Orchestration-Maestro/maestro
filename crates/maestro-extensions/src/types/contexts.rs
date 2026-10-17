//! Records and the base context handed to extension callbacks.
use crate::{EventError, runner::ContextBinding};
use maestro_models::{Cancellation, Model};
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
    pub shutdown: Rc<dyn Fn() -> Result<(), EventError>>,
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
}
