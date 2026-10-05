//! Pure owned model-request views of caller-owned conversation history.

use crate::{
    AssistantContent, AssistantMessage, Context, InputContent, Message, Model, StopReason,
    TextContent, ThinkingContent, ToolCall, ToolResultMessage,
};

/// Clone a selected-model request without modifying history, prompt or tools.
/// Exact requested provider/protocol/model equality retains opaque replay data.
/// Foreign metadata and redactions are omitted; readable thinking becomes text.
/// Signed empty thinking survives same-model replay; unsigned blank thinking is
/// omitted. Without the `image` capability, adjacent image runs become explicit
/// user/tool omission text. Unanswered successful calls receive error results at
/// user/assistant boundaries and transcript end, using only the supplied timestamp.
/// Failed, aborted and partial attempts are omitted without manufacturing arguments.
/// Tool-result details never enter this view. The supplied ID rule must be
/// deterministic and effect-free, preserving distinct IDs within a batch. It is
/// called only for foreign calls; originating-turn mappings pair their results.
pub fn project_context(
    context: &Context,
    model: &Model,
    normalize_tool_call_id: &dyn Fn(&str, &Model, &AssistantMessage) -> String,
    timestamp: u64,
) -> Context {
    let mut projected = Context {
        system_prompt: context.system_prompt.clone(),
        tools: context.tools.clone(),
        messages: Vec::new(),
    };
    let mut pending: Vec<(String, ToolCall, bool)> = Vec::new();
    for message in &context.messages {
        if matches!(message, Message::User(_) | Message::Assistant(_)) {
            repair(&mut projected.messages, &mut pending, timestamp);
        }
        let mut message = message.clone();
        match &mut message {
            Message::Assistant(a) => {
                if !matches!(
                    a.stop_reason,
                    Some(StopReason::Stop | StopReason::Length | StopReason::ToolUse)
                ) {
                    continue;
                }
                let same = a.provider == model.identity.provider
                    && a.protocol == model.protocol
                    && a.model == model.identity.model;
                let source = a.clone();
                a.content = a
                    .content
                    .iter()
                    .filter_map(|block| replay_block(block, same))
                    .collect();
                for block in &mut a.content {
                    if let AssistantContent::ToolCall(call) = block {
                        let original = call.id.clone();
                        if !same {
                            call.id = normalize_tool_call_id(&original, model, &source);
                        }
                        pending.push((original, call.clone(), false));
                    }
                }
            }
            Message::ToolResult(r) => {
                r.details = None;
                r.content = project_images(
                    &r.content,
                    model,
                    "(tool image omitted: model does not support images)",
                );
                if let Some((_, call, answered)) =
                    pending.iter_mut().find(|(id, _, _)| id == &r.tool_call_id)
                {
                    r.tool_call_id = call.id.clone();
                    *answered = true;
                }
            }
            Message::User(u) => {
                u.content = project_images(
                    &u.content,
                    model,
                    "(image omitted: model does not support images)",
                )
            }
        }
        projected.messages.push(message);
    }
    repair(&mut projected.messages, &mut pending, timestamp);
    projected
}

fn repair(
    messages: &mut Vec<Message>,
    pending: &mut Vec<(String, ToolCall, bool)>,
    timestamp: u64,
) {
    for (_, call, answered) in pending.drain(..) {
        if !answered {
            messages.push(Message::ToolResult(ToolResultMessage {
                tool_call_id: call.id,
                tool_name: call.name,
                content: vec![InputContent::Text(TextContent {
                    text: "No result provided".into(),
                    replay_metadata: None,
                })],
                details: None,
                is_error: true,
                timestamp,
            }));
        }
    }
}

fn replay_block(block: &AssistantContent, same: bool) -> Option<AssistantContent> {
    match block {
        AssistantContent::Thinking(ThinkingContent::Readable { text, signature }) => {
            if text.trim().is_empty() && (!same || signature.is_none()) {
                return None;
            }
            if !same {
                return Some(AssistantContent::Text(TextContent {
                    text: text.clone(),
                    replay_metadata: None,
                }));
            }
        }
        AssistantContent::Thinking(ThinkingContent::Redacted { .. }) if !same => return None,
        AssistantContent::Text(text) if !same => {
            return Some(AssistantContent::Text(TextContent {
                text: text.text.clone(),
                replay_metadata: None,
            }));
        }
        AssistantContent::ToolCall(call) if !same => {
            let mut call = call.clone();
            call.replay_metadata = None;
            return Some(AssistantContent::ToolCall(call));
        }
        _ => {}
    }
    Some(block.clone())
}

fn project_images(content: &[InputContent], model: &Model, placeholder: &str) -> Vec<InputContent> {
    if model.input.iter().any(|capability| capability == "image") {
        return content.to_vec();
    }
    let mut projected = Vec::new();
    for block in content {
        let block = match block {
            InputContent::Image(_) => InputContent::Text(TextContent {
                text: placeholder.into(),
                replay_metadata: None,
            }),
            _ => block.clone(),
        };
        let omission = matches!(&block, InputContent::Text(text) if text.text == placeholder && text.replay_metadata.is_none());
        if omission && projected.last() == Some(&block) {
            continue;
        }
        projected.push(block);
    }
    projected
}
