#![doc = include_str!("../../../../../docs/models/content-generation.md")]

use std::iter;

use serde::Serialize;
use serde_json::Value;

use crate::arguments::json_parse::is_blank;
use crate::arguments::replay::nullish_default;
use crate::projection::IMAGE_PLACEHOLDER;
use crate::projection::normalize_tool_call_id;
use crate::{
    AssistantContent, AssistantMessage, Context, DiagnosticErrorInfo, ImageContent, Message, Model,
    ModelInput, StopReason, Tool, ToolResultMessage, UserBlock, UserContent, transform_messages,
};

pub(super) mod api_client;
pub(super) mod client_options;
pub(super) mod invocation;
#[cfg(not(target_arch = "wasm32"))]
mod node_auth;
mod projection;
mod request;
mod stream;
pub(super) mod thinking;
mod wire;

pub use api_client::{ApiClient, PreparedContentRequest};
pub use projection::{
    ProjectedContentRequest, project_content_request, project_vertex_content_request,
};
pub use request::{
    FunctionCallingConfig, GenerateContentConfig, GenerateContentParameters, ThinkingConfig,
    ToolConfig,
};
pub(crate) use request::{
    build_parameters, enabled_thinking, is_gemini_3_flash, is_gemini_3_pro, is_gemma_4,
};
pub use stream::process_google_stream;

/// Schema members that describe the schema itself rather than the described value.
const SCHEMA_METADATA_KEYS: [&str; 8] = [
    "$schema",
    "$id",
    "$anchor",
    "$dynamicAnchor",
    "$vocabulary",
    "$comment",
    "$defs",
    "definitions",
];
/// Every finish reason the service declares.
const FINISH_REASONS: [&str; 17] = [
    "STOP",
    "MAX_TOKENS",
    "BLOCKLIST",
    "PROHIBITED_CONTENT",
    "SPII",
    "SAFETY",
    "IMAGE_SAFETY",
    "IMAGE_PROHIBITED_CONTENT",
    "IMAGE_RECITATION",
    "IMAGE_OTHER",
    "RECITATION",
    "FINISH_REASON_UNSPECIFIED",
    "OTHER",
    "LANGUAGE",
    "MALFORMED_FUNCTION_CALL",
    "UNEXPECTED_TOOL_CALL",
    "NO_IMAGE",
];

/// Thinking level accepted by the newest model generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoogleThinkingLevel {
    /// The service picks the level.
    ThinkingLevelUnspecified,
    /// Least thinking.
    Minimal,
    /// Low thinking.
    Low,
    /// Medium thinking.
    Medium,
    /// Most thinking.
    High,
}

/// Author of a content turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentRole {
    /// The user, including tool results.
    User,
    /// The model.
    Model,
}

/// One turn of generated conversation content.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Content {
    /// Author of the turn.
    pub role: ContentRole,
    /// Parts of the turn, in order.
    pub parts: Vec<Part>,
}

/// One payload with the thought metadata that may accompany it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Part {
    /// The payload.
    #[serde(flatten)]
    pub data: PartData,
    /// Thought flag and replay signature.
    #[serde(flatten)]
    pub metadata: ThoughtMetadata,
}

/// Thought flag and replay signature of a part; either may accompany any payload.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThoughtMetadata {
    /// Whether the part is thinking content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<bool>,
    /// Opaque signature that lets the service resume its reasoning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

/// Payload of a part.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PartData {
    /// Plain text.
    Text(String),
    /// Image bytes.
    InlineData(Blob),
    /// A tool call made by the model.
    FunctionCall(FunctionCall),
    /// The result of a tool call.
    FunctionResponse(FunctionResponse),
}

/// Encoded image bytes.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blob {
    /// Base64 data.
    pub data: String,
    /// Media type.
    pub mime_type: String,
}

