use std::borrow::Cow;

use crate::records::diagnostics::timestamp_now;
use crate::{
    AssistantContent, AssistantMessage, Message, Model, StopReason, TextContent, ToolCall,
    ToolResultMessage, UserBlock, UserContent,
};

type Normalizer<'a> = Option<&'a dyn Fn(&str, &Model, &AssistantMessage) -> String>;

struct Occurrence<'a> {
    source: &'a ToolCall,
    id: Cow<'a, str>,
    answered: bool,
}

struct Projection<'a> {
    source: &'a Message,
    retained: bool,
    same: bool,
    blocks: Vec<bool>,
    calls: Vec<Occurrence<'a>>,
    association: Option<(usize, usize)>,
    missing: Vec<usize>,
}

#[doc = include_str!("../../../../docs/conversation-projection.md")]
#[must_use]
pub fn transform_messages(
    messages: &[Message],
    model: &Model,
    normalize_tool_call_id: Normalizer<'_>,
) -> Vec<Message> {
    let mut projected: Vec<Projection<'_>> = Vec::with_capacity(messages.len());
    let mut batch = None;
    let images = model.input.contains(&crate::ModelInput::Image);
    for message in messages {
        let mut projection = prepare(message, model, normalize_tool_call_id, images);
        match message {
            Message::Assistant(_) => batch = Some(projected.len()),
            Message::User(_) => batch = None,
            Message::ToolResult(source) => {
                if let Some(index) = batch {
                    projection.association = associate_result(source, index, &mut projected[index]);
                }
            }
        }
        projected.push(projection);
    }
    for projection in &mut projected {
        if projection.retained {
            projection.missing = projection
                .calls
                .iter()
                .enumerate()
                .filter_map(|(index, call)| (!call.answered).then_some(index))
                .collect();
        }
    }
    build_output(&projected, images)
}

fn prepare<'a>(
    source: &'a Message,
    model: &Model,
    normalize: Normalizer<'_>,
    images: bool,
) -> Projection<'a> {
    let mut projection = Projection {
        source,
        retained: true,
        same: false,
        blocks: Vec::new(),
        calls: Vec::new(),
        association: None,
        missing: Vec::new(),
    };
    match source {
        Message::Assistant(assistant) => {
            prepare_assistant(&mut projection, assistant, model, normalize);
        }
        Message::User(user) => {
            if let UserContent::Blocks(blocks) = &user.content {
                projection.blocks = retain_blocks(
                    blocks,
                    images,
                    "(image omitted: model does not support images)",
                );
            }
        }
        Message::ToolResult(result) => {
            projection.blocks = retain_blocks(
                &result.content,
                images,
                "(tool image omitted: model does not support images)",
            );
        }
    }
    projection
}

fn prepare_assistant<'a>(
    projection: &mut Projection<'a>,
    assistant: &'a AssistantMessage,
    model: &Model,
    normalize: Normalizer<'_>,
) {
    projection.same = assistant.provider == model.provider
        && assistant.api == model.api
        && assistant.model == model.id;
    projection.retained = !matches!(
        assistant.stop_reason,
        StopReason::Error | StopReason::Aborted
    );
    for block in &assistant.content {
        projection.blocks.push(match block {
            AssistantContent::Thinking(value) => keep_thinking(value, projection.same),
            _ => true,
        });
        if let AssistantContent::ToolCall(call) = block {
            let id = if projection.same {
                None
            } else {
                normalize.map(|normalize| Cow::Owned(normalize(&call.id, model, assistant)))
            }
            .unwrap_or(Cow::Borrowed(call.id.as_str()));
            projection.calls.push(Occurrence {
                source: call,
                id,
                answered: false,
            });
        }
    }
}

fn keep_thinking(value: &crate::ThinkingContent, same: bool) -> bool {
    if value.redacted == Some(true) {
        return same;
    }
    let signed = value
        .thinking_signature
        .as_ref()
        .is_some_and(|signature| !signature.is_empty());
    let blank = value
        .thinking
        .chars()
        .all(|c| (c.is_whitespace() && c != '\u{0085}') || c == '\u{feff}');
    (same && signed) || !blank
}

fn retain_blocks(blocks: &[UserBlock], images: bool, placeholder: &str) -> Vec<bool> {
    let mut previous_placeholder = false;
    blocks
        .iter()
        .map(|block| match block {
            UserBlock::Image(_) if !images => {
                let retained = !previous_placeholder;
                previous_placeholder = true;
                retained
            }
            UserBlock::Text(text) => {
                previous_placeholder = text.text == placeholder;
                true
            }
            UserBlock::Image(_) => {
                previous_placeholder = false;
                true
            }
        })
        .collect()
}

fn associate_result(
    result: &ToolResultMessage,
    batch: usize,
    projection: &mut Projection<'_>,
) -> Option<(usize, usize)> {
    let (index, call) = projection
        .calls
        .iter_mut()
        .enumerate()
        .find(|(_, call)| !call.answered && call.source.id == result.tool_call_id)?;
    call.answered = true;
    Some((batch, index))
}

