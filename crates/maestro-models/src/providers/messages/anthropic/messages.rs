//! Typed wire messages, content blocks and tools, and the conversion from conversation history.

use std::borrow::Cow;
use std::iter::Peekable;

use serde::Serialize;
use serde_json::Value;

use crate::arguments::json_parse::whitespace;
use crate::{
    AssistantContent, AssistantMessage, Context, JsonObject, Message, Model, Tool,
    ToolResultMessage, UserBlock, UserContent, transform_messages,
};

/// Placeholder for a tool result that holds only images.
const IMAGE_PLACEHOLDER: &str = "(see attached image)";
/// Longest tool-call identifier the protocol accepts, in characters.
const ID_LIMIT: usize = 64;

/// Marker that asks the service to cache everything up to the marked block.
#[derive(Clone, Copy, Serialize)]
pub(super) struct CacheControl {
    /// Constant `ephemeral`.
    r#type: &'static str,
    /// Cache lifetime; absent for the service default.
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<&'static str>,
}

impl CacheControl {
    /// The default lifetime.
    pub(super) const SHORT: Self = Self {
        r#type: "ephemeral",
        ttl: None,
    };
    /// A one-hour lifetime.
    pub(super) const LONG: Self = Self {
        r#type: "ephemeral",
        ttl: Some("1h"),
    };
}

/// Who a message is from.
#[derive(Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Role {
    /// The user, including tool results.
    User,
    /// The model.
    Assistant,
}

/// One message of the request history.
#[derive(Serialize)]
pub(super) struct WireMessage {
    /// Author.
    role: Role,
    /// Content.
    content: Content,
}

/// Message content: plain text or blocks.
#[derive(Serialize)]
#[serde(untagged)]
enum Content {
    /// Plain text.
    Text(String),
    /// Ordered blocks.
    Blocks(Vec<Block>),
}

/// The bytes of an image.
#[derive(Serialize)]
struct ImageSource {
    /// Constant `base64`.
    r#type: &'static str,
    /// Image type.
    media_type: String,
    /// Base64 data.
    data: String,
}

/// One content block.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Block {
    /// Text.
    Text {
        /// The text.
        text: String,
        /// Marker placed on this block.
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// An image.
    Image {
        /// The bytes.
        source: ImageSource,
        /// Marker placed on this block.
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Signed reasoning.
    Thinking {
        /// The reasoning.
        thinking: String,
        /// Signature that lets the service accept the reasoning back.
        signature: String,
    },
    /// Reasoning the service withheld.
    RedactedThinking {
        /// Opaque payload.
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<String>,
    },
    /// A tool call.
    ToolUse {
        /// Call identifier.
        id: String,
        /// Tool name.
        name: String,
        /// Arguments.
        input: JsonObject,
    },
    /// A tool result.
    ToolResult {
        /// Identifier of the call answered.
        tool_use_id: String,
        /// Result content.
        content: ResultContent,
        /// Whether the call failed.
        is_error: bool,
        /// Marker placed on this block.
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

impl Block {
    /// A text block without a marker.
    fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            cache_control: None,
        }
    }

    /// Place a marker on a block that can carry one.
    fn mark(&mut self, marker: CacheControl) {
        if let Self::Text { cache_control, .. }
        | Self::Image { cache_control, .. }
        | Self::ToolResult { cache_control, .. } = self
        {
            *cache_control = Some(marker);
        }
    }
}

/// The content of a tool result: text, or blocks when it holds an image.
#[derive(Serialize)]
#[serde(untagged)]
enum ResultContent {
    /// The text blocks joined by line feeds.
    Text(String),
    /// Text and image blocks in order.
    Blocks(Vec<Block>),
}

/// Convert a block a user or tool wrote.
fn user_block(block: &UserBlock) -> Block {
    match block {
        UserBlock::Text(text) => Block::text(&text.text),
        UserBlock::Image(image) => Block::Image {
            source: ImageSource {
                r#type: "base64",
                media_type: image.mime_type.clone(),
                data: image.data.clone(),
            },
            cache_control: None,
        },
    }
}

/// Convert the content of a tool result.
fn result_content(blocks: &[UserBlock]) -> ResultContent {
    let texts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| match block {
            UserBlock::Text(text) => Some(text.text.as_str()),
            UserBlock::Image(_) => None,
        })
        .collect();
    if texts.len() == blocks.len() {
        return ResultContent::Text(texts.join("\n"));
    }
    let mut converted: Vec<Block> = blocks.iter().map(user_block).collect();
    if !converted
        .iter()
        .any(|block| matches!(block, Block::Text { .. }))
    {
        converted.insert(0, Block::text(IMAGE_PLACEHOLDER));
    }
    ResultContent::Blocks(converted)
}

