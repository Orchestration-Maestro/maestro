//! Author-facing events and results over the generated wire unions.
//!
//! The wire unions carry cancellation signals and operation objects beside the event, so an
//! edit a handler makes survives a returned error and capabilities an author retains are
//! never handed back. Only the events and results that hold such a capability differ from
//! the wire; every other payload is the generated record itself.
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use super::context::{AbortSignal, BashOperations, ExtensionContext};
use super::extension_result::ExtensionFuture;
use crate::bindings::maestro::extension::events as wire;
use wire::{
    AfterProviderResponseEvent, AgentEndEvent, BeforeAgentStartEvent, BeforeAgentStartEventResult,
    BeforeProviderRequestEvent, ContextEvent, ContextEventResult, CustomEvent, InputEvent,
    InputEventResult, MessageEndEvent, MessageEndEventResult, MessageStartEvent,
    MessageUpdateEvent, ModelSelectEvent, ResourcesDiscoverEvent, ResourcesDiscoverResult,
    SessionBeforeCompactEventData, SessionBeforeCompactResult, SessionBeforeForkEvent,
    SessionBeforeForkResult, SessionBeforeSwitchEvent, SessionBeforeSwitchResult,
    SessionBeforeTreeEventData, SessionBeforeTreeResult, SessionCompactEvent, SessionShutdownEvent,
    SessionStartEvent, SessionTreeEvent, ThinkingLevelSelectEvent, ToolCallEvent,
    ToolCallEventResult, ToolExecutionEndEvent, ToolExecutionStartEvent, ToolExecutionUpdateEvent,
    ToolResultEvent, ToolResultEventResult, TurnEndEvent, TurnStartEvent, UserBashEvent,
    UserBashEventResultData,
};

/// Fired before context compaction; the handler may cancel or customize it.
#[derive(Debug)]
pub struct SessionBeforeCompactEvent {
    /// Preparation, branch entries and instructions.
    pub data: SessionBeforeCompactEventData,
    /// Cancels the compaction; usable after the handler returns.
    pub signal: AbortSignal,
}

/// Fired before navigating the session tree; the handler may cancel or customize it.
#[derive(Debug)]
pub struct SessionBeforeTreeEvent {
    /// Preparation of the navigation.
    pub data: SessionBeforeTreeEventData,
    /// Cancels the navigation; usable after the handler returns.
    pub signal: AbortSignal,
}

