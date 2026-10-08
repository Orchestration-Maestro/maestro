//! Typed conversation messages and their tagged content.
use super::{Api, JsonObject, Provider};
use crate::records::diagnostics::AssistantMessageDiagnostic;
use serde::{Deserialize, Serialize};
/// Represent version-one text signature metadata and its optional phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextPhase {
    /// Commentary.
    Commentary,
    /// `FinalAnswer`.
    #[serde(rename = "final_answer")]
    FinalAnswer,
}
/// Represent version-one text signature metadata and its optional phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSignatureV1 {
    /// V.
    #[serde(
        serialize_with = "serialize_version",
        deserialize_with = "deserialize_version"
    )]
    pub v: u8,
    /// Id.
    pub id: String,
    /// Phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<TextPhase>,
}
/// Carry tagged text with an optional opaque signature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "text")]
pub struct TextContent {
    /// Text.
    pub text: String,
    /// Text signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_signature: Option<String>,
}
/// Carry tagged reasoning, optional signature and redaction flag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "thinking")]
pub struct ThinkingContent {
    /// Thinking.
    pub thinking: String,
    /// Thinking signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    /// Redacted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted: Option<bool>,
}
/// Carry tagged base64 image data with its MIME type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "image")]
pub struct ImageContent {
    /// Data.
    pub data: String,
    /// Mime type.
    pub mime_type: String,
}
/// Carry a tagged tool invocation with open argument fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", rename = "toolCall")]
pub struct ToolCall {
    /// Id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Arguments.
    pub arguments: JsonObject,
    /// Thought signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}
/// Store the five reported cost categories without recomputation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageCost {
    /// Input.
    pub input: f64,
    /// Output.
    pub output: f64,
    /// Cache read.
    pub cache_read: f64,
    /// Cache write.
    pub cache_write: f64,
    /// Total.
    pub total: f64,
}
/// Store reported token counts and their supplied costs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Input.
    pub input: f64,
    /// Output.
    pub output: f64,
    /// Cache read.
    pub cache_read: f64,
    /// Cache write.
    pub cache_write: f64,
    /// Total tokens.
    pub total_tokens: f64,
    /// Cost.
    pub cost: UsageCost,
}
/// Represent successful, tool-use, error and abort outcomes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// Stop.
    Stop,
    /// Length.
    Length,
    /// `ToolUse`.
    ToolUse,
    /// Error.
    Error,
    /// Aborted.
    Aborted,
}
/// Represent user plain text or ordered text/image blocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    /// Text.
    Text(String),
    /// Blocks.
    Blocks(Vec<UserBlock>),
}
/// Represent user plain text or ordered text/image blocks.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type")]
pub enum UserBlock {
    /// Text.
    #[serde(rename = "text")]
    Text(TextContent),
    /// Image.
    #[serde(rename = "image")]
    Image(ImageContent),
}
/// Represent assistant text, thinking and tool-call blocks.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "type")]
pub enum AssistantContent {
    /// Text.
    #[serde(rename = "text")]
    Text(TextContent),
    /// Thinking.
    #[serde(rename = "thinking")]
    Thinking(ThinkingContent),
    /// `ToolCall`.
    #[serde(rename = "toolCall")]
    ToolCall(ToolCall),
}
/// Carry a whole tagged user message with its timestamp.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "role", rename = "user")]
pub struct UserMessage {
    /// Content.
    pub content: UserContent,
    /// Timestamp.
    pub timestamp: f64,
}
/// Carry a whole tagged assistant response, usage, outcome and diagnostics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "role", rename = "assistant")]
pub struct AssistantMessage {
    /// Content.
    pub content: Vec<AssistantContent>,
    /// Api.
    pub api: Api,
    /// Provider.
    pub provider: Provider,
    /// Model.
    pub model: String,
    /// Response model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_model: Option<String>,
    /// Response id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    /// Diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Vec<AssistantMessageDiagnostic>>,
    /// Usage.
    pub usage: Usage,
    /// Stop reason.
    pub stop_reason: StopReason,
    /// Error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Timestamp.
    pub timestamp: f64,
}
/// Carry a whole tagged tool result, content, details and error flag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "role", rename = "toolResult")]
#[serde(bound(deserialize = "TDetails: Deserialize<'de>"))]
pub struct ToolResultMessage<TDetails = serde_json::Value> {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Content.
    pub content: Vec<UserBlock>,
    /// Details.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::present"
    )]
    pub details: Option<TDetails>,
    /// Is error.
    pub is_error: bool,
    /// Timestamp.
    pub timestamp: f64,
}
/// Select one of the three conversation message roles.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    /// User.
    #[serde(rename = "user")]
    User(UserMessage),
    /// Assistant.
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    /// `ToolResult`.
    #[serde(rename = "toolResult")]
    ToolResult(ToolResultMessage),
}
/// Describe a model-facing tool and its parameter schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool<TParameters = serde_json::Value> {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Parameters.
    pub parameters: TParameters,
}
/// Supply conversation messages with optional system prompt and tools.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// System prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Messages.
    pub messages: Vec<Message>,
    /// Tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

/// Decode and require the supported text-signature version.
fn deserialize_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let version = u8::deserialize(deserializer)?;
    if version == 1 {
        Ok(version)
    } else {
        Err(serde::de::Error::custom("text signature version must be 1"))
    }
}
/// Serialize a text-signature version only when it is supported.
fn serialize_version<S: serde::Serializer>(
    version: impl std::borrow::Borrow<u8>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    if *version.borrow() == 1 {
        serializer.serialize_u8(*version.borrow())
    } else {
        Err(serde::ser::Error::custom(
            "text signature version must be 1",
        ))
    }
}

impl Serialize for UserBlock {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(value) => value.serialize(serializer),
            Self::Image(value) => value.serialize(serializer),
        }
    }
}

impl Serialize for AssistantContent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Text(value) => value.serialize(serializer),
            Self::Thinking(value) => value.serialize(serializer),
            Self::ToolCall(value) => value.serialize(serializer),
        }
    }
}

impl Serialize for Message {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::User(value) => value.serialize(serializer),
            Self::Assistant(value) => value.serialize(serializer),
            Self::ToolResult(value) => value.serialize(serializer),
        }
    }
}