/// Report whether trimming leaves any text.
fn has_text(text: &str) -> bool {
    !text.trim_matches(whitespace).is_empty()
}

/// Every character outside `[A-Za-z0-9_-]` becomes one `_` per UTF-16 unit, and the result is
/// limited to 64 characters.
fn normalize_tool_call_id(id: &str) -> String {
    id.chars()
        .flat_map(|character| {
            let kept = character.is_ascii_alphanumeric() || matches!(character, '_' | '-');
            let units = if kept { 1 } else { character.len_utf16() };
            std::iter::repeat_n(if kept { character } else { '_' }, units)
        })
        .take(ID_LIMIT)
        .collect()
}

/// Project the history for the model, convert it into wire messages and place `cache` on the
/// last block of the final message when that message is the user's.
pub(super) fn convert(
    model: &Model,
    context: &Context,
    cache: Option<CacheControl>,
) -> Vec<WireMessage> {
    let normalize = |id: &str, _: &Model, _: &AssistantMessage| normalize_tool_call_id(id);
    let history = transform_messages(&context.messages, model, Some(&normalize));
    let mut wire = Vec::with_capacity(history.len());
    let mut remaining = history.iter().peekable();
    while let Some(message) = remaining.next() {
        wire.extend(match message {
            Message::User(user) => user_message(&user.content),
            Message::Assistant(assistant) => assistant_message(assistant),
            Message::ToolResult(first) => Some(tool_results(first, &mut remaining)),
        });
    }
    if let Some(marker) = cache {
        mark_final_user_message(&mut wire, marker);
    }
    wire
}

/// Convert a user turn; a turn without any text or image produces no message.
fn user_message(content: &UserContent) -> Option<WireMessage> {
    let content = match content {
        UserContent::Text(text) if has_text(text) => Content::Text(text.clone()),
        UserContent::Text(_) => return None,
        UserContent::Blocks(blocks) => {
            let kept: Vec<Block> = blocks
                .iter()
                .filter(|block| match block {
                    UserBlock::Text(text) => has_text(&text.text),
                    UserBlock::Image(_) => true,
                })
                .map(user_block)
                .collect();
            if kept.is_empty() {
                return None;
            }
            Content::Blocks(kept)
        }
    };
    Some(WireMessage {
        role: Role::User,
        content,
    })
}

/// Convert an assistant turn; a turn without any block produces no message.
fn assistant_message(assistant: &AssistantMessage) -> Option<WireMessage> {
    let blocks: Vec<Block> = assistant
        .content
        .iter()
        .filter_map(assistant_block)
        .collect();
    (!blocks.is_empty()).then_some(WireMessage {
        role: Role::Assistant,
        content: Content::Blocks(blocks),
    })
}

