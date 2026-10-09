//! Borrowed message views for callback payloads.

use super::{tool_ids::ToolCallIds, trim};
use crate::providers::json_text::compact_object;
use crate::{
    AssistantContent, Context, DiagnosticErrorInfo, Message, Model, UserBlock, UserContent,
};
use serde::Serialize;

/// A message in the callback's field spelling.
#[derive(Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub(super) enum ChatMessage<'a> {
    /// System instructions.
    System {
        /// Authored instructions.
        content: &'a str,
    },
    /// User input.
    User {
        /// Text or ordered blocks.
        content: Content<'a>,
    },
    /// Assistant replay.
    Assistant {
        /// Retained content.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        content: Vec<Chunk<'a>>,
        /// Calls in original order.
        #[serde(rename = "toolCalls", skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<Call<'a>>,
    },
    /// Tool output.
    Tool {
        /// Associated call identifier.
        #[serde(rename = "toolCallId")]
        tool_call_id: &'a str,
        /// Tool name.
        name: &'a str,
        /// Text followed by images.
        content: Vec<Chunk<'a>>,
    },
}

/// User content keeps string and block-list forms distinct.
#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Content<'a> {
    /// Plain text.
    Text(&'a str),
    /// Ordered blocks.
    Blocks(Vec<Chunk<'a>>),
}

/// Content selected from the projected conversation.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum Chunk<'a> {
    /// Borrowed text.
    Text {
        /// Authored text.
        text: std::borrow::Cow<'a, str>,
    },
    /// Inline image URL.
    ImageUrl {
        /// MIME and data interpolation.
        #[serde(rename = "imageUrl")]
        image_url: String,
    },
    /// Thinking replay.
    Thinking {
        /// Text inside thinking.
        thinking: Vec<Chunk<'a>>,
    },
}

/// Tool call in callback spelling.
#[derive(Serialize)]
pub(super) struct Call<'a> {
    /// Projected identifier.
    id: &'a str,
    /// Function discriminator.
    r#type: &'static str,
    /// Function invocation.
    function: Function<'a>,
}

/// Function invocation with compact argument text.
#[derive(Serialize)]
struct Function<'a> {
    /// Function name.
    name: &'a str,
    /// Encoded arguments.
    arguments: String,
}

/// Project foreign history with a request-local identifier owner.
pub(super) fn project(context: &Context, model: &Model) -> Vec<Message> {
    let ids = std::cell::RefCell::new(ToolCallIds::default());
    crate::transform_messages(
        &context.messages,
        model,
        Some(&|id, _, _| ids.borrow_mut().normalize(id)),
    )
}

/// Select callback messages from already projected history.
pub(super) fn convert<'a>(
    messages: &'a [Message],
    system: Option<&'a str>,
) -> Result<Vec<ChatMessage<'a>>, DiagnosticErrorInfo> {
    let mut output = Vec::new();
    if let Some(content) = system.filter(|text| !text.is_empty()) {
        output.push(ChatMessage::System { content });
    }
    for message in messages {
        match message {
            Message::User(user) => match &user.content {
                UserContent::Text(text) => output.push(ChatMessage::User {
                    content: Content::Text(text),
                }),
                UserContent::Blocks(blocks) if !blocks.is_empty() => {
                    output.push(ChatMessage::User {
                        content: Content::Blocks(blocks.iter().map(chunk).collect()),
                    });
                }
                UserContent::Blocks(_) => {}
            },
            Message::Assistant(assistant) => {
                if let Some(message) = assistant_message(&assistant.content)? {
                    output.push(message);
                }
            }
            Message::ToolResult(tool) => output.push(tool_message(tool)),
        }
    }
    Ok(output)
}

/// Keep nonblank assistant content and all calls.
fn assistant_message(
    blocks: &[AssistantContent],
) -> Result<Option<ChatMessage<'_>>, DiagnosticErrorInfo> {
    let mut content = Vec::new();
    let mut tool_calls = Vec::new();
    for block in blocks {
        match block {
            AssistantContent::Text(text) if !trim(&text.text).is_empty() => {
                content.push(Chunk::Text {
                    text: text.text.as_str().into(),
                });
            }
            AssistantContent::Thinking(thinking) if !trim(&thinking.thinking).is_empty() => content
                .push(Chunk::Thinking {
                    thinking: vec![Chunk::Text {
                        text: thinking.thinking.as_str().into(),
                    }],
                }),
            AssistantContent::ToolCall(call) => tool_calls.push(Call {
                id: &call.id,
                r#type: "function",
                function: Function {
                    name: &call.name,
                    arguments: compact_object(&call.arguments).map_err(super::native_error)?,
                },
            }),
            _ => {}
        }
    }
    Ok(
        (!content.is_empty() || !tool_calls.is_empty()).then_some(ChatMessage::Assistant {
            content,
            tool_calls,
        }),
    )
}

/// Convert one projected user block.
fn chunk(block: &UserBlock) -> Chunk<'_> {
    match block {
        UserBlock::Text(text) => Chunk::Text {
            text: text.text.as_str().into(),
        },
        UserBlock::Image(image) => Chunk::ImageUrl {
            image_url: format!("data:{};base64,{}", image.mime_type, image.data),
        },
    }
}

/// Join tool text before trimming and append the projected images.
fn tool_message(tool: &crate::ToolResultMessage) -> ChatMessage<'_> {
    let joined = tool
        .content
        .iter()
        .filter_map(|block| match block {
            UserBlock::Text(text) => Some(text.text.as_str()),
            UserBlock::Image(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let text = trim(&joined);
    let has_images = tool
        .content
        .iter()
        .any(|block| matches!(block, UserBlock::Image(_)));
    let text = if text.is_empty() {
        if has_images {
            "(see attached image)"
        } else {
            "(no tool output)"
        }
    } else {
        text
    };
    let prefix = if tool.is_error { "[tool error] " } else { "" };
    let mut content = vec![Chunk::Text {
        text: format!("{prefix}{text}").into(),
    }];
    content.extend(
        tool.content
            .iter()
            .filter(|block| matches!(block, UserBlock::Image(_)))
            .map(chunk),
    );
    ChatMessage::Tool {
        tool_call_id: &tool.tool_call_id,
        name: &tool.tool_name,
        content,
    }
}
