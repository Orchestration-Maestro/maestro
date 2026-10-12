#![doc = include_str!("../../../docs/agent.md")]
/// State ownership, prompt execution and queue control.
pub mod agent;
/// Message and executable-tool records.
pub mod types;
pub use agent::{
    Agent, AgentInitialState, AgentListener, AgentOptions, AgentSpawner, AgentState, PromptInput,
    Subscription,
};
pub use types::*;
/// Awaited and observable conversation operations.
pub mod agent_loop;
pub use agent_loop::{
    AgentEventSink, AgentLoopOptions, agent_loop, agent_loop_continue, run_agent_loop,
    run_agent_loop_continue,
};

/// Compact proxy requests and response reconstruction.
pub mod proxy;
/// The Agent guide, compiled and run as documentation tests.
#[cfg(doctest)]
#[doc = include_str!("../../../docs/agent/api.md")]
mod guide {}
pub use proxy::{ProxyAssistantMessageEvent, ProxyStreamOptions, stream_proxy};