fn build_output(projected: &[Projection<'_>], images: bool) -> Vec<Message> {
    let mut result = Vec::with_capacity(projected.len());
    let mut pending = None;
    for projection in projected {
        if matches!(projection.source, Message::Assistant(_) | Message::User(_))
            && let Some(pending) = pending.take()
        {
            flush_missing(&mut result, pending);
        }
        if !projection.retained {
            continue;
        }
        if matches!(projection.source, Message::Assistant(_)) {
            pending = Some(projection);
        }
        let message = build_message(projection, projected, images);
        result.push(message);
    }
    if let Some(pending) = pending {
        flush_missing(&mut result, pending);
    }
    result
}

fn build_message(
    projection: &Projection<'_>,
    projected: &[Projection<'_>],
    images: bool,
) -> Message {
    match projection.source {
        Message::Assistant(source) => Message::Assistant(build_assistant(source, projection)),
        Message::User(source) => {
            let content = match &source.content {
                UserContent::Text(text) => UserContent::Text(text.clone()),
                UserContent::Blocks(blocks) => UserContent::Blocks(build_blocks(
                    blocks,
                    &projection.blocks,
                    images,
                    "(image omitted: model does not support images)",
                )),
            };
            Message::User(crate::UserMessage {
                content,
                timestamp: source.timestamp,
            })
        }
        Message::ToolResult(source) => {
            let id = projection
                .association
                .map_or(source.tool_call_id.as_str(), |(batch, index)| {
                    projected[batch].calls[index].id.as_ref()
                });
            Message::ToolResult(ToolResultMessage {
                tool_call_id: id.to_owned(),
                tool_name: source.tool_name.clone(),
                content: build_blocks(
                    &source.content,
                    &projection.blocks,
                    images,
                    "(tool image omitted: model does not support images)",
                ),
                details: source.details.clone(),
                is_error: source.is_error,
                timestamp: source.timestamp,
            })
        }
    }
}

fn build_assistant(source: &AssistantMessage, projection: &Projection<'_>) -> AssistantMessage {
    let content = build_content(source, projection);
    AssistantMessage {
        content,
        api: source.api.clone(),
        provider: source.provider.clone(),
        model: source.model.clone(),
        response_model: source.response_model.clone(),
        response_id: source.response_id.clone(),
        diagnostics: source.diagnostics.clone(),
        usage: source.usage.clone(),
        stop_reason: source.stop_reason.clone(),
        error_message: source.error_message.clone(),
        timestamp: source.timestamp,
    }
}

fn build_content(source: &AssistantMessage, projection: &Projection<'_>) -> Vec<AssistantContent> {
    let mut calls = projection.calls.iter();
    source
        .content
        .iter()
        .zip(&projection.blocks)
        .filter_map(|(block, retained)| {
            if !retained {
                return None;
            }
            Some(match block {
                AssistantContent::Thinking(value) if !projection.same => {
                    AssistantContent::Text(unsigned_text(&value.thinking))
                }
                AssistantContent::Thinking(value) => AssistantContent::Thinking(value.clone()),
                AssistantContent::Text(value) => AssistantContent::Text(TextContent {
                    text: value.text.clone(),
                    text_signature: if projection.same {
                        value.text_signature.clone()
                    } else {
                        None
                    },
                }),
                AssistantContent::ToolCall(value) => AssistantContent::ToolCall(ToolCall {
                    id: calls
                        .next()
                        .map_or(value.id.as_str(), |call| call.id.as_ref())
                        .to_owned(),
                    name: value.name.clone(),
                    arguments: value.arguments.clone(),
                    thought_signature: value
                        .thought_signature
                        .as_ref()
                        .filter(|signature| projection.same || signature.is_empty())
                        .cloned(),
                }),
            })
        })
        .collect()
}

fn unsigned_text(text: &str) -> TextContent {
    TextContent {
        text: text.to_owned(),
        text_signature: None,
    }
}

fn build_blocks(
    blocks: &[UserBlock],
    retained: &[bool],
    images: bool,
    placeholder: &str,
) -> Vec<UserBlock> {
    blocks
        .iter()
        .zip(retained)
        .filter(|(_, retained)| **retained)
        .map(|(block, _)| match block {
            UserBlock::Image(_) if !images => UserBlock::Text(unsigned_text(placeholder)),
            _ => block.clone(),
        })
        .collect()
}

fn flush_missing(result: &mut Vec<Message>, projection: &Projection<'_>) {
    for &index in &projection.missing {
        let call = &projection.calls[index];
        let tool_call_id = call.id.to_string();
        let tool_name = call.source.name.clone();
        result.push(Message::ToolResult(ToolResultMessage {
            tool_call_id,
            tool_name,
            content: vec![UserBlock::Text(unsigned_text("No result provided"))],
            details: None,
            is_error: true,
            timestamp: timestamp_now(),
        }));
    }
}