/// A tool call made by the model.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FunctionCall {
    /// Tool name.
    pub name: String,
    /// Arguments, in their supplied order.
    pub args: Value,
    /// Call identifier, present for model families that require one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// The result of a tool call.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FunctionResponse {
    /// Tool name.
    pub name: String,
    /// Output or error text.
    pub response: FunctionResponseResult,
    /// Images nested in the response, for model families that accept them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    /// Identifier of the answered call, present for model families that require one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Text of a tool result, filed under the key that tells success from failure.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FunctionResponseResult {
    /// Successful output.
    Output(String),
    /// Failure message.
    Error(String),
}

/// A tool offered to the model.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FunctionDeclaration {
    /// Tool name.
    pub name: String,
    /// What the tool does.
    pub description: String,
    /// The schema of the tool's arguments, in the field the chosen mode names.
    #[serde(flatten)]
    pub parameters: FunctionParameters,
}

/// The argument schema of a declaration, in exactly one of the service's two fields.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FunctionParameters {
    /// The restricted schema dialect, with schema metadata removed.
    Parameters(Value),
    /// The complete schema as supplied.
    ParametersJsonSchema(Value),
}

/// The declarations sent in one tool entry.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDeclarations {
    /// The declared tools, in their supplied order.
    pub function_declarations: Vec<FunctionDeclaration>,
}

/// How the model may use tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FunctionCallingConfigMode {
    /// The model decides.
    Auto,
    /// The model may not call tools.
    None,
    /// The model must call a tool.
    Any,
}

impl From<PartData> for Part {
    fn from(data: PartData) -> Self {
        Self {
            data,
            metadata: ThoughtMetadata::default(),
        }
    }
}

/// Report whether a part is thinking content; its signature never decides.
#[must_use]
pub fn is_thinking_part(part: &ThoughtMetadata) -> bool {
    part.thought == Some(true)
}

/// Take a nonempty incoming signature; otherwise keep the existing one, even when empty.
#[must_use]
pub fn retain_thought_signature<'a>(
    existing: Option<&'a str>,
    incoming: Option<&'a str>,
) -> Option<&'a str> {
    incoming
        .filter(|signature| !signature.is_empty())
        .or(existing)
}

/// Report whether the model family needs explicit tool-call identifiers.
#[must_use]
pub fn requires_tool_call_id(model_id: &str) -> bool {
    model_id.starts_with("claude-") || model_id.starts_with("gpt-oss-")
}

/// Convert the conversation into content turns for `model`.
#[must_use]
pub fn convert_messages(model: &Model, context: &Context) -> Vec<Content> {
    let with_ids = requires_tool_call_id(&model.id);
    let normalize = |id: &str, _: &Model, _: &AssistantMessage| {
        if with_ids {
            normalize_tool_call_id(id)
        } else {
            id.to_owned()
        }
    };
    let mut contents = Vec::new();
    for message in transform_messages(&context.messages, model, Some(&normalize)) {
        match message {
            Message::User(user) => contents.extend(user_content(user.content)),
            Message::Assistant(assistant) => contents.extend(model_content(assistant, with_ids)),
            Message::ToolResult(result) => push_result(&mut contents, result, model, with_ids),
        }
    }
    contents
}

/// Turn a user message into a turn; an empty block list yields none.
fn user_content(content: UserContent) -> Option<Content> {
    let parts: Vec<Part> = match content {
        UserContent::Text(text) => vec![PartData::Text(text).into()],
        UserContent::Blocks(blocks) => blocks.into_iter().map(user_part).collect(),
    };
    (!parts.is_empty()).then_some(Content {
        role: ContentRole::User,
        parts,
    })
}

/// Convert a user block, keeping text and images in place.
fn user_part(block: UserBlock) -> Part {
    match block {
        UserBlock::Text(text) => PartData::Text(text.text).into(),
        UserBlock::Image(image) => image_part(image),
    }
}

/// Convert an image block into inline data.
fn image_part(image: ImageContent) -> Part {
    PartData::InlineData(Blob {
        data: image.data,
        mime_type: image.mime_type,
    })
    .into()
}

