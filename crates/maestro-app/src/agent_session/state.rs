//! The one session state, and the bridge between typed agent messages and stored records.
use super::{ExtensionBindings, ToolDefinitionHandle};
use indexmap::IndexMap;
use maestro_agent::SharedAgentTool;
use maestro_agent::{AgentEvent, AgentMessage};
use maestro_catalog::ScopedModel;
use maestro_extensions::{
    BuildSystemPromptOptions, ExtensionExecution, NativeAgentMessage, RegisteredTool,
};
use maestro_models::{Cancellation, DiagnosticErrorInfo, Local, SharedAssistantMessage};
use maestro_session::messages::CustomAgentMessages;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

/// A release that every observer sees once.
pub(super) type Gate = futures_util::future::Shared<tokio::sync::oneshot::Receiver<()>>;

/// An agent event as the agent shares it with its listeners.
pub(super) type SharedAgentEvent = Arc<RwLock<AgentEvent<CustomAgentMessages>>>;

/// Session state the session owns beside its supplied owners.
pub(super) struct AgentSessionState<E: ExtensionExecution> {
    /// Models to cycle through, in order.
    pub(super) scoped_models: Vec<ScopedModel>,
    /// Tool definitions by name, in registry order.
    pub(super) tool_definitions: IndexMap<String, RegisteredTool<E>>,
    /// Executable tools by name, in definition order.
    pub(super) tool_registry: IndexMap<String, SharedAgentTool<Local>>,
    /// Normalized prompt snippets by tool name.
    pub(super) tool_prompt_snippets: IndexMap<String, String>,
    /// Normalized prompt guidelines by tool name.
    pub(super) tool_prompt_guidelines: IndexMap<String, Vec<String>>,
    /// The manual or automatic compaction now running.
    pub(super) compaction_signal: Option<Cancellation>,
    /// The automatic compaction now running.
    pub(super) auto_compaction_signal: Option<Cancellation>,
    /// The branch summary now running.
    pub(super) branch_summary_signal: Option<Cancellation>,
    /// Whether this turn already recovered from a context overflow.
    pub(super) overflow_recovery_attempted: bool,
    /// The response retry now waiting.
    pub(super) retry_signal: Option<Cancellation>,
    /// The retry attempts made in this turn.
    pub(super) retry_attempt: u32,
    /// The release every idle waiter awaits; present exactly while a run is active.
    pub(super) run_gate: Option<Gate>,
    /// Opens the run's release once, when the run settles.
    pub(super) run_open: Option<tokio::sync::oneshot::Sender<()>>,
    /// The system prompt built for the active tools, before any extension replaces it.
    pub(super) base_system_prompt: String,
    /// The system prompt the extensions set for the run in progress, including an empty one.
    pub(super) system_prompt_override: Option<String>,
    /// The options that built the base system prompt.
    pub(super) base_system_prompt_options: BuildSystemPromptOptions,
    /// Pending steering texts, as the agent's steering queue holds them.
    pub(super) steering_messages: Vec<String>,
    /// Pending follow-up texts, as the agent's follow-up queue holds them.
    pub(super) follow_up_messages: Vec<String>,
    /// Custom messages sent with the next prompt.
    pub(super) pending_next_turn_messages: Vec<NativeAgentMessage>,
    /// Custom messages that start no turn, kept until the current turn ends.
    pub(super) pending_custom_messages: Vec<NativeAgentMessage>,
    /// Shell results recorded while a response streamed, appended before the next prompt.
    pub(super) pending_bash_messages: VecDeque<NativeAgentMessage>,
    /// Stops the direct shell command now running.
    pub(super) bash_signal: Option<Cancellation>,
    /// Tool definitions registered outside extensions.
    pub(super) custom_tools: Vec<ToolDefinitionHandle<E>>,
    /// The built-in tool definitions by name, rebuilt with each runtime.
    pub(super) base_tool_definitions: IndexMap<String, ToolDefinitionHandle<E>>,
    /// The host operations bound to the extensions.
    pub(super) bindings: ExtensionBindings,
    /// Removes the bound error listener from the runner it was added to.
    pub(super) error_unsubscriber: Option<Box<dyn Fn()>>,
    /// The turn the extensions' turn events report, counted from the agent start.
    pub(super) turn_index: u32,
    /// The newest finished response, checked for retry and compaction when the run ends.
    pub(super) last_assistant_message: Option<SharedAssistantMessage>,
    /// Agent events waiting for the session's ordered processing.
    pub(super) agent_events: VecDeque<SharedAgentEvent>,
    /// Released when the queued agent events are processed; absent while none are.
    pub(super) agent_event_drain: Option<Gate>,
}

