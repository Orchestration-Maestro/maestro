use crate::{AbortSignal, BashOperations};
use serde_json::Value;
use std::rc::Rc;

pub use crate::bindings::maestro::extension::types::{
    AfterProviderResponseEvent, AgentStartEvent, AnthropicMessagesCompat, AuthStatus,
    AutocompleteItem, BashResult, BeforeAgentStartEvent, BuildSystemPromptOptions,
    CompactionSettings, ContentPart, ContextFile, ContextUsage, ExtensionError,
    ExtensionFlagOptions, FileOperations, FlagType, FlagValue, ForkPosition, ImageContent,
    InputEvent, InputEventResult, InputEventResultTransform, InputSource, MaxPrice,
    MessageDelivery, Modality, Model, ModelCompat, ModelCost, ModelSelectEvent, ModelSelectSource,
    NavigateTreeOptions, NewSessionOptions, NumberOrString,
    OpenAiCompletionsCompat as OpenAICompletionsCompat,
    OpenAiResponsesCompat as OpenAIResponsesCompat, OpenRouterRouting, RenderShell,
    ResolvedRequestAuth, ResolvedRequestAuthFailure, ResolvedRequestAuthSuccess,
    ResourcesDiscoverEvent, ResourcesDiscoverReason, ResourcesDiscoverResult, RoutingPercentiles,
    RoutingPercentilesFields, RoutingSort, RoutingSortFields, SendMessageOptions,
    SendUserMessageOptions, SessionBeforeForkEvent, SessionBeforeForkResult,
    SessionBeforeSwitchEvent, SessionBeforeSwitchResult, SessionModel, SessionOutcome,
    SessionShutdownEvent, SessionShutdownReason, SessionStartEvent, SessionStartReason,
    SessionSwitchReason, Skill, SlashCommandInfo, SourceInfo, TextContent, ThinkingLevel,
    ThinkingLevelMap, ThinkingLevelSelectEvent, ToolCallEventResult, ToolExecutionMode,
    TurnStartEvent, UserBashEvent, UserContent, UserMessageDelivery, VercelGatewayRouting,
};

/// An event before a tool executes, with its arguments unchanged.
pub struct ToolCallEvent {
    /// Invocation identity.
    pub tool_call_id: String,
    /// Tool name used for case-sensitive narrowing.
    pub tool_name: String,
    /// Arbitrary tool arguments.
    pub input: Value,
}

impl ToolCallEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "tool_call"
    }
}

/// An event after a tool executes.
pub struct ToolResultEvent {
    /// Invocation identity.
    pub tool_call_id: String,
    /// Tool name used for case-sensitive narrowing.
    pub tool_name: String,
    /// Original arbitrary tool arguments.
    pub input: Value,
    /// Result content, including signatures and images.
    pub content: Vec<ContentPart>,
    /// Whether execution failed.
    pub is_error: bool,
    /// Arbitrary details; absence differs from JSON null.
    pub details: Option<Value>,
}

impl ToolResultEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "tool_result"
    }
}

