//! Pure owned model-request views of caller-owned conversation history.

use crate::{
    AssistantContent, AssistantMessage, Context, InputContent, Message, Model, StopReason,
    TextContent, ToolCall, ToolResultMessage, UserContent,
};

/// Clone a selected-model request without modifying history, prompt or tools.
/// Exact requested provider/protocol/model equality retains opaque replay data.
/// Foreign metadata and redactions are omitted; readable thinking becomes text.
/// Nonempty signatures preserve blank same-model thinking; unsigned thinking blank
/// under ECMAScript whitespace is omitted. Without the `image` capability, image
/// runs become user/tool omission text. Supplied text always survives unchanged;
/// a preceding omission literal suppresses another placeholder regardless of metadata.
/// Unanswered successful calls receive error results at
/// user/assistant boundaries and transcript end, using only the supplied timestamp.
/// Failed, aborted and partial attempts are omitted without manufacturing arguments.
/// Tool-result details never enter this view. The supplied ID rule must be
/// deterministic and effect-free, preserving distinct IDs within a batch. It is
/// called only for foreign calls. Mappings survive repair to pair delayed real
/// results; each later successful originating call supersedes its ID mapping.
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
    let mut ids = std::collections::HashMap::new();
    for message in &context.messages {
        if matches!(message, Message::User(_) | Message::Assistant(_)) {
            repair(&mut projected.messages, &mut pending, timestamp);
        }
        let mut message = message.clone();
        match &mut message {
            Message::Assistant(a) => {
                if !matches!(
                    a.stop_reason,
                    StopReason::Stop | StopReason::Length | StopReason::ToolUse
                ) {
                    continue;
                }
                let same =
                    a.provider == model.provider && a.api == model.api && a.model == model.id;
                let source = a.clone();
                a.content = a
                    .content
                    .iter()
                    .filter_map(|block| replay_block(block, same))
                    .collect();
                for block in &mut a.content {
                    if let AssistantContent::ToolCall(call) = block {
                        let mut call = call.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let original = call.id.clone();
                        if !same {
                            call.id = normalize_tool_call_id(&original, model, &source);
                        }
                        ids.insert(original.clone(), call.id.clone());
                        pending.push((original, call.clone(), false));
                        *block = AssistantContent::ToolCall(std::sync::Arc::new(
                            std::sync::RwLock::new(call),
                        ));
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
                if let Some((_, _, answered)) =
                    pending.iter_mut().find(|(id, _, _)| id == &r.tool_call_id)
                {
                    *answered = true;
                }
                if let Some(id) = ids.get(&r.tool_call_id) {
                    r.tool_call_id = id.clone();
                }
            }
            Message::User(u) => {
                if let UserContent::Blocks(content) = &u.content {
                    u.content = UserContent::Blocks(project_images(
                        content,
                        model,
                        "(image omitted: model does not support images)",
                    ));
                }
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
                    text_signature: None,
                })],
                details: None,
                is_error: true,
                timestamp: timestamp as f64,
            }));
        }
    }
}

fn replay_block(block: &AssistantContent, same: bool) -> Option<AssistantContent> {
    match block {
        AssistantContent::Thinking(thinking) => {
            if thinking.redacted == Some(true) {
                return same.then(|| block.clone());
            }
            if crate::scalar::trim(&thinking.thinking).is_empty()
                && (!same
                    || thinking
                        .thinking_signature
                        .as_deref()
                        .is_none_or(str::is_empty))
            {
                return None;
            }
            if !same {
                return Some(AssistantContent::Text(TextContent {
                    text: thinking.thinking.clone(),
                    text_signature: None,
                }));
            }
        }
        AssistantContent::Text(text) if !same => {
            return Some(AssistantContent::Text(TextContent {
                text: text.text.clone(),
                text_signature: None,
            }));
        }
        AssistantContent::ToolCall(call) => {
            let mut call = call.read().unwrap_or_else(|p| p.into_inner()).clone();
            if !same {
                call.thought_signature = None;
            }
            return Some(AssistantContent::ToolCall(std::sync::Arc::new(
                std::sync::RwLock::new(call),
            )));
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
        match block {
            InputContent::Text(_) => projected.push(block.clone()),
            InputContent::Image(_) => {
                if !matches!(projected.last(), Some(InputContent::Text(text)) if text.text == placeholder)
                {
                    projected.push(InputContent::Text(TextContent {
                        text: placeholder.into(),
                        text_signature: None,
                    }));
                }
            }
        }
    }
    projected
}
