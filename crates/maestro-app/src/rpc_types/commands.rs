//! Command records of both families on one `type` tag, and the decoder that routes a record
//! to the family naming its type.
use super::workspace;
use maestro_agent::QueueMode;
use maestro_models::{ImageContent, ModelThinkingLevel};
use maestro_session::messages::UserMessageDelivery;
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Value;

/// The session commands, tagged by `type`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RpcSessionCommand {
    /// Sends a prompt; it is answered once its preflight succeeds.
    #[serde(rename = "prompt", rename_all = "camelCase")]
    Prompt {
        /// The prompt text.
        message: String,
        /// Images sent after the text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        images: Option<Vec<ImageContent>>,
        /// How the prompt is queued while a response streams.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        streaming_behavior: Option<UserMessageDelivery>,
    },
    /// Queues a steering message.
    #[serde(rename = "steer")]
    Steer {
        /// The message text.
        message: String,
        /// Images sent after the text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        images: Option<Vec<ImageContent>>,
    },
    /// Queues a follow-up message.
    #[serde(rename = "follow_up")]
    FollowUp {
        /// The message text.
        message: String,
        /// Images sent after the text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        images: Option<Vec<ImageContent>>,
    },
    /// Stops the running turn and waits until the agent is idle.
    #[serde(rename = "abort")]
    Abort,
    /// Starts a new session.
    #[serde(rename = "new_session", rename_all = "camelCase")]
    NewSession {
        /// The session file recorded as the new session's parent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_session: Option<String>,
    },
    /// Reads the session state.
    #[serde(rename = "get_state")]
    GetState,
    /// Selects an available model.
    #[serde(rename = "set_model", rename_all = "camelCase")]
    SetModel {
        /// The model's provider.
        provider: String,
        /// The model's identifier.
        model_id: String,
    },
    /// Selects the next model.
    #[serde(rename = "cycle_model")]
    CycleModel,
    /// Lists the available models.
    #[serde(rename = "get_available_models")]
    GetAvailableModels,
    /// Sets the thinking level.
    #[serde(rename = "set_thinking_level")]
    SetThinkingLevel {
        /// The requested level.
        level: ModelThinkingLevel,
    },
    /// Selects the next thinking level.
    #[serde(rename = "cycle_thinking_level")]
    CycleThinkingLevel,
    /// Sets how queued steering messages are delivered.
    #[serde(rename = "set_steering_mode")]
    SetSteeringMode {
        /// The delivery policy.
        mode: QueueMode,
    },
    /// Sets how queued follow-up messages are delivered.
    #[serde(rename = "set_follow_up_mode")]
    SetFollowUpMode {
        /// The delivery policy.
        mode: QueueMode,
    },
    /// Compacts the session context.
    #[serde(rename = "compact", rename_all = "camelCase")]
    Compact {
        /// Instructions for the summary.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        custom_instructions: Option<String>,
    },
    /// Turns automatic compaction on or off.
    #[serde(rename = "set_auto_compaction")]
    SetAutoCompaction {
        /// Whether it is on.
        enabled: bool,
    },
    /// Turns automatic retry on or off.
    #[serde(rename = "set_auto_retry")]
    SetAutoRetry {
        /// Whether it is on.
        enabled: bool,
    },
    /// Stops a pending retry.
    #[serde(rename = "abort_retry")]
    AbortRetry,
    /// Runs a shell command and records its output.
    #[serde(rename = "bash")]
    Bash {
        /// The command line.
        command: String,
    },
    /// Stops the running shell command.
    #[serde(rename = "abort_bash")]
    AbortBash,
    /// Reads the session statistics.
    #[serde(rename = "get_session_stats")]
    GetSessionStats,
    /// Writes the session as an HTML document.
    #[serde(rename = "export_html", rename_all = "camelCase")]
    ExportHtml {
        /// Where to write it; a default name when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output_path: Option<String>,
    },
    /// Resumes another session file.
    #[serde(rename = "switch_session", rename_all = "camelCase")]
    SwitchSession {
        /// The session file.
        session_path: String,
    },
    /// Forks the session from an entry.
    #[serde(rename = "fork", rename_all = "camelCase")]
    Fork {
        /// The entry the fork starts from.
        entry_id: String,
    },
    /// Duplicates the session at its current entry.
    #[serde(rename = "clone")]
    Clone,
    /// Lists the user messages the session can be forked from.
    #[serde(rename = "get_fork_messages")]
    GetForkMessages,
    /// Reads the last assistant text.
    #[serde(rename = "get_last_assistant_text")]
    GetLastAssistantText,
    /// Names the session.
    #[serde(rename = "set_session_name")]
    SetSessionName {
        /// The name, trimmed before use.
        name: String,
    },
    /// Reads the session messages.
    #[serde(rename = "get_messages")]
    GetMessages,
    /// Lists the commands a prompt can invoke.
    #[serde(rename = "get_commands")]
    GetCommands,
}