impl<E: ExtensionExecution> AgentSessionState<E> {
    /// The system prompt the agent uses now: the run's override, else the base prompt.
    pub(super) fn effective_system_prompt(&self) -> String {
        (self.system_prompt_override.clone()).unwrap_or_else(|| self.base_system_prompt.clone())
    }

    /// Empty registries over the supplied scoped models and SDK tools, with an empty base
    /// system prompt and the options it starts from.
    pub(super) fn new(
        scoped_models: Vec<ScopedModel>,
        custom_tools: Vec<ToolDefinitionHandle<E>>,
        base_system_prompt_options: BuildSystemPromptOptions,
    ) -> Self {
        Self {
            scoped_models,
            tool_definitions: IndexMap::new(),
            tool_registry: IndexMap::new(),
            tool_prompt_snippets: IndexMap::new(),
            tool_prompt_guidelines: IndexMap::new(),
            compaction_signal: None,
            auto_compaction_signal: None,
            branch_summary_signal: None,
            overflow_recovery_attempted: false,
            retry_signal: None,
            retry_attempt: 0,
            run_gate: None,
            run_open: None,
            base_system_prompt: String::new(),
            system_prompt_override: None,
            base_system_prompt_options,
            steering_messages: Vec::new(),
            follow_up_messages: Vec::new(),
            pending_next_turn_messages: Vec::new(),
            pending_custom_messages: Vec::new(),
            pending_bash_messages: VecDeque::new(),
            bash_signal: None,
            custom_tools,
            base_tool_definitions: IndexMap::new(),
            bindings: ExtensionBindings::default(),
            error_unsubscriber: None,
            turn_index: 0,
            last_assistant_message: None,
            agent_events: VecDeque::new(),
            agent_event_drain: None,
        }
    }
}

/// Typed messages from stored message records.
pub(crate) fn agent_messages(
    records: Vec<Value>,
) -> Result<Vec<NativeAgentMessage>, DiagnosticErrorInfo> {
    records
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(thrown)
}

/// Stored records of typed messages.
pub(super) fn message_records(
    messages: &[NativeAgentMessage],
) -> Result<Vec<Value>, DiagnosticErrorInfo> {
    messages
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(thrown)
}

/// The text blocks of stored message content joined without a separator; plain text as is.
pub(crate) fn content_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect(),
        _ => String::new(),
    }
}

/// The newest response in `messages`, aborted ones included.
pub(super) fn last_assistant_message(
    messages: &[NativeAgentMessage],
) -> Option<SharedAssistantMessage> {
    messages.iter().rev().find_map(|message| match message {
        AgentMessage::Assistant(response) => Some(Arc::clone(response)),
        _ => None,
    })
}

/// The thrown form of a lower failure: the diagnostic a supplied failure retains, else an
/// error carrying the failure's text.
pub(crate) fn thrown(
    error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> DiagnosticErrorInfo {
    let error = error.into();
    maestro_extensions::runner::retained_diagnostic(error.as_ref()).map_or_else(
        || DiagnosticErrorInfo::error(error.to_string()),
        Clone::clone,
    )
}
