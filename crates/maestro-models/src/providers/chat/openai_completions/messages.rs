//! Typed chat-completion wire messages and the conversion from conversation history.

use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Serialize, Serializer};
use serde_json::Value;

use super::compat::{OpenAICompletionsCapability as Capability, ResolvedOpenAICompletionsCompat};
use crate::arguments::json_parse::whitespace;
use crate::providers::json_text::{compact_object, is_truthy};
use crate::{
    AssistantContent, AssistantMessage, Context, DiagnosticErrorInfo, JsonObject, Message, Model,
    ModelInput, ThinkingContent, ToolCall, ToolResultMessage, UserBlock, UserContent, UserMessage,
    transform_messages,
};

/// Marker that asks the endpoint to cache everything up to the marked part.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenAICompatCacheControl {
    /// Cache lifetime, such as `1h`; absent for the endpoint default.
    pub ttl: Option<String>,
}

impl Serialize for OpenAICompatCacheControl {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut marker = serializer.serialize_struct("OpenAICompatCacheControl", 2)?;
        marker.serialize_field("type", "ephemeral")?;
        if let Some(ttl) = &self.ttl {
            marker.serialize_field("ttl", ttl)?;
        }
        marker.end()
    }
}

/// Message content: plain text or an ordered list of parts.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ChatCompletionContent {
    /// Plain text.
    Text(String),
    /// Text and image parts.
    Parts(Vec<ChatCompletionContentPart>),
}

/// One part of structured message content.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ChatCompletionContentPart {
    /// A text part.
    #[serde(rename = "text")]
    Text(ChatCompletionContentPartText),
    /// An image part.
    #[serde(rename = "image_url")]
    Image(ChatCompletionContentPartImage),
}

/// A text part with an optional cache marker.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChatCompletionContentPartText {
    /// The text.
    pub text: String,
    /// Marker placed on this part.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<OpenAICompatCacheControl>,
}

/// An image part addressed by URL.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatCompletionContentPartImage {
    /// Image URL, usually a base64 data URL.
    pub url: String,
}

impl Serialize for ChatCompletionContentPartImage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut part = serializer.serialize_struct("ChatCompletionContentPartImage", 1)?;
        part.serialize_field("image_url", &ImageUrl { url: &self.url })?;
        part.end()
    }
}

/// Nested `image_url` object of an image part.
#[derive(Serialize)]
struct ImageUrl<'a> {
    /// Image URL.
    url: &'a str,
}

/// A function call requested by the assistant.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatCompletionMessageToolCall {
    /// Call identifier.
    pub id: String,
    /// Function name.
    pub name: String,
    /// Arguments as compact JSON text.
    pub arguments: String,
}

impl Serialize for ChatCompletionMessageToolCall {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut call = serializer.serialize_map(Some(3))?;
        call.serialize_entry("id", &self.id)?;
        call.serialize_entry("type", "function")?;
        call.serialize_entry(
            "function",
            &FunctionCall {
                name: &self.name,
                arguments: &self.arguments,
            },
        )?;
        call.end()
    }
}

/// Nested `function` object of a tool call.
#[derive(Serialize)]
struct FunctionCall<'a> {
    /// Function name.
    name: &'a str,
    /// Arguments as JSON text.
    arguments: &'a str,
}

