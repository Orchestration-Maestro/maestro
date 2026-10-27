//! Concrete native aliases of one shared schema.
/// Native application messages supplied to event handlers.
pub type NativeAgentMessage =
    maestro_agent::AgentMessage<maestro_session::messages::CustomAgentMessages>;
/// Existing native domain operand.
type Payload = serde_json::Value;
/// Existing native domain operand.
type Details = serde_json::Value;
/// Existing native domain operand.
type ToolInput = serde_json::Value;
/// Existing native domain operand.
type ToolDetails = serde_json::Value;
/// Existing native domain operand.
type Headers = std::collections::BTreeMap<String, String>;
/// Existing native domain operand.
type Model = std::sync::Arc<std::sync::RwLock<maestro_models::Model>>;
/// Existing native domain operand.
type Stream = maestro_models::AssistantMessageEvent;
/// Existing native domain operand.
type Prompt = maestro_models::system_prompt::BuildSystemPromptOptions;
/// Existing native domain operand.
type CustomInput = maestro_session::messages::CustomMessageInput;
/// Existing native domain operand.
type Signal = maestro_models::Cancellation;
/// Existing native domain operand.
type SessionEntry = maestro_session::session_manager::SessionEntry;
/// Existing native domain operand.
type BranchSummaryEntry = maestro_session::session_manager::BranchSummaryEntry;
/// Existing native domain operand.
type CompactionEntry = maestro_session::session_manager::CompactionEntry;
/// Existing native domain operand.
type Preparation = maestro_session::compaction::CompactionPreparation;
/// Existing native domain operand.
type CompactionResult = maestro_session::compaction::CompactionResult;
/// Shared event schema specialized for native operands.
pub type ContextEvent<M = NativeAgentMessage> =
    maestro_models::extension_events::agent_events::ContextEvent<M>;
/// Shared event schema specialized for native operands.
pub type BeforeAgentStartEvent =
    maestro_models::extension_events::agent_events::BeforeAgentStartEvent<Prompt>;
/// Shared event schema specialized for native operands.
pub type AgentStartEvent = maestro_models::extension_events::agent_events::AgentStartEvent;
/// Shared event schema specialized for native operands.
pub type AgentEndEvent<M = NativeAgentMessage> =
    maestro_models::extension_events::agent_events::AgentEndEvent<M>;
/// Shared event schema specialized for native operands.
pub type TurnStartEvent = maestro_models::extension_events::agent_events::TurnStartEvent;
/// Shared event schema specialized for native operands.
pub type TurnEndEvent<M = NativeAgentMessage> =
    maestro_models::extension_events::agent_events::TurnEndEvent<M>;
/// Shared event schema specialized for native operands.
pub type MessageStartEvent<M = NativeAgentMessage> =
    maestro_models::extension_events::agent_events::MessageStartEvent<M>;
/// Shared event schema specialized for native operands.
pub type MessageUpdateEvent<M = NativeAgentMessage> =
    maestro_models::extension_events::agent_events::MessageUpdateEvent<M, Stream>;
/// Shared event schema specialized for native operands.
pub type MessageEndEvent<M = NativeAgentMessage> =
    maestro_models::extension_events::agent_events::MessageEndEvent<M>;
/// Shared event schema specialized for native operands.
pub type ContextEventResult<M = NativeAgentMessage> =
    maestro_models::extension_events::agent_results::ContextEventResult<M>;
/// Shared event schema specialized for native operands.
pub type MessageEndEventResult<M = NativeAgentMessage> =
    maestro_models::extension_events::agent_results::MessageEndEventResult<M>;
/// Shared event schema specialized for native operands.
pub type BeforeAgentStartEventResult =
    maestro_models::extension_events::agent_results::BeforeAgentStartEventResult<CustomInput>;
/// Shared event schema specialized for native operands.
pub type ModelSelectSource = maestro_models::extension_events::selection_events::ModelSelectSource;
/// Shared event schema specialized for native operands.
pub type ModelSelectEvent =
    maestro_models::extension_events::selection_events::ModelSelectEvent<Model>;