/// Turn an assistant message into a model turn; a message without parts yields none.
fn model_content(assistant: AssistantMessage, with_ids: bool) -> Option<Content> {
    let parts: Vec<Part> = assistant
        .content
        .into_iter()
        .filter_map(|block| model_part(block, with_ids))
        .collect();
    (!parts.is_empty()).then_some(Content {
        role: ContentRole::Model,
        parts,
    })
}

/// Convert one assistant block, dropping blank text and thinking. The projection has already
/// turned retained thinking from other models into text, removed their signatures and
/// dropped their redacted thinking.
fn model_part(block: AssistantContent, with_ids: bool) -> Option<Part> {
    match block {
        AssistantContent::Text(text) if !is_blank(&text.text) => Some(Part {
            data: PartData::Text(text.text),
            metadata: replay(None, text.text_signature),
        }),
        AssistantContent::Thinking(thinking) if !is_blank(&thinking.thinking) => Some(Part {
            data: PartData::Text(thinking.thinking),
            metadata: replay(Some(true), thinking.thinking_signature),
        }),
        AssistantContent::ToolCall(call) => Some(Part {
            data: PartData::FunctionCall(FunctionCall {
                name: call.name,
                args: nullish_default(call.arguments),
                id: with_ids.then_some(call.id),
            }),
            metadata: replay(None, call.thought_signature),
        }),
        _ => None,
    }
}

/// Build thought metadata, keeping a signature only when it is shaped like base64.
fn replay(thought: Option<bool>, signature: Option<String>) -> ThoughtMetadata {
    ThoughtMetadata {
        thought,
        thought_signature: signature.filter(|signature| is_base64_shaped(signature)),
    }
}

/// Report whether `signature` is nonempty, a multiple of four long and made of base64
/// characters followed by at most two `=`; it is not decoded.
fn is_base64_shaped(signature: &str) -> bool {
    let body = signature.trim_end_matches('=');
    signature.len().is_multiple_of(4)
        && signature.len() - body.len() <= 2
        && !body.is_empty()
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}

/// Add a tool result to the turns, merging with the previous result turn.
fn push_result(
    contents: &mut Vec<Content>,
    result: ToolResultMessage,
    model: &Model,
    with_ids: bool,
) {
    let accepts_images = model.input.contains(&ModelInput::Image);
    let mut texts = Vec::new();
    let mut images = Vec::new();
    for block in result.content {
        match block {
            UserBlock::Text(text) => texts.push(text.text),
            UserBlock::Image(image) if accepts_images => images.push(image_part(image)),
            UserBlock::Image(_) => {}
        }
    }
    let text = texts.join("\n");
    let value = match (text.is_empty(), images.is_empty()) {
        (false, _) => text,
        (true, false) => IMAGE_PLACEHOLDER.to_owned(),
        (true, true) => String::new(),
    };
    let (nested, separate) = if accepts_nested_images(&model.id) {
        (images, Vec::new())
    } else {
        (Vec::new(), images)
    };
    let response = FunctionResponse {
        name: result.tool_name,
        response: if result.is_error {
            FunctionResponseResult::Error(value)
        } else {
            FunctionResponseResult::Output(value)
        },
        parts: nested,
        id: with_ids.then_some(result.tool_call_id),
    };
    let part = Part::from(PartData::FunctionResponse(response));
    match contents.last_mut() {
        Some(last) if holds_function_response(last) => last.parts.push(part),
        _ => contents.push(Content {
            role: ContentRole::User,
            parts: vec![part],
        }),
    }
    if !separate.is_empty() {
        let parts = iter::once(PartData::Text("Tool result image:".to_owned()).into())
            .chain(separate)
            .collect();
        contents.push(Content {
            role: ContentRole::User,
            parts,
        });
    }
}

/// Report whether a user turn already carries tool results.
fn holds_function_response(content: &Content) -> bool {
    content.role == ContentRole::User
        && content
            .parts
            .iter()
            .any(|part| matches!(part.data, PartData::FunctionResponse(_)))
}

