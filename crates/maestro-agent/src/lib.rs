#![doc = include_str!("../../../docs/agent.md")]
/// State ownership and queue control.
pub mod agent;
/// Message and executable-tool records.
pub mod types;
pub use agent::{Agent, AgentInitialState, AgentOptions, AgentState};
pub use types::*;