/// Shared event schema specialized for native operands.
pub type ThinkingLevelSelectEvent =
    maestro_models::extension_events::selection_events::ThinkingLevelSelectEvent;
/// Shared event schema specialized for native operands.
pub type SessionCompactEvent =
    maestro_models::extension_events::session_events::SessionCompactEvent<CompactionEntry>;
/// Shared event schema specialized for native operands.
pub type TreePreparation =
    maestro_models::extension_events::session_events::TreePreparation<SessionEntry>;
/// Shared event schema specialized for native operands.
pub type SessionTreeEvent =
    maestro_models::extension_events::session_events::SessionTreeEvent<BranchSummaryEntry>;
/// Shared event schema specialized for native operands.
pub type SessionBeforeTreeSummary =
    maestro_models::extension_events::session_events::SessionBeforeTreeSummary<Details>;
/// Shared event schema specialized for native operands.
pub type SessionBeforeTreeResult =
    maestro_models::extension_events::session_events::SessionBeforeTreeResult<Details>;
/// Shared event schema specialized for native operands.
pub type SessionBeforeTreeEvent =
    maestro_models::extension_events::session_events::SessionBeforeTreeEvent<
        TreePreparation,
        Signal,
    >;
/// Shared event schema specialized for native operands.
pub type ToolExecutionStartEvent =
    maestro_models::extension_events::tool_events::ToolExecutionStartEvent<Payload>;
/// Shared event schema specialized for native operands.
pub type ToolExecutionUpdateEvent =
    maestro_models::extension_events::tool_events::ToolExecutionUpdateEvent<Payload, Payload>;
/// Shared event schema specialized for native operands.
pub type ToolExecutionEndEvent =
    maestro_models::extension_events::tool_events::ToolExecutionEndEvent<Payload>;
/// Shared event schema specialized for native operands.
pub type ToolCallEvent = maestro_models::extension_events::tool_events::ToolCallEvent<ToolInput>;
/// Shared event schema specialized for native operands.
pub type ToolResultEvent =
    maestro_models::extension_events::tool_events::ToolResultEvent<ToolDetails, Payload>;
/// Shared event schema specialized for native operands.
pub type CustomToolCallEvent =
    maestro_models::extension_events::tool_events::CustomToolCallEvent<Payload>;
/// Shared event schema specialized for native operands.
pub type CustomToolResultEvent =
    maestro_models::extension_events::tool_events::CustomToolResultEvent<Details, Payload>;
/// Shared event schema specialized for native operands.
pub type UserBashEvent = maestro_models::extension_events::tool_results::UserBashEvent;
/// Shared event schema specialized for native operands.
pub type UserBashEventResult<O, R> =
    maestro_models::extension_events::tool_results::UserBashEventResult<O, R>;
/// Shared event schema specialized for native operands.
pub type ToolCallEventResult = maestro_models::extension_events::tool_results::ToolCallEventResult;
/// Shared event schema specialized for native operands.
pub type ToolResultEventResult =
    maestro_models::extension_events::tool_results::ToolResultEventResult<Details>;
/// Shared event schema specialized for native operands.
pub type BeforeProviderRequestEvent =
    maestro_models::extension_events::events::BeforeProviderRequestEvent<Payload>;
/// Shared event schema specialized for native operands.
pub type BeforeProviderRequestEventResult =
    maestro_models::extension_events::events::BeforeProviderRequestEventResult<Payload>;
/// Shared event schema whose header collection `H` defaults to a native name-ordered map.
pub type AfterProviderResponseEvent<H = Headers> =
    maestro_models::extension_events::events::AfterProviderResponseEvent<H>;
/// Shared event schema specialized for native operands.
pub type SessionBeforeCompactEventData =
    maestro_models::extension_events::events::SessionBeforeCompactEventData<
        Preparation,
        SessionEntry,
    >;
