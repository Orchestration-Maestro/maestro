use crate::records::diagnostics::timestamp_now;
use crate::{
    AssistantContent, AssistantMessage, Message, Model, StopReason, TextContent, ToolResultMessage,
    UserBlock,
};

type Normalizer<'a> = Option<&'a dyn Fn(&str, &Model, &AssistantMessage) -> String>;

struct Occurrence<'a> {
    original_id: &'a str,
    id: String,
    name: String,
    answered: bool,
}

#[doc = include_str!("../../../../docs/conversation-projection.md")]
#[must_use]
pub fn transform_messages(
    messages: &[Message],
    model: &Model,
    normalize_tool_call_id: Normalizer<'_>,
) -> Vec<Message> {
    let mut projected: Vec<(Message, Vec<Occurrence<'_>>)> = Vec::with_capacity(messages.len());
    let mut batch = None;
    for message in messages {
        let (mut output, calls) = match message {
            Message::Assistant(source) => {
                let mut output = copy_assistant_metadata(source);
                let calls = project_assistant(&mut output, source, model, normalize_tool_call_id);
                batch = Some(projected.len());
                (Message::Assistant(output), calls)
            }
            Message::User(source) => {
                batch = None;
                (Message::User(source.clone()), Vec::new())
            }
            Message::ToolResult(source) => {
                let mut output = source.clone();
                if let Some(index) = batch {
                    associate_result(&mut output, &mut projected[index].1);
                }
                (Message::ToolResult(output), Vec::new())
            }
        };
        if !model.input.contains(&crate::ModelInput::Image) {
            downgrade_images(&mut output);
        }
        projected.push((output, calls));
    }
    repair_results(projected)
}

fn project_assistant<'a>(
    output: &mut AssistantMessage,
    source: &'a AssistantMessage,
    model: &Model,
    normalize: Normalizer<'_>,
) -> Vec<Occurrence<'a>> {
    let same =
        source.provider == model.provider && source.api == model.api && source.model == model.id;
    let mut calls = Vec::new();
    output.content = source
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContent::Thinking(value) => project_thinking(value.clone(), same),
            AssistantContent::Text(value) => {
                let mut value = value.clone();
                if !same {
                    value.text_signature = None;
                }
                Some(AssistantContent::Text(value))
            }
            AssistantContent::ToolCall(original) => {
                let mut value = original.clone();
                if !same {
                    if value
                        .thought_signature
                        .as_ref()
                        .is_some_and(|signature| !signature.is_empty())
                    {
                        value.thought_signature = None;
                    }
                    if let Some(normalize) = normalize {
                        value.id = normalize(&original.id, model, source);
                    }
                }
                calls.push(Occurrence {
                    original_id: &original.id,
                    id: value.id.clone(),
                    name: value.name.clone(),
                    answered: false,
                });
                Some(AssistantContent::ToolCall(value))
            }
        })
        .collect();
    calls
}

fn copy_assistant_metadata(source: &AssistantMessage) -> AssistantMessage {
    AssistantMessage {
        content: Vec::new(),
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

fn project_thinking(value: crate::ThinkingContent, same: bool) -> Option<AssistantContent> {
    if value.redacted == Some(true) {
        return same.then_some(AssistantContent::Thinking(value));
    }
    let signed = value
        .thinking_signature
        .as_ref()
        .is_some_and(|signature| !signature.is_empty());
    let blank = value
        .thinking
        .chars()
        .all(|c| (c.is_whitespace() && c != '\u{0085}') || c == '\u{feff}');
    if same && signed {
        Some(AssistantContent::Thinking(value))
    } else if blank {
        None
    } else if same {
        Some(AssistantContent::Thinking(value))
    } else {
        Some(AssistantContent::Text(TextContent {
            text: value.thinking,
            text_signature: None,
        }))
    }
}

fn unsigned_text(text: &str) -> TextContent {
    TextContent {
        text: text.to_owned(),
        text_signature: None,
    }
}

fn downgrade_images(message: &mut Message) {
    let (blocks, placeholder) = match message {
        Message::User(crate::UserMessage {
            content: crate::UserContent::Blocks(blocks),
            ..
        }) => (blocks, "(image omitted: model does not support images)"),
        Message::ToolResult(result) => (
            &mut result.content,
            "(tool image omitted: model does not support images)",
        ),
        _ => return,
    };
    let mut output = Vec::with_capacity(blocks.len());
    for block in std::mem::take(blocks) {
        match block {
            UserBlock::Image(_) => {
                if !matches!(output.last(), Some(UserBlock::Text(text)) if text.text == placeholder)
                {
                    output.push(UserBlock::Text(unsigned_text(placeholder)));
                }
            }
            UserBlock::Text(_) => output.push(block),
        }
    }
    *blocks = output;
}

fn associate_result(result: &mut ToolResultMessage, calls: &mut [Occurrence<'_>]) {
    if let Some(call) = calls
        .iter_mut()
        .find(|call| !call.answered && call.original_id == result.tool_call_id)
    {
        result.tool_call_id.clone_from(&call.id);
        call.answered = true;
    }
}

fn repair_results(projected: Vec<(Message, Vec<Occurrence<'_>>)>) -> Vec<Message> {
    let mut result = Vec::with_capacity(projected.len());
    let mut pending = Vec::new();
    for (message, calls) in projected {
        match &message {
            Message::Assistant(assistant) => {
                flush_missing(&mut result, std::mem::take(&mut pending));
                if matches!(
                    assistant.stop_reason,
                    StopReason::Error | StopReason::Aborted
                ) {
                    continue;
                }
                pending = calls;
            }
            Message::User(_) => flush_missing(&mut result, std::mem::take(&mut pending)),
            Message::ToolResult(_) => {}
        }
        result.push(message);
    }
    flush_missing(&mut result, pending);
    result
}

fn flush_missing(result: &mut Vec<Message>, pending: Vec<Occurrence<'_>>) {
    result.extend(
        pending
            .into_iter()
            .filter(|call| !call.answered)
            .map(|call| {
                Message::ToolResult(ToolResultMessage {
                    tool_call_id: call.id,
                    tool_name: call.name,
                    content: vec![UserBlock::Text(unsigned_text("No result provided"))],
                    details: None,
                    is_error: true,
                    timestamp: timestamp_now(),
                })
            }),
    );
}
