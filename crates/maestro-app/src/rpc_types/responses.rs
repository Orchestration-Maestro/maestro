//! Correlated responses: session results in their declared shapes, failures, and workspace
//! responses, all written through the shared envelope.
use super::{RpcSessionState, RpcSlashCommand, workspace};
use crate::agent_session::{ForkMessage, ModelCycleResult, SessionStats};
use maestro_extensions::NativeAgentMessage;
use maestro_models::{Model, ModelThinkingLevel};
use maestro_session::CompactionResult;
use maestro_tools::BashResult;
use serde::{Serialize, Serializer};
use std::sync::{Arc, RwLock};

/// The data of a successful session command.
#[derive(Serialize)]
#[serde(untagged)]
pub enum RpcData {
    /// No `data` member.
    #[serde(skip)]
    Empty,
    /// A present `null`.
    Null,
    /// The session state.
    State(RpcSessionState),
    /// A model.
    Model(Arc<RwLock<Model>>),
    /// The available models.
    Models {
        /// The models in catalog order.
        models: Vec<Arc<RwLock<Model>>>,
    },
    /// A model selected by cycling.
    ModelCycle(ModelCycleResult),
    /// A thinking level selected by cycling.
    ThinkingLevel {
        /// The level.
        level: ModelThinkingLevel,
    },
    /// A shell command's result.
    Bash(BashResult),
    /// A compaction's result.
    Compact(CompactionResult),
    /// The session statistics.
    Stats(SessionStats),
    /// Whether a session change was cancelled.
    Cancelled {
        /// Whether it was cancelled.
        cancelled: bool,
    },
    /// A fork's outcome.
    Fork {
        /// The text of the user message the fork starts before.
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// Whether it was cancelled.
        cancelled: bool,
    },
    /// A written file.
    Path {
        /// Its path.
        path: String,
    },
    /// The user messages the session can be forked from.
    ForkMessages {
        /// The messages in session order.
        messages: Vec<ForkMessage>,
    },
    /// The last assistant text.
    LastAssistantText {
        /// The text, absent without an assistant message.
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// The session messages.
    Messages {
        /// The shared message list.
        messages: Arc<RwLock<Vec<NativeAgentMessage>>>,
    },
    /// The commands a prompt can invoke.
    Commands {
        /// Extension commands, then prompt templates, then skills.
        commands: Vec<RpcSlashCommand>,
    },
}

/// A correlated response.
pub enum RpcResponse {
    /// A session command succeeded.
    Success {
        /// The request's identifier.
        id: Option<String>,
        /// The command type.
        command: String,
        /// The result.
        data: RpcData,
    },
    /// A command failed, or a record was not a command.
    Error {
        /// The request's identifier, when the failure is correlated.
        id: Option<String>,
        /// The command type, when the record had one.
        command: Option<String>,
        /// The failure message.
        error: String,
    },
    /// A workspace command's response.
    Workspace(Box<workspace::RpcResponse>),
}

impl Serialize for RpcResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Success { id, command, data } => workspace::serialize_rpc_response(
                serializer,
                id.as_deref(),
                Some(command),
                Ok((!matches!(data, RpcData::Empty)).then_some(data)),
            ),
            Self::Error { id, command, error } => workspace::serialize_rpc_response::<_, ()>(
                serializer,
                id.as_deref(),
                command.as_deref(),
                Err(workspace::RpcErrorRef {
                    message: error,
                    code: None,
                }),
            ),
            Self::Workspace(response) => response.serialize(serializer),
        }
    }
}
