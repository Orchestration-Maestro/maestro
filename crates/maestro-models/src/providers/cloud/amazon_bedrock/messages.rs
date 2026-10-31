//! Conversion of the conversation into the service's turns, system blocks and tool
//! configuration.

use std::iter::Peekable;
use std::vec::IntoIter;

use base64::Engine as _;

use super::request::Message;
use super::thinking::is_anthropic_claude_model;
use super::wire::{
    Blob, CachePointBlock, ContentBlock, ImageBlock, ImageSource, NamedToolChoice,
    ReasoningContentBlock, ReasoningTextBlock, SystemContentBlock, Tool, ToolChoiceBlock,
    ToolConfiguration, ToolInputSchema, ToolResultBlock, ToolResultContentBlock, ToolSpecification,
    ToolUseBlock, Union, Unit,
};
use crate::arguments::json_parse::whitespace;
use crate::projection::normalize_tool_call_id;
use crate::providers::base64_text::FORGIVING;
use crate::providers::http::RequestFailure;
use crate::{
    AssistantContent, AssistantMessage, Message as Turn, Model, Tool as ContextTool, ToolChoice,
    ToolResultMessage, UserBlock, UserContent, transform_messages,
};

/// Project the history for the model and convert it into turns; `cache` is placed after the
/// last block of the final turn when that turn is the user's.
///
/// # Errors
/// Fails when an image remaining after [`transform_messages`] projection has an unsupported
/// type or undecodable data.
pub(super) fn messages(
    model: &Model,
    history: &[Turn],
    cache: Option<&CachePointBlock>,
) -> Result<Vec<Message>, RequestFailure> {
    let normalize = |id: &str, _: &Model, _: &AssistantMessage| normalize_tool_call_id(id);
    let claude = is_anthropic_claude_model(model);
    let mut remaining = transform_messages(history, model, Some(&normalize))
        .into_iter()
        .peekable();
    let mut turns = Vec::with_capacity(history.len());
    while let Some(turn) = remaining.next() {
        turns.extend(match turn {
            Turn::User(user) => Some(user_message(user.content)?),
            Turn::Assistant(assistant) => assistant_message(assistant, claude),
            Turn::ToolResult(first) => Some(tool_results(first, &mut remaining)?),
        });
    }
    let last = turns
        .last_mut()
        .filter(|turn| turn.role.as_deref() == Some("user"));
    if let (Some(content), Some(cache)) = (last.and_then(|turn| turn.content.as_mut()), cache) {
        content.push(
            ContentBlock {
                cache_point: Some(cache.clone()),
                ..ContentBlock::default()
            }
            .into(),
        );
    }
    Ok(turns)
}

/// The system prompt as one text block followed by `cache`, or none when it is empty.
pub(super) fn system_blocks(
    system_prompt: Option<String>,
    cache: Option<&CachePointBlock>,
) -> Option<Vec<Union<SystemContentBlock>>> {
    let prompt = system_prompt.filter(|prompt| !prompt.is_empty())?;
    let mut blocks = vec![
        SystemContentBlock {
            text: Some(prompt),
            cache_point: None,
            ..SystemContentBlock::default()
        }
        .into(),
    ];
    blocks.extend(cache.map(|cache| {
        SystemContentBlock {
            text: None,
            cache_point: Some(cache.clone()),
            ..SystemContentBlock::default()
        }
        .into()
    }));
    Some(blocks)
}

/// The tool declarations with the selected choice; none without tools or with the `None` choice.
pub(super) fn tool_config(
    tools: Option<Vec<ContextTool>>,
    choice: Option<&ToolChoice>,
) -> Option<ToolConfiguration> {
    let tools = tools.filter(|tools| !tools.is_empty())?;
    let tool_choice = match choice {
        Some(ToolChoice::None) => return None,
        Some(ToolChoice::Auto) => Some(
            ToolChoiceBlock {
                auto: Some(Unit {}),
                ..ToolChoiceBlock::default()
            }
            .into(),
        ),
        Some(ToolChoice::Required) => Some(
            ToolChoiceBlock {
                any: Some(Unit {}),
                ..ToolChoiceBlock::default()
            }
            .into(),
        ),
        Some(ToolChoice::Function { name }) => Some(
            ToolChoiceBlock {
                tool: Some(NamedToolChoice {
                    name: Some(name.clone()),
                }),
                ..ToolChoiceBlock::default()
            }
            .into(),
        ),
        None => None,
    };
    Some(ToolConfiguration {
        tools: Some(
            tools
                .into_iter()
                .map(|tool| {
                    Tool {
                        spec: Some(ToolSpecification {
                            name: Some(tool.name),
                            description: Some(tool.description),
                            input_schema: Some(
                                ToolInputSchema {
                                    json: Some(tool.parameters.into_json()),
                                }
                                .into(),
                            ),
                            ..ToolSpecification::default()
                        }),
                        ..Tool::default()
                    }
                    .into()
                })
                .collect(),
        ),
        tool_choice,
    })
}

/// Text sent in place of a required block that has no text.
const EMPTY_TEXT_PLACEHOLDER: &str = "<empty>";

/// Whether `text` holds only whitespace.
fn blank(text: &str) -> bool {
    text.trim_matches(whitespace).is_empty()
}

/// A text block holding `text` as written.
fn text_block(text: String) -> Union<ContentBlock> {
    ContentBlock {
        text: Some(text),
        ..ContentBlock::default()
    }
    .into()
}