/// Agent Tool Result.
pub struct AgentToolResult {
    /// Content.
    pub content: Vec<ContentPart>,
    /// Details.
    pub details: Option<Value>,
    /// Terminate.
    pub terminate: Option<bool>,
}
/// Custom Message.
pub struct CustomMessage {
    /// Custom type.
    pub custom_type: String,
    /// Content.
    pub content: UserContent,
    /// Display.
    pub display: bool,
    /// Details.
    pub details: Option<Value>,
}
/// Tool Info.
pub struct ToolInfo {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Parameters.
    pub parameters: Value,
    /// Source info.
    pub source_info: SourceInfo,
}
/// Session Context.
pub struct SessionContext {
    /// Messages.
    pub messages: Vec<Value>,
    /// Thinking level.
    pub thinking_level: String,
    /// Model.
    pub model: Option<SessionModel>,
}
/// Assistant Message Event.
pub enum AssistantMessageEvent {
    /// Start.
    Start {
        /// Partial.
        partial: Value,
    },
    /// TextStart.
    TextStart {
        /// Content index.
        content_index: f64,
        /// Partial.
        partial: Value,
    },
    /// TextDelta.
    TextDelta {
        /// Content index.
        content_index: f64,
        /// Delta.
        delta: String,
        /// Partial.
        partial: Value,
    },
    /// TextEnd.
    TextEnd {
        /// Content index.
        content_index: f64,
        /// Content.
        content: String,
        /// Partial.
        partial: Value,
    },
    /// ThinkingStart.
    ThinkingStart {
        /// Content index.
        content_index: f64,
        /// Partial.
        partial: Value,
    },
    /// ThinkingDelta.
    ThinkingDelta {
        /// Content index.
        content_index: f64,
        /// Delta.
        delta: String,
        /// Partial.
        partial: Value,
    },
    /// ThinkingEnd.
    ThinkingEnd {
        /// Content index.
        content_index: f64,
        /// Content.
        content: String,
        /// Partial.
        partial: Value,
    },
    /// ToolcallStart.
    ToolcallStart {
        /// Content index.
        content_index: f64,
        /// Partial.
        partial: Value,
    },
    /// ToolcallDelta.
    ToolcallDelta {
        /// Content index.
        content_index: f64,
        /// Delta.
        delta: String,
        /// Partial.
        partial: Value,
    },
    /// ToolcallEnd.
    ToolcallEnd {
        /// Content index.
        content_index: f64,
        /// Tool call.
        tool_call: ToolCall,
        /// Partial.
        partial: Value,
    },
    /// Done.
    Done {
        /// Reason.
        reason: String,
        /// Message.
        message: Value,
    },
    /// Error.
    Error {
        /// Reason.
        reason: String,
        /// Error.
        error: Value,
    },
}
/// Tool Call.
pub struct ToolCall {
    /// Id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Arguments.
    pub arguments: Value,
    /// Thought signature.
    pub thought_signature: Option<String>,
}
/// Session Before Compact Event.
pub struct SessionBeforeCompactEvent {
    /// Preparation.
    pub preparation: CompactionPreparation,
    /// Branch entries.
    pub branch_entries: Vec<Value>,
    /// Custom instructions.
    pub custom_instructions: Option<String>,
    /// Signal.
    pub signal: Rc<dyn AbortSignal>,
}
/// Session Compact Event.
pub struct SessionCompactEvent {
    /// Compaction entry.
    pub compaction_entry: CompactionEntry,
    /// From extension.
    pub from_extension: bool,
}
/// Tree Preparation.
pub struct TreePreparation {
    /// Target id.
    pub target_id: String,
    /// Old leaf id.
    pub old_leaf_id: Option<String>,
    /// Common ancestor id.
    pub common_ancestor_id: Option<String>,
    /// Entries to summarize.
    pub entries_to_summarize: Vec<Value>,
    /// User wants summary.
    pub user_wants_summary: bool,
    /// Custom instructions.
    pub custom_instructions: Option<String>,
    /// Replace instructions.
    pub replace_instructions: Option<bool>,
    /// Label.
    pub label: Option<String>,
}
/// Session Before Tree Event.
pub struct SessionBeforeTreeEvent {
    /// Preparation.
    pub preparation: TreePreparation,
    /// Signal.
    pub signal: Rc<dyn AbortSignal>,
}
/// Session Tree Event.
pub struct SessionTreeEvent {
    /// New leaf id.
    pub new_leaf_id: Option<String>,
    /// Old leaf id.
    pub old_leaf_id: Option<String>,
    /// Summary entry.
    pub summary_entry: Option<Value>,
    /// From extension.
    pub from_extension: Option<bool>,
}
/// Context Event.
pub struct ContextEvent {
    /// Messages.
    pub messages: Vec<Value>,
}
/// Context Event Result.
pub struct ContextEventResult {
    /// Messages.
    pub messages: Option<Vec<Value>>,
}
/// Before Provider Request Event.
pub struct BeforeProviderRequestEvent {
    /// Payload.
    pub payload: Value,
}
/// Agent End Event.
pub struct AgentEndEvent {
    /// Messages.
    pub messages: Vec<Value>,
}
/// Turn End Event.
pub struct TurnEndEvent {
    /// Turn index.
    pub turn_index: f64,
    /// Message.
    pub message: Value,
    /// Tool results.
    pub tool_results: Vec<Value>,
}
/// Message Start Event.
pub struct MessageStartEvent {
    /// Message.
    pub message: Value,
}
/// Message End Event.
pub struct MessageEndEvent {
    /// Message.
    pub message: Value,
}
/// Message Update Event.
pub struct MessageUpdateEvent {
    /// Message.
    pub message: Value,
    /// Assistant message event.
    pub assistant_message_event: AssistantMessageEvent,
}
/// Tool Execution Start Event.
pub struct ToolExecutionStartEvent {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Args.
    pub args: Value,
}
/// Tool Execution Update Event.
pub struct ToolExecutionUpdateEvent {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Args.
    pub args: Value,
    /// Partial result.
    pub partial_result: Value,
}
/// Tool Execution End Event.
pub struct ToolExecutionEndEvent {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Result.
    pub result: Value,
    /// Is error.
    pub is_error: bool,
}
/// Tool Result Event Result.
pub struct ToolResultEventResult {
    /// Content.
    pub content: Option<Vec<ContentPart>>,
    /// Details.
    pub details: Option<Value>,
    /// Is error.
    pub is_error: Option<bool>,
}
/// Message End Event Result.
pub struct MessageEndEventResult {
    /// Message.
    pub message: Option<Value>,
}
/// Before Agent Start Event Result.
pub struct BeforeAgentStartEventResult {
    /// Message.
    pub message: Option<CustomMessage>,
    /// System prompt.
    pub system_prompt: Option<String>,
}
/// Session Before Compact Result.
pub struct SessionBeforeCompactResult {
    /// Cancel.
    pub cancel: Option<bool>,
    /// Compaction.
    pub compaction: Option<CompactionResult>,
}
/// Session Before Tree Result.
pub struct SessionBeforeTreeResult {
    /// Cancel.
    pub cancel: Option<bool>,
    /// Summary.
    pub summary: Option<BranchSummary>,
    /// Custom instructions.
    pub custom_instructions: Option<String>,
    /// Replace instructions.
    pub replace_instructions: Option<bool>,
    /// Label.
    pub label: Option<String>,
}
/// Branch Summary.
pub struct BranchSummary {
    /// Summary.
    pub summary: String,
    /// Details.
    pub details: Option<Value>,
}
/// Compaction Result.
pub struct CompactionResult {
    /// Summary.
    pub summary: String,
    /// First kept entry id.
    pub first_kept_entry_id: String,
    /// Tokens before.
    pub tokens_before: f64,
    /// Details.
    pub details: Option<Value>,
}
/// Compaction Preparation.
pub struct CompactionPreparation {
    /// First kept entry id.
    pub first_kept_entry_id: String,
    /// Messages to summarize.
    pub messages_to_summarize: Vec<Value>,
    /// Turn prefix messages.
    pub turn_prefix_messages: Vec<Value>,
    /// Is split turn.
    pub is_split_turn: bool,
    /// Tokens before.
    pub tokens_before: f64,
    /// Previous summary.
    pub previous_summary: Option<String>,
    /// File ops.
    pub file_ops: FileOperations,
    /// Settings.
    pub settings: CompactionSettings,
}
/// Compaction Entry.
pub struct CompactionEntry {
    /// Id.
    pub id: String,
    /// Parent id.
    pub parent_id: Option<String>,
    /// Timestamp.
    pub timestamp: String,
    /// Summary.
    pub summary: String,
    /// First kept entry id.
    pub first_kept_entry_id: String,
    /// Tokens before.
    pub tokens_before: f64,
    /// Details.
    pub details: Option<Value>,
    /// From hook.
    pub from_hook: Option<bool>,
}

/// Session Tree Node.
pub struct SessionTreeNode {
    /// Entry.
    pub entry: Value,
    /// Children.
    pub children: Vec<SessionTreeNode>,
    /// Label.
    pub label: Option<String>,
    /// Label timestamp.
    pub label_timestamp: Option<String>,
}

/// An arbitrary replacement provider payload; the handler's outer option carries absence.
pub type BeforeProviderRequestEventResult = Value;

/// Optional host operations or a supplied result for a user bash invocation.
pub struct UserBashEventResult {
    /// Retained host bash operations.
    pub operations: Option<BashOperations>,
    /// Optional completed bash result.
    pub result: Option<BashResult>,
}