/// A command of either family; both carry their type in the same top-level `type` member.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum RpcCommandBody {
    /// A session command.
    Session(RpcSessionCommand),
    /// A workspace command.
    Workspace(Box<workspace::RpcCommand>),
}

/// A command with its optional correlation identifier.
pub type RpcCommand = workspace::Request<RpcCommandBody>;

/// Why a decoded JSON value is not a command.
#[derive(Debug)]
pub enum RpcCommandRejection {
    /// A type one family names, with members that do not decode.
    Invalid {
        /// The supplied type.
        command: String,
        /// The string identifier the record supplied, when it did.
        id: Option<String>,
        /// The decoder's failure.
        error: serde_json::Error,
    },
    /// A string type neither family names.
    Unknown(String),
    /// Not an object, or an object without a string type.
    Untyped(serde_json::Error),
}

impl std::fmt::Display for RpcCommandRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid { error, .. } | Self::Untyped(error) => error.fmt(formatter),
            Self::Unknown(command) => write!(formatter, "Unknown command: {command}"),
        }
    }
}

impl std::error::Error for RpcCommandRejection {}

/// Decodes `value` as a command of the family that names its type.
///
/// # Errors
/// Returns the rejection: a named type with members that do not decode, an unnamed type, or
/// a value without a string type.
pub fn parse_rpc_command(value: &Value) -> Result<RpcCommand, RpcCommandRejection> {
    let command = RpcCommandBody::route(value)?;
    let id = value
        .get("id")
        .map_or(Ok(None), Option::<String>::deserialize)
        .map_err(|error| RpcCommandRejection::Invalid {
            command: value["type"].as_str().unwrap_or_default().to_owned(),
            id: None,
            error,
        })?;
    Ok(RpcCommand { id, command })
}

impl RpcCommandBody {
    /// Decodes `value` with the family that names its type.
    fn route(value: &Value) -> Result<Self, RpcCommandRejection> {
        let Some(kind) = value.get("type").and_then(Value::as_str) else {
            return RpcSessionCommand::deserialize(value)
                .map(Self::Session)
                .map_err(RpcCommandRejection::Untyped);
        };
        let invalid = |error| RpcCommandRejection::Invalid {
            command: kind.to_owned(),
            id: value.get("id").and_then(Value::as_str).map(str::to_owned),
            error,
        };
        if names::<RpcSessionCommand>(kind) {
            RpcSessionCommand::deserialize(value)
                .map(Self::Session)
                .map_err(invalid)
        } else if names::<workspace::RpcCommand>(kind) {
            workspace::RpcCommand::deserialize(value)
                .map(|command| Self::Workspace(Box::new(command)))
                .map_err(invalid)
        } else {
            Err(RpcCommandRejection::Unknown(kind.to_owned()))
        }
    }
}

impl<'de> Deserialize<'de> for RpcCommandBody {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        Self::route(&Value::deserialize(decoder)?).map_err(de::Error::custom)
    }
}

/// Whether the type-tagged family `T` names `kind`, asked of `T`'s own decoder.
pub(crate) fn names<T: de::DeserializeOwned>(kind: &str) -> bool {
    let tag = de::value::MapDeserializer::<_, NameProbe>::new(std::iter::once(("type", kind)));
    !matches!(T::deserialize(tag), Err(NameProbe::UnknownVariant))
}

/// The outcome of decoding a lone type tag: unknown, or any other failure.
#[derive(Debug)]
enum NameProbe {
    /// The decoder reported the tag as an unknown variant.
    UnknownVariant,
    /// Any other failure, such as a missing member of a named variant.
    Other,
}

impl std::fmt::Display for NameProbe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownVariant => "unknown variant",
            Self::Other => "other failure",
        })
    }
}

impl std::error::Error for NameProbe {}

impl de::Error for NameProbe {
    fn custom<T: std::fmt::Display>(_message: T) -> Self {
        Self::Other
    }

    fn unknown_variant(_variant: &str, _expected: &'static [&'static str]) -> Self {
        Self::UnknownVariant
    }
}