/// Shared event schema specialized for native operands.
pub type SessionBeforeCompactEvent =
    maestro_models::extension_events::events::SessionBeforeCompactEvent<
        Preparation,
        SessionEntry,
        Signal,
    >;
/// Shared event schema specialized for native operands.
pub type SessionBeforeCompactResult =
    maestro_models::extension_events::events::SessionBeforeCompactResult<CompactionResult>;
/// Shared event schema specialized for native operands.
pub type ResourcesDiscoverReason =
    maestro_models::extension_events::events::ResourcesDiscoverReason;
/// Shared event schema specialized for native operands.
pub type ResourcesDiscoverEvent = maestro_models::extension_events::events::ResourcesDiscoverEvent;
/// Shared event schema specialized for native operands.
pub type ResourcesDiscoverResult =
    maestro_models::extension_events::events::ResourcesDiscoverResult;
/// Shared event schema specialized for native operands.
pub type SessionStartReason = maestro_models::extension_events::events::SessionStartReason;
/// Shared event schema specialized for native operands.
pub type SessionStartEvent = maestro_models::extension_events::events::SessionStartEvent;
/// Shared event schema specialized for native operands.
pub type SessionBeforeSwitchReason =
    maestro_models::extension_events::events::SessionBeforeSwitchReason;
/// Shared event schema specialized for native operands.
pub type SessionBeforeSwitchEvent =
    maestro_models::extension_events::events::SessionBeforeSwitchEvent;
/// Shared event schema specialized for native operands.
pub use maestro_session::session_manager::ForkPosition;
/// Shared event schema specialized for native operands.
pub type SessionBeforeForkEvent =
    maestro_models::extension_events::events::SessionBeforeForkEvent<ForkPosition>;
/// Shared event schema specialized for native operands.
pub type SessionShutdownReason = maestro_models::extension_events::events::SessionShutdownReason;
/// Shared event schema specialized for native operands.
pub type SessionShutdownEvent = maestro_models::extension_events::events::SessionShutdownEvent;
/// Shared event schema specialized for native operands.
pub type SessionBeforeSwitchResult =
    maestro_models::extension_events::events::SessionBeforeSwitchResult;
/// Shared event schema specialized for native operands.
pub type SessionBeforeForkResult =
    maestro_models::extension_events::events::SessionBeforeForkResult;
/// Shared event schema specialized for native operands.
pub type InputSource = maestro_models::extension_events::events::InputSource;
/// Shared event schema specialized for native operands.
pub type InputEvent = maestro_models::extension_events::events::InputEvent;
/// Shared event schema specialized for native operands.
pub type InputTransform = maestro_models::extension_events::events::InputTransform;
/// Shared event schema specialized for native operands.
pub type InputEventResult = maestro_models::extension_events::events::InputEventResult;
/// Built-in input view.
#[cfg(not(target_arch = "wasm32"))]
pub type BashToolCallEvent =
    maestro_models::extension_events::tool_events::ToolCallEvent<maestro_tools::BashToolInput>;
/// Built-in details view.
#[cfg(not(target_arch = "wasm32"))]
pub type BashToolResultEvent = maestro_models::extension_events::tool_events::ToolResultEvent<
    maestro_tools::BashToolDetails,
    Payload,
>;
/// Built-in input view.
#[cfg(not(target_arch = "wasm32"))]
pub type ReadToolCallEvent =
    maestro_models::extension_events::tool_events::ToolCallEvent<maestro_tools::ReadToolInput>;
/// Built-in details view.
#[cfg(not(target_arch = "wasm32"))]
pub type ReadToolResultEvent = maestro_models::extension_events::tool_events::ToolResultEvent<
    maestro_tools::ReadToolDetails,
    Payload,
>;
/// Built-in input view.
#[cfg(not(target_arch = "wasm32"))]
pub type EditToolCallEvent =
    maestro_models::extension_events::tool_events::ToolCallEvent<maestro_tools::EditToolInput>;
