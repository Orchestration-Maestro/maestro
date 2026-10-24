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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueueMode {
    /// Consume all queued input together.
    All,
    /// Consume one queued entry at a time.
    #[default]
    OneAtATime,
}
/// A caller-defined message role with a typed payload.
///
/// Context dispatch clones the payload under a read lock. A payload's `Clone`
/// implementation must not take a write lock on the same message.
pub trait CustomAgentMessages {
    /// The custom message's role.
    ///
    /// Called while reading the message: do not access its own lock or call back into the agent.
    fn role(&self) -> &str;
}
impl CustomAgentMessages for Infallible {
    fn role(&self) -> &str {
        match *self {}
    }
}
/// Empty custom-role set for standard conversations.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NoCustomMessages {}
impl CustomAgentMessages for NoCustomMessages {
    fn role(&self) -> &str {
        match *self {}
    }
}
/// A shared model-owned or caller-defined conversation entry.
#[derive(serde::Deserialize)]
#[serde(tag = "role")]
pub enum AgentMessage<C: CustomAgentMessages = NoCustomMessages> {
    /// User input.
    #[serde(rename = "user")]
    User(Arc<RwLock<UserMessage>>),
    /// Assistant response.
    #[serde(rename = "assistant")]
    Assistant(SharedAssistantMessage),
    /// Tool output.
    #[serde(rename = "toolResult")]
    ToolResult(Arc<RwLock<ToolResultMessage>>),
    /// Caller-defined input.
    #[serde(untagged)]
    Custom(Arc<RwLock<C>>),
}
impl<C: CustomAgentMessages + serde::Serialize> serde::Serialize for AgentMessage<C> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::User(value) => value.serialize(serializer),
            Self::Assistant(value) => value.serialize(serializer),
            Self::ToolResult(value) => value.serialize(serializer),
            Self::Custom(value) => value.serialize(serializer),
        }
    }
}
impl<C: CustomAgentMessages> AgentMessage<C> {
    /// Return an owned snapshot of a standard model message, or `None` for custom input.
    #[must_use]
    pub fn to_message(&self) -> Option<maestro_models::Message> {
        use crate::agent::read;
        use maestro_models::Message;
        match self {
            Self::User(value) => Some(Message::User(read(value).clone())),
            Self::Assistant(value) => Some(Message::Assistant(read(value).clone())),
            Self::ToolResult(value) => Some(Message::ToolResult(read(value).clone())),
            Self::Custom(_) => None,
        }
    }
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
/// Conversation observations.
mod events;
/// Model-boundary callbacks and conversation inputs.
mod loop_config;
pub use events::*;
pub use loop_config::*;

/// Tool callback records.
mod hooks;
pub use hooks::*;

/// Execution affinity representations.
pub(crate) mod execution;
pub use execution::{AgentExecution, ExecutionFor};
pub use maestro_models::{Execution, Local, Threaded};
