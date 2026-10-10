#![doc = include_str!("../../../docs/agent.md")]
/// State ownership and queue control.
pub mod agent;
/// Message and executable-tool records.
pub mod types;
pub use agent::{Agent, AgentInitialState, AgentOptions, AgentState};
pub use types::*;
/// Awaited conversation operations.
pub mod agent_loop;
pub use agent_loop::{AgentEventSink, AgentLoopOptions, run_agent_loop, run_agent_loop_continue};
