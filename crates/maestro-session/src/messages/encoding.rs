//! JSON representation of the application's closed default message-role set.
use super::{
    BashExecutionMessage, BranchSummaryMessage, CompactionSummaryMessage, CustomAgentMessages,
    CustomMessage,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Each record checks its own role tag before accepting the payload.
#[derive(Deserialize)]
#[serde(untagged, bound(deserialize = "T: Deserialize<'de>"))]
enum ApplicationMessage<T> {
    /// Recorded shell execution.
    BashExecution(BashExecutionMessage),
    /// Extension-injected message.
    Custom(CustomMessage<T>),
    /// Branch summary.
    BranchSummary(BranchSummaryMessage),
    /// Compaction summary.
    CompactionSummary(CompactionSummaryMessage),
}

impl<T: Serialize> Serialize for CustomAgentMessages<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::BashExecution(message) => message.serialize(serializer),
            Self::Custom(message) => message.serialize(serializer),
            Self::BranchSummary(message) => message.serialize(serializer),
            Self::CompactionSummary(message) => message.serialize(serializer),
            Self::Other(message) => match *message {},
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for CustomAgentMessages<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match ApplicationMessage::deserialize(deserializer)? {
            ApplicationMessage::BashExecution(message) => Self::BashExecution(message),
            ApplicationMessage::Custom(message) => Self::Custom(message),
            ApplicationMessage::BranchSummary(message) => Self::BranchSummary(message),
            ApplicationMessage::CompactionSummary(message) => Self::CompactionSummary(message),
        })
    }
}