/// Convert one block of an assistant turn. Blank text and blank unredacted reasoning are
/// dropped; unredacted reasoning without a nonblank signature becomes plain text; redacted
/// reasoning is kept whatever its text, with its opaque payload.
fn assistant_block(block: &AssistantContent) -> Option<Block> {
    match block {
        AssistantContent::Text(text) => has_text(&text.text).then(|| Block::text(&text.text)),
        AssistantContent::Thinking(thinking) if thinking.redacted == Some(true) => {
            Some(Block::RedactedThinking {
                data: thinking.thinking_signature.clone(),
            })
        }
        AssistantContent::Thinking(thinking) if !has_text(&thinking.thinking) => None,
        AssistantContent::Thinking(thinking) => Some(
            match thinking
                .thinking_signature
                .as_deref()
                .filter(|s| has_text(s))
            {
                Some(signature) => Block::Thinking {
                    thinking: thinking.thinking.clone(),
                    signature: signature.to_owned(),
                },
                None => Block::text(&thinking.thinking),
            },
        ),
        AssistantContent::ToolCall(call) => Some(Block::ToolUse {
            id: call.id.clone(),
            name: call.name.clone(),
            input: call.arguments.clone(),
        }),
    }
}

/// Convert a run of consecutive tool results into one user message.
fn tool_results<'a>(
    first: &ToolResultMessage,
    remaining: &mut Peekable<impl Iterator<Item = &'a Message>>,
) -> WireMessage {
    let result = |message: &ToolResultMessage| Block::ToolResult {
        tool_use_id: message.tool_call_id.clone(),
        content: result_content(&message.content),
        is_error: message.is_error,
        cache_control: None,
    };
    let mut blocks = vec![result(first)];
    while let Some(Message::ToolResult(next)) =
        remaining.next_if(|message| matches!(message, Message::ToolResult(_)))
    {
        blocks.push(result(next));
    }
    WireMessage {
        role: Role::User,
        content: Content::Blocks(blocks),
    }
}

/// Place a marker on the last block of the final message when that message is the user's; plain
/// text becomes a single marked text block.
fn mark_final_user_message(wire: &mut [WireMessage], marker: CacheControl) {
    let Some(last) = wire.last_mut().filter(|message| message.role == Role::User) else {
        return;
    };
    match &mut last.content {
        Content::Blocks(blocks) => {
            if let Some(block) = blocks.last_mut() {
                block.mark(marker);
            }
        }
        Content::Text(text) => {
            last.content = Content::Blocks(vec![Block::Text {
                text: std::mem::take(text),
                cache_control: Some(marker),
            }]);
        }
    }
}

/// A tool declaration.
#[derive(Serialize)]
pub(super) struct ToolParam<'a> {
    /// Tool name.
    name: &'a str,
    /// What the tool does.
    description: &'a str,
    /// Ask the service to stream the arguments as they are written.
    #[serde(skip_serializing_if = "Option::is_none")]
    eager_input_streaming: Option<bool>,
    /// Shape of the arguments.
    input_schema: InputSchema<'a>,
    /// Marker placed on the last tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

/// The parts of a tool's schema the protocol admits.
#[derive(Serialize)]
struct InputSchema<'a> {
    /// Constant `object`.
    r#type: &'static str,
    /// Properties; none declared means an empty object.
    properties: Cow<'a, Value>,
    /// Required property names; none declared means an empty list.
    required: Cow<'a, Value>,
}

/// A member of a tool's schema, or `default` when the schema has none or holds `null`.
fn schema_member<'a>(tool: &'a Tool, name: &str, default: fn() -> Value) -> Cow<'a, Value> {
    match tool.parameters.get(name).filter(|value| !value.is_null()) {
        Some(value) => Cow::Borrowed(value),
        None => Cow::Owned(default()),
    }
}

/// Declare the tools; `cache` marks the last one.
pub(super) fn tools(
    tools: &[Tool],
    eager_input_streaming: bool,
    cache: Option<CacheControl>,
) -> Vec<ToolParam<'_>> {
    let last = tools.len().saturating_sub(1);
    tools
        .iter()
        .enumerate()
        .map(|(position, tool)| ToolParam {
            name: &tool.name,
            description: &tool.description,
            eager_input_streaming: eager_input_streaming.then_some(true),
            input_schema: InputSchema {
                r#type: "object",
                properties: schema_member(tool, "properties", || Value::Object(JsonObject::new())),
                required: schema_member(tool, "required", || Value::Array(Vec::new())),
            },
            cache_control: cache.filter(|_| position == last),
        })
        .collect()
}