/// One message of the request history.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatCompletionMessageParam {
    /// Instructions for endpoints without a developer role.
    System {
        /// Instruction content.
        content: ChatCompletionContent,
    },
    /// Instructions for reasoning models.
    Developer {
        /// Instruction content.
        content: ChatCompletionContent,
    },
    /// A user turn.
    User {
        /// User content.
        content: ChatCompletionContent,
    },
    /// An assistant turn.
    Assistant {
        /// Assistant content: text or parts when the turn has any, an empty string for tool-only
        /// turns of endpoints that require an assistant message after tool results, else `null`.
        content: Option<ChatCompletionContent>,
        /// Requested function calls.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ChatCompletionMessageToolCall>,
        /// Reasoning replay fields: the field that carried earlier reasoning, `reasoning_details`
        /// and `reasoning_content`, each only where it applies.
        #[serde(flatten)]
        extension_fields: JsonObject,
    },
    /// A tool result.
    Tool {
        /// Result text.
        content: String,
        /// Identifier of the call answered.
        tool_call_id: String,
        /// Tool name, for endpoints that require it.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

/// Text sent between tool results and the next user message when an endpoint requires it.
const BRIDGE_TEXT: &str = "I have processed the tool results.";
/// Placeholder for a tool result without text.
const IMAGE_PLACEHOLDER: &str = "(see attached image)";
/// Lead-in for images lifted out of tool results.
const IMAGE_INTRODUCTION: &str = "Attached image(s) from tool result:";
/// Longest tool-call identifier the protocol accepts, in UTF-16 units.
const ID_LIMIT: usize = 40;

/// Project the history for the model and convert it into wire messages.
///
/// # Errors
/// Returns the failure to serialize a tool call's arguments.
pub fn convert_messages(
    model: &Model,
    context: &Context,
    compat: &ResolvedOpenAICompletionsCompat,
) -> Result<Vec<ChatCompletionMessageParam>, DiagnosticErrorInfo> {
    let normalize =
        |id: &str, _: &Model, _: &AssistantMessage| normalize_tool_call_id(id, &model.provider);
    let history = transform_messages(&context.messages, model, Some(&normalize));
    let mut params = Vec::with_capacity(history.len() + 1);
    if let Some(prompt) = context.system_prompt.as_deref().filter(|p| !p.is_empty()) {
        let instructions = ChatCompletionContent::Text(prompt.to_owned());
        params.push(
            if model.reasoning && compat.has(Capability::SupportsDeveloperRole) {
                ChatCompletionMessageParam::Developer {
                    content: instructions,
                }
            } else {
                ChatCompletionMessageParam::System {
                    content: instructions,
                }
            },
        );
    }
    let mut after_tool_result = false;
    let mut remaining = history.iter().peekable();
    while let Some(message) = remaining.next() {
        let is_user = matches!(message, Message::User(_));
        if after_tool_result && is_user && compat.has(Capability::RequiresAssistantAfterToolResult)
        {
            params.push(bridge());
        }
        match message {
            Message::User(user) => {
                let converted = convert_user(user);
                after_tool_result = after_tool_result && converted.is_none();
                params.extend(converted);
            }
            Message::Assistant(assistant) => {
                let converted = convert_assistant(assistant, model, compat)?;
                after_tool_result = after_tool_result && converted.is_none();
                params.extend(converted);
            }
            Message::ToolResult(first) => {
                let mut results = vec![first];
                while let Some(Message::ToolResult(next)) =
                    remaining.next_if(|m| matches!(m, Message::ToolResult(_)))
                {
                    results.push(next);
                }
                after_tool_result = convert_tool_results(&results, model, compat, &mut params);
            }
        }
    }
    Ok(params)
}

/// Build the assistant message that separates tool results from a user message.
fn bridge() -> ChatCompletionMessageParam {
    ChatCompletionMessageParam::Assistant {
        content: Some(ChatCompletionContent::Text(BRIDGE_TEXT.to_owned())),
        tool_calls: Vec::new(),
        extension_fields: JsonObject::new(),
    }
}

/// An identifier containing `|` is cut there, every character outside `[A-Za-z0-9_-]` becomes
/// one `_` per UTF-16 unit and the result is limited to 40 units. Any other identifier is kept,
/// except that the `openai` provider limits it to 40 units at a character boundary.
fn normalize_tool_call_id(id: &str, provider: &str) -> String {
    if let Some((call_id, _)) = id.split_once('|') {
        return call_id
            .chars()
            .flat_map(|character| {
                let kept = character.is_ascii_alphanumeric() || matches!(character, '_' | '-');
                let units = if kept { 1 } else { character.len_utf16() };
                std::iter::repeat_n(if kept { character } else { '_' }, units)
            })
            .take(ID_LIMIT)
            .collect();
    }
    if provider == "openai" {
        return truncate_utf16(id, ID_LIMIT);
    }
    id.to_owned()
}

/// Keep the longest prefix of whole characters that fits in `limit` UTF-16 units.
fn truncate_utf16(text: &str, limit: usize) -> String {
    let mut units = 0;
    text.chars()
        .take_while(|character| {
            units += character.len_utf16();
            units <= limit
        })
        .collect()
}

/// Convert a user turn; an empty block list produces no message.
fn convert_user(user: &UserMessage) -> Option<ChatCompletionMessageParam> {
    match &user.content {
        UserContent::Text(text) => Some(ChatCompletionContent::Text(text.clone())),
        UserContent::Blocks(blocks) if blocks.is_empty() => None,
        UserContent::Blocks(blocks) => Some(ChatCompletionContent::Parts(
            blocks.iter().map(user_part).collect(),
        )),
    }
    .map(|content| ChatCompletionMessageParam::User { content })
}

/// Convert one user block into a content part.
fn user_part(block: &UserBlock) -> ChatCompletionContentPart {
    match block {
        UserBlock::Text(text) => text_part(&text.text),
        UserBlock::Image(image) => {
            ChatCompletionContentPart::Image(ChatCompletionContentPartImage {
                url: format!("data:{};base64,{}", image.mime_type, image.data),
            })
        }
    }
}

/// Build a text part without a cache marker.
fn text_part(text: impl Into<String>) -> ChatCompletionContentPart {
    ChatCompletionContentPart::Text(ChatCompletionContentPartText {
        text: text.into(),
        cache_control: None,
    })
}

/// Report whether trimming leaves any text.
fn has_text(text: &str) -> bool {
    !text.trim_matches(whitespace).is_empty()
}

/// The blocks of an assistant turn that the wire format carries.
struct AssistantBlocks<'a> {
    /// Text blocks with visible characters.
    texts: Vec<&'a str>,
    /// Reasoning blocks with visible characters.
    thinking: Vec<&'a ThinkingContent>,
    /// Requested calls.
    calls: Vec<&'a ToolCall>,
}

