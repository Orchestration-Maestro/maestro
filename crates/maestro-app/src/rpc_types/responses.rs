//! Correlated responses: session results in their declared shapes, failures, and workspace
//! responses, all written through the shared envelope.
use super::commands::names;
use super::{RpcSessionState, RpcSlashCommand, workspace};
use crate::agent_session::{ForkMessage, ModelCycleResult, SessionStats};
use maestro_extensions::NativeAgentMessage;
use maestro_models::{Model, ModelThinkingLevel};
use maestro_session::{CompactionResult, Presence};
use maestro_tools::BashResult;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use serde_json::Value;
use std::sync::{Arc, RwLock};

/// Whether a session change was cancelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcCancelled {
    /// Whether it was cancelled.
    pub cancelled: bool,
}

/// A fork's outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcForkResult {
    /// The text of the user message the fork starts before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Whether it was cancelled.
    pub cancelled: bool,
}

/// A written file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcExportPath {
    /// Its path.
    pub path: String,
}

/// A thinking level selected by cycling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcThinkingLevel {
    /// The level.
    pub level: ModelThinkingLevel,
}

/// The available models.
#[derive(Serialize, Deserialize)]
pub struct RpcAvailableModels {
    /// The models in catalog order.
    pub models: Vec<Arc<RwLock<Model>>>,
}

/// The user messages a session can be forked from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcForkMessages {
    /// The messages in session order.
    pub messages: Vec<ForkMessage>,
}

/// The last assistant text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RpcLastAssistantText {
    /// The text: absent without an assistant message, and present `null` when a client
    /// reads one.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub text: Presence<String>,
}

/// The session messages.
#[derive(Serialize, Deserialize)]
pub struct RpcMessages {
    /// The shared message list.
    pub messages: Arc<RwLock<Vec<NativeAgentMessage>>>,
}

/// The commands a prompt can invoke.
#[derive(Serialize, Deserialize)]
pub struct RpcCommandList {
    /// Extension commands, then prompt templates, then skills.
    pub commands: Vec<RpcSlashCommand>,
}

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
    Models(RpcAvailableModels),
    /// A model selected by cycling.
    ModelCycle(ModelCycleResult),
    /// A thinking level selected by cycling.
    ThinkingLevel(RpcThinkingLevel),
    /// A shell command's result.
    Bash(BashResult),
    /// A compaction's result.
    Compact(CompactionResult),
    /// The session statistics.
    Stats(SessionStats),
    /// Whether a session change was cancelled.
    Cancelled(RpcCancelled),
    /// A fork's outcome.
    Fork(RpcForkResult),
    /// A written file.
    Path(RpcExportPath),
    /// The user messages the session can be forked from.
    ForkMessages(RpcForkMessages),
    /// The last assistant text.
    LastAssistantText(RpcLastAssistantText),
    /// The session messages.
    Messages(RpcMessages),
    /// The commands a prompt can invoke.
    Commands(RpcCommandList),
}

impl RpcData {
    /// Decodes the `data` member of a successful `command` in the shape that command
    /// answers; `null` is [`RpcData::Null`].
    ///
    /// # Errors
    /// Returns the decoder's failure, or a failure for a command that answers no data.
    pub fn decode(command: &str, data: &Value) -> Result<Self, serde_json::Error> {
        if data.is_null() {
            return Ok(Self::Null);
        }
        Ok(match command {
            "get_state" => Self::State(Deserialize::deserialize(data)?),
            "set_model" => Self::Model(Deserialize::deserialize(data)?),
            "get_available_models" => Self::Models(Deserialize::deserialize(data)?),
            "cycle_model" => Self::ModelCycle(Deserialize::deserialize(data)?),
            "cycle_thinking_level" => Self::ThinkingLevel(Deserialize::deserialize(data)?),
            "bash" => Self::Bash(Deserialize::deserialize(data)?),
            "compact" => Self::Compact(Deserialize::deserialize(data)?),
            "get_session_stats" => Self::Stats(Deserialize::deserialize(data)?),
            "new_session" | "switch_session" | "clone" => {
                Self::Cancelled(Deserialize::deserialize(data)?)
            }
            "fork" => Self::Fork(Deserialize::deserialize(data)?),
            "export_html" => Self::Path(Deserialize::deserialize(data)?),
            "get_fork_messages" => Self::ForkMessages(Deserialize::deserialize(data)?),
            "get_last_assistant_text" => Self::LastAssistantText(Deserialize::deserialize(data)?),
            "get_messages" => Self::Messages(Deserialize::deserialize(data)?),
            "get_commands" => Self::Commands(Deserialize::deserialize(data)?),
            other => {
                return Err(de::Error::custom(format!(
                    "command {other} answers no data"
                )));
            }
        })
    }
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

/// The members every session response carries.
#[derive(Deserialize)]
struct Envelope {
    /// The request's identifier.
    #[serde(default)]
    id: Option<String>,
    /// The command type.
    #[serde(default)]
    command: Option<String>,
    /// Whether the command succeeded.
    success: bool,
    /// The failure message.
    #[serde(default)]
    error: Option<String>,
}

impl<'de> Deserialize<'de> for RpcResponse {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(decoder)?;
        let envelope = Envelope::deserialize(&value).map_err(de::Error::custom)?;
        if envelope
            .command
            .as_deref()
            .is_some_and(names::<workspace::RpcCommand>)
        {
            let response = workspace::RpcResponse::deserialize(value).map_err(de::Error::custom)?;
            return Ok(Self::Workspace(Box::new(response)));
        }
        let Envelope {
            id,
            command,
            success,
            error,
        } = envelope;
        if !success {
            let error = error.ok_or_else(|| de::Error::missing_field("error"))?;
            return Ok(Self::Error { id, command, error });
        }
        let command = command.ok_or_else(|| de::Error::missing_field("command"))?;
        let data = match value.get("data") {
            None => RpcData::Empty,
            Some(data) => RpcData::decode(&command, data).map_err(de::Error::custom)?,
        };
        Ok(Self::Success { id, command, data })
    }
}
