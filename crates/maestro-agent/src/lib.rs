#![doc = include_str!("../../../docs/agent.md")]
/// State ownership, prompt execution and queue control.
pub mod runtime;
/// Message and executable-tool records.
pub mod types;
pub use runtime::{
    Agent, AgentInitialState, AgentListener, AgentOptions, AgentSpawner, AgentState, PromptInput,
    Subscription,
};
pub use types::*;
/// Awaited and observable conversation operations.
pub mod execution;
pub use execution::{
    AgentEventSink, AgentLoopOptions, agent_loop, agent_loop_continue, run_agent_loop,
    run_agent_loop_continue,
};

/// Compact proxy requests and response reconstruction.
pub mod remote;
/// The Agent guide, compiled and run as documentation tests.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/agent/api.md")]
mod guide {}
pub use remote::{ProxyAssistantMessageEvent, ProxyStreamOptions, stream_proxy};
