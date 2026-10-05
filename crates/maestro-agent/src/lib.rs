//! Owned conversation and executable-tool turns through injected model access.
//! Ordered subscribers observe reduced state and participate in low-level idle.
#![doc = include_str!("../../../docs/agent.md")]
mod agent;
mod batch;
mod events;
mod progress;
mod queues;
mod run;
mod tool;
mod types;
pub use agent::{Agent, RunHandle, SubscriptionId};
pub use tool::{
    AfterToolCall, AfterToolCallContext, AfterToolCallResult, BeforeToolCall,
    BeforeToolCallContext, BeforeToolCallResult, Tool, ToolExecute, ToolExecutionMode,
    ToolInvocation, ToolPrepare, ToolProgress, ToolResult,
};
pub use types::{
    AgentContext, AgentError, AgentEvent, AgentListener, AgentMessage, AgentOptions, AgentState,
    ContextTransform, MessageConverter, Queue, QueueMode, StopAfterTurn, StopAfterTurnContext,
};