impl Deref for SessionBeforeCompactEvent {
    type Target = SessionBeforeCompactEventData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for SessionBeforeCompactEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl Deref for SessionBeforeTreeEvent {
    type Target = SessionBeforeTreeEventData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for SessionBeforeTreeEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

/// Events about the session itself.
#[derive(Debug)]
pub enum SessionEvent {
    /// A session started, was loaded or was reloaded.
    Start(SessionStartEvent),
    /// A switch to another session is about to happen.
    BeforeSwitch(SessionBeforeSwitchEvent),
    /// A fork is about to happen.
    BeforeFork(SessionBeforeForkEvent),
    /// A compaction is about to happen.
    BeforeCompact(SessionBeforeCompactEvent),
    /// A compaction finished.
    Compact(SessionCompactEvent),
    /// The extension runtime is about to be torn down.
    Shutdown(SessionShutdownEvent),
    /// A tree navigation is about to happen.
    BeforeTree(SessionBeforeTreeEvent),
    /// A tree navigation finished.
    Tree(SessionTreeEvent),
}

/// Every event an extension can receive.
#[derive(Debug)]
pub enum ExtensionEvent {
    /// Resource paths are requested.
    ResourcesDiscover(ResourcesDiscoverEvent),
    /// An event about the session.
    Session(SessionEvent),
    /// Messages are about to be sent to the model.
    Context(ContextEvent),
    /// A provider request is about to be sent.
    BeforeProviderRequest(BeforeProviderRequestEvent),
    /// A provider response was received.
    AfterProviderResponse(AfterProviderResponseEvent),
    /// A prompt was submitted before the agent loop starts.
    BeforeAgentStart(Box<BeforeAgentStartEvent>),
    /// An agent loop started.
    AgentStart,
    /// An agent loop ended.
    AgentEnd(AgentEndEvent),
    /// A turn started.
    TurnStart(TurnStartEvent),
    /// A turn ended.
    TurnEnd(TurnEndEvent),
    /// A message started.
    MessageStart(MessageStartEvent),
    /// A streamed assistant message changed.
    MessageUpdate(Box<MessageUpdateEvent>),
    /// A message ended.
    MessageEnd(MessageEndEvent),
    /// A tool started.
    ToolExecutionStart(ToolExecutionStartEvent),
    /// A tool reported progress.
    ToolExecutionUpdate(ToolExecutionUpdateEvent),
    /// A tool finished.
    ToolExecutionEnd(ToolExecutionEndEvent),
    /// A model was selected.
    ModelSelect(Box<ModelSelectEvent>),
    /// A thinking level was selected.
    ThinkingLevelSelect(ThinkingLevelSelectEvent),
    /// The user ran a bash command.
    UserBash(UserBashEvent),
    /// A user input before agent processing.
    Input(InputEvent),
    /// A tool is about to execute; the handler may edit its input.
    ToolCall(ToolCallEvent),
    /// A tool finished executing.
    ToolResult(ToolResultEvent),
    /// An event under a name the host does not define.
    Custom(CustomEvent),
}

/// Full replacement of a user bash command.
#[derive(Debug)]
pub struct UserBashEventResult {
    /// Replacement result, when the extension handled execution.
    pub data: UserBashEventResultData,
    /// Operations to execute the command with.
    pub operations: Option<BashOperations>,
}

/// Result a handler returns, matching the event it handled.
#[derive(Debug)]
pub enum ExtensionEventResult {
    /// Resource paths to add.
    ResourcesDiscover(ResourcesDiscoverResult),
    /// Replacement messages.
    Context(ContextEventResult),
    /// Replacement provider request, as JSON text.
    BeforeProviderRequest(String),
    /// Verdict of a tool call.
    ToolCall(ToolCallEventResult),
    /// Replacement of a user bash command.
    UserBash(UserBashEventResult),
    /// Replacement fields of a tool result.
    ToolResult(ToolResultEventResult),
    /// Replacement of a finalized message.
    MessageEnd(MessageEndEventResult),
    /// Message and system prompt to add.
    BeforeAgentStart(BeforeAgentStartEventResult),
    /// Verdict of a session switch.
    SessionBeforeSwitch(SessionBeforeSwitchResult),
    /// Verdict of a fork.
    SessionBeforeFork(SessionBeforeForkResult),
    /// Verdict or replacement of a compaction.
    SessionBeforeCompact(SessionBeforeCompactResult),
    /// Verdict or replacement of a tree navigation.
    SessionBeforeTree(SessionBeforeTreeResult),
    /// Decision about an input.
    Input(InputEventResult),
}

impl ExtensionEventResult {
    /// Splits the result into its wire form and the operations a user bash replacement
    /// supplies, which stay with the extension.
    #[must_use]
    pub fn into_wire(self) -> (wire::ExtensionEventResult, Option<BashOperations>) {
        let result = match self {
            Self::UserBash(UserBashEventResult { data, operations }) => {
                return (wire::ExtensionEventResult::UserBash(data), operations);
            }
            Self::ResourcesDiscover(r) => wire::ExtensionEventResult::ResourcesDiscover(r),
            Self::Context(r) => wire::ExtensionEventResult::Context(r),
            Self::BeforeProviderRequest(r) => wire::ExtensionEventResult::BeforeProviderRequest(r),
            Self::ToolCall(r) => wire::ExtensionEventResult::ToolCall(r),
            Self::ToolResult(r) => wire::ExtensionEventResult::ToolResult(r),
            Self::MessageEnd(r) => wire::ExtensionEventResult::MessageEnd(r),
            Self::BeforeAgentStart(r) => wire::ExtensionEventResult::BeforeAgentStart(r),
            Self::SessionBeforeSwitch(r) => wire::ExtensionEventResult::SessionBeforeSwitch(r),
            Self::SessionBeforeFork(r) => wire::ExtensionEventResult::SessionBeforeFork(r),
            Self::SessionBeforeCompact(r) => wire::ExtensionEventResult::SessionBeforeCompact(r),
            Self::SessionBeforeTree(r) => wire::ExtensionEventResult::SessionBeforeTree(r),
            Self::Input(r) => wire::ExtensionEventResult::Input(r),
        };
        (result, None)
    }
}

/// Handler of one event. It may edit the event in place and returns an optional result.
pub type ExtensionHandler = Rc<
    dyn for<'a> Fn(
        &'a mut ExtensionEvent,
        ExtensionContext,
    ) -> ExtensionFuture<'a, Option<ExtensionEventResult>>,
>;
