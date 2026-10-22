//! The one session state, and the bridge between typed agent messages and stored records.
use indexmap::IndexMap;
use maestro_agent::SharedAgentTool;
use maestro_catalog::ScopedModel;
use maestro_extensions::{ExtensionExecution, NativeAgentMessage, RegisteredTool};
use maestro_models::{DiagnosticErrorInfo, Local};
use serde_json::Value;

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
}

impl<E: ExtensionExecution> AgentSessionState<E> {
    /// Empty registries over the supplied scoped models.
    pub(super) fn new(scoped_models: Vec<ScopedModel>) -> Self {
        Self {
            scoped_models,
            tool_definitions: IndexMap::new(),
            tool_registry: IndexMap::new(),
            tool_prompt_snippets: IndexMap::new(),
            tool_prompt_guidelines: IndexMap::new(),
        }
    }
}

/// Typed messages from stored message records.
pub(super) fn agent_messages(
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
pub(super) fn content_text(content: Option<&Value>) -> String {
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

/// The thrown form of a lower failure.
pub(super) fn thrown(error: impl std::fmt::Display) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::error(error.to_string())
}
