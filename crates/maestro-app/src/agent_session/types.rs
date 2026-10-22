//! Session configuration, events and operation results.
use crate::resources::resource_loader::ResourceLoader;
use maestro_agent::{Agent, AgentEvent};
use maestro_catalog::{ModelRegistry, ScopedModel};
use maestro_credentials::ConfigValueOperations;
use maestro_extensions::{ExtensionExecution, ExtensionRunner};
use maestro_models::{Local, Model, ModelThinkingLevel, Threaded};
use maestro_session::{
    BranchSummaryEntry, CompactionResult, SessionManager, messages::CustomAgentMessages,
};
use maestro_settings::SettingsManager;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// What started a compaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactionReason {
    /// Requested by the user.
    Manual,
    /// The context passed the reserve threshold.
    Threshold,
    /// The provider reported a context overflow.
    Overflow,
}

/// An agent event or a session event.
pub enum AgentSessionEvent {
    /// An agent event, as the agent delivered it.
    Agent(Arc<RwLock<AgentEvent<CustomAgentMessages>>>),
    /// The pending steering and follow-up texts changed.
    QueueUpdate {
        /// Pending steering texts.
        steering: Vec<String>,
        /// Pending follow-up texts.
        follow_up: Vec<String>,
    },
    /// A compaction started.
    CompactionStart {
        /// What started it.
        reason: CompactionReason,
    },
    /// The session name changed.
    SessionInfoChanged {
        /// The name now in effect.
        name: Option<String>,
    },
    /// The effective thinking level changed.
    ThinkingLevelChanged {
        /// The new level.
        level: ModelThinkingLevel,
    },
    /// A compaction ended.
    CompactionEnd {
        /// What started it.
        reason: CompactionReason,
        /// The stored result, absent when it was aborted or failed.
        result: Option<CompactionResult>,
        /// Whether it was aborted.
        aborted: bool,
        /// Whether the failed prompt is retried.
        will_retry: bool,
        /// The failure message.
        error_message: Option<String>,
    },
    /// A retry of a failed response is scheduled.
    AutoRetryStart {
        /// The attempt about to run.
        attempt: u32,
        /// The configured attempt limit.
        max_attempts: u32,
        /// The wait before the attempt.
        delay_ms: f64,
        /// The failure being retried.
        error_message: String,
    },
    /// Retrying ended.
    AutoRetryEnd {
        /// Whether a retry succeeded.
        success: bool,
        /// The last attempt.
        attempt: u32,
        /// The final failure.
        final_error: Option<String>,
    },
}

/// A session listener.
pub type AgentSessionEventListener = Rc<dyn Fn(&AgentSessionEvent)>;

/// The owners a session works over.
pub struct AgentSessionConfig<E: ExtensionExecution = Threaded> {
    /// The agent whose state the session reads and changes.
    pub agent: Rc<Agent<CustomAgentMessages, Local>>,
    /// The session history.
    pub session_manager: Rc<RefCell<SessionManager>>,
    /// The preferences.
    pub settings_manager: Rc<RefCell<SettingsManager>>,
    /// The session working directory.
    pub cwd: String,
    /// Models to cycle through, in order.
    pub scoped_models: Vec<ScopedModel>,
    /// The loaded skills, prompts, context files and system prompt.
    pub resource_loader: Rc<dyn ResourceLoader<E>>,
    /// The model catalog used for credentials and model discovery.
    pub model_registry: Arc<ModelRegistry>,
    /// The extensions' runner.
    pub extension_runner: Rc<ExtensionRunner<E>>,
    /// Runs the commands that configured credentials name.
    pub config_value_operations: Arc<dyn ConfigValueOperations>,
}

/// What a tool-registry refresh activates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefreshToolRegistryOptions {
    /// The tools to activate in place of the active ones.
    pub active_tool_names: Option<Vec<String>>,
    /// Whether every extension tool is activated as well.
    pub include_all_extension_tools: bool,
}

/// Which way models are cycled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CycleDirection {
    /// The next model.
    #[default]
    Forward,
    /// The previous model.
    Backward,
}

/// The model selected by cycling.
pub struct ModelCycleResult {
    /// The selected model.
    pub model: Arc<RwLock<Model>>,
    /// The thinking level after clamping to the model.
    pub thinking_level: ModelThinkingLevel,
    /// Whether the scoped models were cycled rather than every available model.
    pub is_scoped: bool,
}

/// Token totals of a session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionStatsTokens {
    /// Input tokens.
    pub input: f64,
    /// Output tokens.
    pub output: f64,
    /// Cache-read tokens.
    pub cache_read: f64,
    /// Cache-write tokens.
    pub cache_write: f64,
    /// The sum of the four.
    pub total: f64,
}

/// Session statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionStats {
    /// The session file, absent in memory.
    pub session_file: Option<String>,
    /// The session identity.
    pub session_id: String,
    /// User messages.
    pub user_messages: usize,
    /// Assistant messages.
    pub assistant_messages: usize,
    /// Tool calls requested by assistant messages.
    pub tool_calls: usize,
    /// Tool results.
    pub tool_results: usize,
    /// Every message, including application messages.
    pub total_messages: usize,
    /// Token totals.
    pub tokens: SessionStatsTokens,
    /// Total cost.
    pub cost: f64,
    /// Context usage, absent without a context window.
    pub context_usage: Option<maestro_extensions::ContextUsage>,
}

/// The outcome of moving to another entry.
#[derive(Debug, Default)]
pub struct NavigateTreeResult {
    /// Text to put back in the editor.
    pub editor_text: Option<String>,
    /// Whether navigation was cancelled.
    pub cancelled: bool,
    /// Whether summarizing was aborted.
    pub aborted: bool,
    /// The stored summary.
    pub summary_entry: Option<BranchSummaryEntry>,
}

/// A user message a session can be forked from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkMessage {
    /// The message entry.
    pub entry_id: String,
    /// The message text.
    pub text: String,
}
