//! Session configuration, events and operation results.
use crate::resources::resource_loader::ResourceLoader;
use indexmap::IndexMap;
use maestro_agent::SharedAgentTool;
use maestro_agent::{Agent, AgentEvent};
use maestro_catalog::{ModelRegistry, ScopedModel};
use maestro_extensions::{
    ExtensionCommandContextActions, ExtensionErrorListener, ExtensionExecution, ExtensionRunner,
    ExtensionUIContextBinding, InputSource, SessionStartEvent, types::ShutdownHandler,
};
use maestro_models::{ImageContent, Local, Model, ModelThinkingLevel};
use maestro_session::{
    BranchSummaryEntry, CompactionResult, SessionManager,
    messages::{CustomAgentMessages, UserMessageDelivery},
};
use maestro_settings::SettingsManager;
use maestro_theme::LiveTheme;
pub use maestro_tools::ToolsManager;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// What started a compaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompactionReason {
    /// Requested by the user.
    Manual,
    /// The context passed the reserve threshold.
    Threshold,
    /// The provider reported a context overflow.
    Overflow,
}

/// An agent event or a session event, written as one record with its snake-case `type`.
#[derive(Clone, serde::Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AgentSessionEvent {
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
        #[serde(skip_serializing_if = "Option::is_none")]
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
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<CompactionResult>,
        /// Whether it was aborted.
        aborted: bool,
        /// Whether the failed prompt is retried.
        will_retry: bool,
        /// The failure message.
        #[serde(skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
    },
    /// A retry of a failed response is scheduled.
    AutoRetryStart {
        /// The attempt about to run.
        attempt: u32,
        /// The configured attempt limit.
        max_attempts: f64,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        final_error: Option<String>,
    },
    /// An agent event, as the agent delivered it.
    #[serde(untagged)]
    Agent(Arc<RwLock<AgentEvent<CustomAgentMessages>>>),
}

/// Hears whether a prompt passed its preflight, once per prompt.
pub type PreflightResult = Rc<dyn Fn(bool)>;

/// How a prompt is sent.
#[derive(Default)]
pub struct PromptOptions {
    /// Whether commands, skills and templates expand; absent means they do.
    pub expand_prompt_templates: Option<bool>,
    /// Images sent after the text; an empty list stays present for the extensions.
    pub images: Option<Vec<ImageContent>>,
    /// How a prompt that arrives while a response streams is queued; required then.
    pub streaming_behavior: Option<UserMessageDelivery>,
    /// Where the input came from; absent means interactive.
    pub source: Option<InputSource>,
    /// Hears whether the prompt passed its preflight.
    pub preflight_result: Option<PreflightResult>,
}

/// The pending steering and follow-up texts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueTexts {
    /// Pending steering texts.
    pub steering: Vec<String>,
    /// Pending follow-up texts.
    pub follow_up: Vec<String>,
}

/// Hears each cleaned output chunk of a direct shell command as it arrives.
pub type BashChunkCallback = Box<dyn FnMut(&str)>;

/// How a direct shell command runs and is recorded.
#[derive(Clone, Default)]
pub struct BashRunOptions {
    /// Whether the recorded output is kept out of the model context; absent is not recorded.
    pub exclude_from_context: Option<bool>,
    /// The operations that run the command; absent means the local shell.
    pub operations: Option<Rc<dyn maestro_tools::BashOperations<Local>>>,
}

/// A session listener.
pub type AgentSessionEventListener = Rc<dyn Fn(&AgentSessionEvent)>;

/// The owners a session works over.
pub struct AgentSessionConfig<E: ExtensionExecution = Local> {
    /// The agent whose state the session reads and changes.
    pub agent: Rc<Agent<CustomAgentMessages, Local>>,
    /// The session history.
    pub session_manager: Rc<RefCell<SessionManager>>,
    /// The preferences.
    pub settings_manager: Rc<RefCell<SettingsManager>>,
    /// The session working directory.
    pub cwd: String,
    /// The directory of the tool binaries that shell commands find first.
    pub bin_dir: String,
    /// Models to cycle through, in order.
    pub scoped_models: Vec<ScopedModel>,
    /// The loaded skills, prompts, context files, extensions and system prompt.
    pub resource_loader: Rc<dyn ResourceLoader<E>>,
    /// Tool definitions registered outside extensions, each with an `<sdk:name>` source.
    pub custom_tools: Vec<ToolDefinitionHandle<E>>,
    /// The model catalog used for credentials and model discovery.
    pub model_registry: Arc<ModelRegistry>,
    /// The built-in tools activated first; absent means read, bash, edit and write.
    pub initial_active_tool_names: Option<Vec<String>>,
    /// The only tool names exposed, when present.
    pub allowed_tool_names: Option<Vec<String>>,
    /// Executable tools used in place of the built-in ones, by name.
    pub base_tools_override: Option<IndexMap<String, SharedAgentTool<Local>>>,
    /// A slot that always holds the runner currently in effect.
    pub extension_runner_ref: Option<ExtensionRunnerRef<E>>,
    /// The start event extensions receive when they are bound; absent means `startup`.
    pub session_start_event: Option<SessionStartEvent>,
    /// The search index the built-in grep and find tools share.
    pub tools_manager: Arc<ToolsManager>,
    /// The published theme extensions see while no user interface is bound.
    pub theme: LiveTheme,
}

/// A slot that always holds the runner currently in effect.
pub type ExtensionRunnerRef<E = Local> = Rc<RefCell<Option<Rc<ExtensionRunner<E>>>>>;

/// A retained tool definition, as extensions register them.
pub type ToolDefinitionHandle<E = Local> =
    <E as maestro_models::Execution>::Shared<<E as ExtensionExecution>::RegisteredDefinition>;

/// The host operations a session binds to its extensions; an absent field leaves the bound
/// operation unchanged.
#[derive(Clone, Default)]
pub struct ExtensionBindings {
    /// The user interface extensions draw on.
    pub ui_context: Option<ExtensionUIContextBinding>,
    /// The session operations behind command contexts.
    pub command_context_actions: Option<ExtensionCommandContextActions>,
    /// Handles an extension's request to abort; absent, the session aborts its own run.
    pub abort_handler: Option<Rc<dyn Fn()>>,
    /// Requests a graceful shutdown.
    pub shutdown_handler: Option<ShutdownHandler>,
    /// Hears extension failures.
    pub on_error: Option<ExtensionErrorListener>,
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
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCycleResult {
    /// The selected model.
    pub model: Arc<RwLock<Model>>,
    /// The thinking level after clamping to the model.
    pub thinking_level: ModelThinkingLevel,
    /// Whether the scoped models were cycled rather than every available model.
    pub is_scoped: bool,
}

/// Token totals of a session.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    /// The session file, absent in memory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkMessage {
    /// The message entry.
    pub entry_id: String,
    /// The message text.
    pub text: String,
}