/// Built-in details view.
#[cfg(not(target_arch = "wasm32"))]
pub type EditToolResultEvent = maestro_models::extension_events::tool_events::ToolResultEvent<
    maestro_tools::EditToolDetails,
    Payload,
>;
/// Built-in input view.
#[cfg(not(target_arch = "wasm32"))]
pub type WriteToolCallEvent =
    maestro_models::extension_events::tool_events::ToolCallEvent<maestro_tools::WriteToolInput>;
/// Built-in details view.
#[cfg(not(target_arch = "wasm32"))]
pub type WriteToolResultEvent =
    maestro_models::extension_events::tool_events::ToolResultEvent<(), Payload>;
/// Built-in input view.
#[cfg(not(target_arch = "wasm32"))]
pub type GrepToolCallEvent =
    maestro_models::extension_events::tool_events::ToolCallEvent<maestro_tools::GrepToolInput>;
/// Built-in details view.
#[cfg(not(target_arch = "wasm32"))]
pub type GrepToolResultEvent = maestro_models::extension_events::tool_events::ToolResultEvent<
    maestro_tools::GrepToolDetails,
    Payload,
>;
/// Built-in input view.
#[cfg(not(target_arch = "wasm32"))]
pub type FindToolCallEvent =
    maestro_models::extension_events::tool_events::ToolCallEvent<maestro_tools::FindToolInput>;
/// Built-in details view.
#[cfg(not(target_arch = "wasm32"))]
pub type FindToolResultEvent = maestro_models::extension_events::tool_events::ToolResultEvent<
    maestro_tools::FindToolDetails,
    Payload,
>;
/// Built-in input view.
#[cfg(not(target_arch = "wasm32"))]
pub type LsToolCallEvent =
    maestro_models::extension_events::tool_events::ToolCallEvent<maestro_tools::LsToolInput>;
/// Built-in details view.
#[cfg(not(target_arch = "wasm32"))]
pub type LsToolResultEvent = maestro_models::extension_events::tool_events::ToolResultEvent<
    maestro_tools::LsToolDetails,
    Payload,
>;
/// One session event with its concrete capability owners.
pub type SessionEvent = maestro_models::extension_events::unions::SessionEvent<
    SessionBeforeForkEvent,
    SessionBeforeCompactEvent,
    SessionCompactEvent,
    SessionBeforeTreeEvent,
    SessionTreeEvent,
>;
/// All 29 event forms.
pub type ExtensionEvent<M = NativeAgentMessage> =
    maestro_models::extension_events::unions::ExtensionEvent<
        SessionEvent,
        ContextEvent<M>,
        BeforeAgentStartEvent,
        AgentEndEvent<M>,
        TurnEndEvent<M>,
        MessageStartEvent<M>,
        MessageUpdateEvent<M>,
        MessageEndEvent<M>,
        ToolExecutionStartEvent,
        ToolExecutionUpdateEvent,
        ToolExecutionEndEvent,
        ModelSelectEvent,
        ToolCallEvent,
        ToolResultEvent,
        BeforeProviderRequestEvent,
        AfterProviderResponseEvent,
    >;
/// Each non-void result family.
pub type ExtensionEventResult<B, M = NativeAgentMessage> =
    maestro_models::extension_events::unions::ExtensionEventResult<
        ContextEventResult<M>,
        MessageEndEventResult<M>,
        BeforeAgentStartEventResult,
        SessionBeforeCompactResult,
        SessionBeforeTreeResult,
        ToolResultEventResult,
        B,
        BeforeProviderRequestEventResult,
    >;

pub use maestro_models::extension_events::tool_events::{
    is_bash_tool_result, is_edit_tool_result, is_find_tool_result, is_grep_tool_result,
    is_ls_tool_result, is_read_tool_result, is_tool_call_event_type, is_write_tool_result,
};