impl<'a> AssistantBlocks<'a> {
    /// Sort the blocks of a turn by kind, dropping blank text and reasoning.
    fn of(assistant: &'a AssistantMessage) -> Self {
        let mut blocks = Self {
            texts: Vec::new(),
            thinking: Vec::new(),
            calls: Vec::new(),
        };
        for block in &assistant.content {
            match block {
                AssistantContent::Text(text) if has_text(&text.text) => {
                    blocks.texts.push(&text.text);
                }
                AssistantContent::Thinking(thinking) if has_text(&thinking.thinking) => {
                    blocks.thinking.push(thinking);
                }
                AssistantContent::ToolCall(call) => blocks.calls.push(call),
                AssistantContent::Text(_) | AssistantContent::Thinking(_) => {}
            }
        }
        blocks
    }
}

/// Convert an assistant turn; a turn without content and tool calls produces no message.
fn convert_assistant(
    assistant: &AssistantMessage,
    model: &Model,
    compat: &ResolvedOpenAICompletionsCompat,
) -> Result<Option<ChatCompletionMessageParam>, DiagnosticErrorInfo> {
    let blocks = AssistantBlocks::of(assistant);
    let (content, mut extension_fields) = replay_content(&blocks, compat);
    let tool_calls = blocks
        .calls
        .iter()
        .map(|call| wire_tool_call(call))
        .collect::<Result<Vec<_>, _>>()?;
    let details: Vec<Value> = blocks
        .calls
        .iter()
        .filter_map(|call| call.thought_signature.as_deref())
        .filter_map(|signature| serde_json::from_str::<Value>(signature).ok())
        .filter(is_truthy)
        .collect();
    if !details.is_empty() {
        extension_fields.insert("reasoning_details".to_owned(), Value::Array(details));
    }
    if compat.has(Capability::RequiresReasoningContentOnAssistantMessages)
        && model.reasoning
        && !extension_fields.contains_key("reasoning_content")
    {
        extension_fields.insert("reasoning_content".to_owned(), Value::String(String::new()));
    }
    let has_content = match &content {
        Some(ChatCompletionContent::Text(text)) => !text.is_empty(),
        Some(ChatCompletionContent::Parts(parts)) => !parts.is_empty(),
        None => false,
    };
    Ok(
        (has_content || !tool_calls.is_empty()).then_some(ChatCompletionMessageParam::Assistant {
            content,
            tool_calls,
            extension_fields,
        }),
    )
}