/// Report whether tool results may nest images; only Gemini generations before 3 may not.
fn accepts_nested_images(model_id: &str) -> bool {
    let lowered = model_id.to_lowercase();
    let Some(rest) = lowered.strip_prefix("gemini") else {
        return true;
    };
    let rest = rest.strip_prefix("-live").unwrap_or(rest);
    let digits: String = rest
        .strip_prefix('-')
        .unwrap_or_default()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse::<u64>().map_or(true, |major| major >= 3)
}

/// Declare `tools`, or return `None` for an empty list. `use_parameters` selects the
/// restricted schema field, with schema metadata removed from every schema node.
#[must_use]
pub fn convert_tools(tools: &[Tool], use_parameters: bool) -> Option<Vec<FunctionDeclarations>> {
    if tools.is_empty() {
        return None;
    }
    let function_declarations = tools
        .iter()
        .map(|tool| FunctionDeclaration {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: if use_parameters {
                FunctionParameters::Parameters(without_metadata(&tool.parameters))
            } else {
                FunctionParameters::ParametersJsonSchema(tool.parameters.as_json().clone())
            },
        })
        .collect();
    Some(vec![FunctionDeclarations {
        function_declarations,
    }])
}

/// Copy a schema node without schema metadata members. Only keywords that hold schemas are
/// walked; names in property maps and literal values (`default`, `const`, `enum`, ...) are kept.
fn without_metadata(value: &Value) -> Value {
    match value {
        Value::Object(members) => Value::Object(
            members
                .iter()
                .filter(|(key, _)| !SCHEMA_METADATA_KEYS.contains(&key.as_str()))
                .map(|(key, member)| (key.clone(), keyword_value(key, member)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_metadata).collect()),
        other => other.clone(),
    }
}

/// Copy the value of one schema keyword, walking it only when it holds schemas.
fn keyword_value(keyword: &str, value: &Value) -> Value {
    match (keyword, value) {
        (
            "properties" | "patternProperties" | "dependentSchemas" | "dependencies",
            Value::Object(named),
        ) => Value::Object(
            named
                .iter()
                .map(|(name, schema)| (name.clone(), without_metadata(schema)))
                .collect(),
        ),
        (
            "items"
            | "prefixItems"
            | "additionalItems"
            | "additionalProperties"
            | "anyOf"
            | "oneOf"
            | "allOf"
            | "not"
            | "if"
            | "then"
            | "else"
            | "contains"
            | "propertyNames"
            | "unevaluatedItems"
            | "unevaluatedProperties"
            | "contentSchema",
            _,
        ) => without_metadata(value),
        _ => value.clone(),
    }
}

/// Map a tool-choice string; anything but `none` and `any` lets the model decide.
#[must_use]
pub fn map_tool_choice(choice: &str) -> FunctionCallingConfigMode {
    match choice {
        "none" => FunctionCallingConfigMode::None,
        "any" => FunctionCallingConfigMode::Any,
        _ => FunctionCallingConfigMode::Auto,
    }
}

/// Map a declared finish reason.
///
/// # Errors
/// Returns `Unhandled stop reason: <reason>` for a string the service does not declare.
pub fn map_stop_reason(reason: &str) -> Result<StopReason, DiagnosticErrorInfo> {
    if FINISH_REASONS.contains(&reason) {
        Ok(map_stop_reason_string(reason))
    } else {
        Err(DiagnosticErrorInfo::new(
            None,
            format!("Unhandled stop reason: {reason}"),
        ))
    }
}

/// Map a raw finish reason; every string but `STOP` and `MAX_TOKENS` is an error.
#[must_use]
pub fn map_stop_reason_string(reason: &str) -> StopReason {
    match reason {
        "STOP" => StopReason::Stop,
        "MAX_TOKENS" => StopReason::Length,
        _ => StopReason::Error,
    }
}
