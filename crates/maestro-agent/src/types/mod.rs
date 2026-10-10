//! Shared message records and executable tool declarations.
use maestro_models::{SharedAssistantMessage, ToolResultMessage, UserMessage};
use std::{
    convert::Infallible,
    sync::{Arc, RwLock},
};
/// Executable tool records and callback signatures.
mod tools;
pub use maestro_models::ModelThinkingLevel as ThinkingLevel;
pub use tools::*;
/// Queue consumption policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueueMode {
    /// Consume all queued input together.
    All,
    /// Consume one queued entry at a time.
    #[default]
    OneAtATime,
}
/// A caller-defined message role with a typed payload.
pub trait CustomAgentMessages {
    /// The custom message's role.
    fn role(&self) -> &str;
}
impl CustomAgentMessages for Infallible {
    fn role(&self) -> &str {
        match *self {}
    }
}
/// A shared model-owned or caller-defined conversation entry.
pub enum AgentMessage<C: CustomAgentMessages = Infallible> {
    /// User input.
    User(Arc<RwLock<UserMessage>>),
    /// Assistant response.
    Assistant(SharedAssistantMessage),
    /// Tool output.
    ToolResult(Arc<RwLock<ToolResultMessage>>),
    /// Caller-defined input.
    Custom(Arc<RwLock<C>>),
}
impl<C: CustomAgentMessages> Clone for AgentMessage<C> {
    fn clone(&self) -> Self {
        match self {
            Self::User(entry) => Self::User(Arc::clone(entry)),
            Self::Assistant(entry) => Self::Assistant(Arc::clone(entry)),
            Self::ToolResult(entry) => Self::ToolResult(Arc::clone(entry)),
            Self::Custom(entry) => Self::Custom(Arc::clone(entry)),
        }
    }
}