/// A user turn; blank text blocks are dropped and a turn left without a block, or whose plain
/// text is blank, holds the placeholder.
fn user_message(content: UserContent) -> Result<Message, RequestFailure> {
    let mut blocks = match content {
        UserContent::Text(text) if blank(&text) => Vec::new(),
        UserContent::Text(text) => vec![text_block(text)],
        UserContent::Blocks(blocks) => blocks
            .into_iter()
            .filter_map(|block| match media(block) {
                Ok((Some(text), _)) if blank(&text) => None,
                Ok((text, image)) => Some(Ok(ContentBlock {
                    text,
                    image,
                    ..ContentBlock::default()
                }
                .into())),
                Err(failure) => Some(Err(failure)),
            })
            .collect::<Result<Vec<_>, RequestFailure>>()?,
    };
    if blocks.is_empty() {
        blocks.push(text_block(EMPTY_TEXT_PLACEHOLDER.to_owned()));
    }
    Ok(Message {
        role: Some("user".to_owned()),
        content: Some(blocks),
    })
}

/// An assistant turn; a turn without any surviving block produces no message.
fn assistant_message(assistant: AssistantMessage, claude: bool) -> Option<Message> {
    let blocks: Vec<Union<ContentBlock>> = assistant
        .content
        .into_iter()
        .filter_map(|block| assistant_block(block, claude))
        .collect();
    (!blocks.is_empty()).then_some(Message {
        role: Some("assistant".to_owned()),
        content: Some(blocks),
    })
}

/// One block of an assistant turn. Blank text and blank thinking are dropped. Claude thinking
/// without a nonblank signature becomes plain text; other models get it without a signature.
fn assistant_block(block: AssistantContent, claude: bool) -> Option<Union<ContentBlock>> {
    let reasoning = |text: String, signature: Option<String>| {
        ContentBlock {
            reasoning_content: Some(
                ReasoningContentBlock {
                    reasoning_text: Some(ReasoningTextBlock {
                        text: Some(text),
                        signature,
                    }),
                    ..ReasoningContentBlock::default()
                }
                .into(),
            ),
            ..ContentBlock::default()
        }
        .into()
    };
    match block {
        AssistantContent::Text(text) => (!blank(&text.text)).then(|| text_block(text.text)),
        AssistantContent::ToolCall(call) => Some(
            ContentBlock {
                tool_use: Some(ToolUseBlock {
                    tool_use_id: Some(call.id),
                    name: Some(call.name),
                    input: Some(call.arguments),
                    ..ToolUseBlock::default()
                }),
                ..ContentBlock::default()
            }
            .into(),
        ),
        AssistantContent::Thinking(thinking) => {
            if blank(&thinking.thinking) {
                return None;
            }
            let signature = thinking
                .thinking_signature
                .filter(|signature| !blank(signature));
            Some(match (claude, signature) {
                (true, None) => text_block(thinking.thinking),
                (true, signature) => reasoning(thinking.thinking, signature),
                (false, _) => reasoning(thinking.thinking, None),
            })
        }
    }
}

/// A run of consecutive tool results as one user turn.
fn tool_results(
    first: ToolResultMessage,
    remaining: &mut Peekable<IntoIter<Turn>>,
) -> Result<Message, RequestFailure> {
    let mut blocks = vec![tool_result(first)?];
    while let Some(Turn::ToolResult(next)) =
        remaining.next_if(|turn| matches!(turn, Turn::ToolResult(_)))
    {
        blocks.push(tool_result(next)?);
    }
    Ok(Message {
        role: Some("user".to_owned()),
        content: Some(blocks),
    })
}

/// One tool result with its status; blank text is dropped and a result left without content
/// holds the placeholder.
fn tool_result(result: ToolResultMessage) -> Result<Union<ContentBlock>, RequestFailure> {
    let mut content = Vec::new();
    for block in result.content {
        let (text, image) = media(block)?;
        if text.as_deref().is_some_and(blank) {
            continue;
        }
        content.push(
            ToolResultContentBlock {
                text,
                image,
                ..ToolResultContentBlock::default()
            }
            .into(),
        );
    }
    if content.is_empty() {
        content.push(
            ToolResultContentBlock {
                text: Some(EMPTY_TEXT_PLACEHOLDER.to_owned()),
                ..ToolResultContentBlock::default()
            }
            .into(),
        );
    }
    Ok(ContentBlock {
        tool_result: Some(ToolResultBlock {
            tool_use_id: Some(result.tool_call_id),
            content: Some(content),
            status: Some(if result.is_error { "error" } else { "success" }.to_owned()),
            ..ToolResultBlock::default()
        }),
        ..ContentBlock::default()
    }
    .into())
}

/// The text or the converted image of a user block.
fn media(block: UserBlock) -> Result<(Option<String>, Option<ImageBlock>), RequestFailure> {
    Ok(match block {
        UserBlock::Text(text) => (Some(text.text), None),
        UserBlock::Image(image) => (None, Some(image_block(&image.mime_type, &image.data)?)),
    })
}

/// An image block: the format is chosen from the MIME type before the data is decoded, and
/// ASCII whitespace is removed from the data.
fn image_block(mime_type: &str, data: &str) -> Result<ImageBlock, RequestFailure> {
    let format = match mime_type {
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => {
            return Err(RequestFailure::new(format!(
                "Unknown image type: {mime_type}"
            )));
        }
    };
    let encoded: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let bytes = FORGIVING
        .decode(encoded)
        .map_err(|error| RequestFailure::new(error.to_string()))?;
    Ok(ImageBlock {
        source: Some(
            ImageSource {
                bytes: Some(Blob::Bytes(bytes)),
                ..ImageSource::default()
            }
            .into(),
        ),
        format: Some(format.to_owned()),
        ..ImageBlock::default()
    })
}
