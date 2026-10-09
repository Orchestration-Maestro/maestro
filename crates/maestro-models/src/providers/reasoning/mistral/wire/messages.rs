//! Typed messages transport records.

use super::content::{MessageContent, SystemMessageContent};
use super::{Nullable, Object, SafeInteger, literal, nullable, optional, required_nullable};
use serde::{Deserialize, Serialize};

/// Transport fields for `AssistantMessage`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct AssistantMessage {
    /// The `role` field.
    #[serde(deserialize_with = "literal")]
    role: AssistantMessageTag,
    /// The `content` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    content: Option<Nullable<MessageContent>>,
    /// The `toolCalls` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tool_calls: Option<Nullable<Vec<Object<ToolCall>>>>,
    /// The `prefix` field.
    #[serde(default)]
    prefix: bool,
}

/// Transport fields for `SystemMessage`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct SystemMessage {
    /// The `role` field.
    #[serde(deserialize_with = "literal")]
    role: SystemMessageTag,
    /// The `content` field.
    content: SystemMessageContent,
}

/// Transport fields for `UserMessage`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct UserMessage {
    /// The `role` field.
    #[serde(deserialize_with = "literal")]
    role: UserMessageTag,
    /// The `content` field.
    #[serde(deserialize_with = "required_nullable")]
    content: Option<MessageContent>,
}

/// Transport fields for `ToolMessage`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ToolMessage {
    /// The `role` field.
    #[serde(deserialize_with = "literal")]
    role: ToolMessageTag,
    /// The `content` field.
    #[serde(deserialize_with = "required_nullable")]
    content: Option<MessageContent>,
    /// The `toolCallId` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tool_call_id: Option<Nullable<String>>,
    /// The `name` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    name: Option<Nullable<String>>,
}

/// Transport fields for `ToolCall`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ToolCall {
    /// The `id` field.
    #[serde(default = "tool_call_id_default")]
    id: String,
    /// The `type` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none",
        rename = "type"
    )]
    kind: Option<String>,
    /// The `function` field.
    function: Object<FunctionCall>,
    /// The `index` field.
    #[serde(default = "tool_call_index_default")]
    index: SafeInteger,
}

/// Transport fields for `FunctionCall`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct FunctionCall {
    /// The `name` field.
    name: String,
    /// The `arguments` field.
    arguments: Arguments,
}

/// Literal `assistant` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum AssistantMessageTag {
    /// Transport discriminator.
    #[serde(rename = "assistant")]
    Value,
}

/// Literal `system` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum SystemMessageTag {
    /// Transport discriminator.
    #[serde(rename = "system")]
    Value,
}

/// Literal `user` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum UserMessageTag {
    /// Transport discriminator.
    #[serde(rename = "user")]
    Value,
}

/// Literal `tool` tag.
#[derive(Deserialize, Serialize)]
pub(super) enum ToolMessageTag {
    /// Transport discriminator.
    #[serde(rename = "tool")]
    Value,
}

/// The declared field default.
fn tool_call_id_default() -> String {
    "null".to_owned()
}

/// The declared field default.
fn tool_call_index_default() -> SafeInteger {
    SafeInteger(serde_json::Number::from(0))
}

/// A conversation message with a required role discriminator.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum Message {
    /// Assistant replay.
    Assistant(Object<AssistantMessage>),
    /// System instruction.
    System(Object<SystemMessage>),
    /// Tool result.
    Tool(Object<ToolMessage>),
    /// User input.
    User(Object<UserMessage>),
}

/// Function arguments accept dictionaries decoded by `serde_json` or authored strings.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum Arguments {
    /// Dictionary members in the shared JSON representation.
    Object(serde_json::Map<String, serde_json::Value>),
    /// Authored argument text.
    String(String),
}