/// Choose the assistant content and the reasoning replay fields for the endpoint.
fn replay_content(
    blocks: &AssistantBlocks<'_>,
    compat: &ResolvedOpenAICompletionsCompat,
) -> (Option<ChatCompletionContent>, JsonObject) {
    let mut content = compat
        .has(Capability::RequiresAssistantAfterToolResult)
        .then(|| ChatCompletionContent::Text(String::new()));
    let mut fields = JsonObject::new();
    let reasoning: Vec<&str> = blocks
        .thinking
        .iter()
        .map(|t| t.thinking.as_str())
        .collect();
    if !blocks.thinking.is_empty() && compat.has(Capability::RequiresThinkingAsText) {
        let parts = std::iter::once(text_part(reasoning.join("\n\n")))
            .chain(blocks.texts.iter().map(|text| text_part(*text)))
            .collect();
        return (Some(ChatCompletionContent::Parts(parts)), fields);
    }
    let text = blocks.texts.concat();
    if !text.is_empty() {
        content = Some(ChatCompletionContent::Text(text));
    }
    let signature = blocks
        .thinking
        .first()
        .and_then(|first| first.thinking_signature.as_deref())
        .filter(|signature| !signature.is_empty());
    if let Some(signature) = signature {
        fields.insert(signature.to_owned(), Value::String(reasoning.join("\n")));
    }
    (content, fields)
}

/// Convert a tool call, rendering its arguments as compact JSON text.
fn wire_tool_call(call: &ToolCall) -> Result<ChatCompletionMessageToolCall, DiagnosticErrorInfo> {
    let arguments = compact_object(&call.arguments).map_err(|error| DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: error.to_string(),
        stack: None,
        code: None,
    })?;
    Ok(ChatCompletionMessageToolCall {
        id: call.id.clone(),
        name: call.name.clone(),
        arguments,
    })
}

/// Convert a run of consecutive tool results; returns whether the last message was a tool result.
fn convert_tool_results(
    results: &[&ToolResultMessage],
    model: &Model,
    compat: &ResolvedOpenAICompletionsCompat,
    params: &mut Vec<ChatCompletionMessageParam>,
) -> bool {
    let mut images = Vec::new();
    for result in results {
        let text = result
            .content
            .iter()
            .filter_map(|block| match block {
                UserBlock::Text(text) => Some(text.text.as_str()),
                UserBlock::Image(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let name = (compat.has(Capability::RequiresToolResultName) && !result.tool_name.is_empty())
            .then(|| result.tool_name.clone());
        params.push(ChatCompletionMessageParam::Tool {
            content: if text.is_empty() {
                IMAGE_PLACEHOLDER.to_owned()
            } else {
                text
            },
            tool_call_id: result.tool_call_id.clone(),
            name,
        });
        if model.input.contains(&ModelInput::Image) {
            images.extend(
                result
                    .content
                    .iter()
                    .filter(|block| matches!(block, UserBlock::Image(_)))
                    .map(user_part),
            );
        }
    }
    if images.is_empty() {
        return true;
    }
    if compat.has(Capability::RequiresAssistantAfterToolResult) {
        params.push(bridge());
    }
    let parts = std::iter::once(text_part(IMAGE_INTRODUCTION))
        .chain(images)
        .collect();
    params.push(ChatCompletionMessageParam::User {
        content: ChatCompletionContent::Parts(parts),
    });
    false
}
