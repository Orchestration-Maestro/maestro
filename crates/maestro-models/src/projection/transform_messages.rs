use crate::{AssistantContent, AssistantMessage, Message, Model, TextContent};

/// Transform supplied messages into an ordered model-facing sequence.
///
/// `None` leaves identifiers untouched; a callback runs only for foreign calls.
/// Same-model replay requires matching provider, API and model ID. Image and
/// replay conversion completes before missing-result repair. Original result
/// details are retained. Unchanged tool calls share their original handles;
/// the transformation does not write history, but a callback may mutate handles.
/// Each inserted result samples Unix milliseconds internally. Callback panics
/// propagate without recovery output. Poisoned tool-call locks are recovered.
/// This function performs no validation, tool execution or history writes.
#[allow(clippy::type_complexity)]
pub fn transform_messages(
    messages: &[Message],
    model: &Model,
    mut normalize_tool_call_id: Option<&mut dyn FnMut(&str, &Model, &AssistantMessage) -> String>,
) -> Vec<Message> {
    let image_aware_messages = downgrade_unsupported_images(messages, model);
    let mut ids = std::collections::HashMap::<String, String>::new();
    let transformed: Vec<_> = image_aware_messages
        .iter()
        .map(|message| {
            let source_message = message;
            let mut message = message.clone();
            if let Message::Assistant(a) = &mut message {
                let same =
                    a.provider == model.provider && a.api == model.api && a.model == model.id;
                a.content = a
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        AssistantContent::Thinking(t) => {
                            if t.redacted == Some(true) {
                                return same.then(|| block.clone());
                            }
                            if same
                                && t.thinking_signature
                                    .as_deref()
                                    .is_some_and(|s| !s.is_empty())
                            {
                                return Some(block.clone());
                            }
                            if crate::scalar::trim(&t.thinking).is_empty() {
                                return None;
                            }
                            if same {
                                Some(block.clone())
                            } else {
                                Some(AssistantContent::Text(TextContent {
                                    text: t.thinking.clone(),
                                    text_signature: None,
                                }))
                            }
                        }
                        AssistantContent::Text(t) if !same => {
                            Some(AssistantContent::Text(TextContent {
                                text: t.text.clone(),
                                text_signature: None,
                            }))
                        }
                        AssistantContent::ToolCall(call) if !same => {
                            let snapshot = call.read().unwrap_or_else(|p| p.into_inner()).clone();
                            let mut changed = snapshot
                                .thought_signature
                                .as_deref()
                                .is_some_and(|s| !s.is_empty());
                            let mut selected = snapshot;
                            if changed {
                                selected.thought_signature = None;
                            }
                            if let Some(normalize) = normalize_tool_call_id.as_deref_mut() {
                                let source = match source_message {
                                    Message::Assistant(source) => source,
                                    _ => unreachable!(),
                                };
                                let id = call.read().unwrap_or_else(|p| p.into_inner()).id.clone();
                                let normalized = normalize(&id, model, source);
                                let source_id =
                                    call.read().unwrap_or_else(|p| p.into_inner()).id.clone();
                                if normalized != source_id {
                                    ids.insert(source_id, normalized.clone());
                                    if !changed {
                                        selected =
                                            call.read().unwrap_or_else(|p| p.into_inner()).clone();
                                    }
                                    selected.id = normalized;
                                    changed = true;
                                }
                            }
                            if changed {
                                Some(AssistantContent::ToolCall(std::sync::Arc::new(
                                    std::sync::RwLock::new(selected),
                                )))
                            } else {
                                Some(block.clone())
                            }
                        }
                        _ => Some(block.clone()),
                    })
                    .collect();
            }
            if let Message::ToolResult(r) = &mut message
                && let Some(id) = ids
                    .get(&r.tool_call_id)
                    .filter(|id| !id.is_empty() && **id != r.tool_call_id)
            {
                r.tool_call_id = id.clone();
            }
            message
        })
        .collect();
    let mut result = Vec::new();
    let mut pending = Vec::new();
    let mut existing = std::collections::HashSet::new();
    for message in transformed {
        match &message {
            Message::Assistant(a) => {
                insert_synthetic_tool_results(&mut result, &mut pending, &mut existing);
                if matches!(
                    a.stop_reason,
                    crate::StopReason::Error | crate::StopReason::Aborted
                ) {
                    continue;
                }
                let calls: Vec<_> = a
                    .content
                    .iter()
                    .filter_map(|b| {
                        if let AssistantContent::ToolCall(c) = b {
                            Some(c.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !calls.is_empty() {
                    pending = calls;
                    existing.clear();
                }
            }
            Message::ToolResult(r) => {
                existing.insert(r.tool_call_id.clone());
            }
            Message::User(_) => {
                insert_synthetic_tool_results(&mut result, &mut pending, &mut existing)
            }
        }
        result.push(message);
    }
    insert_synthetic_tool_results(&mut result, &mut pending, &mut existing);
    result
}

fn synthetic(call: &crate::ToolCall) -> Message {
    Message::ToolResult(crate::ToolResultMessage {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        content: vec![crate::InputContent::Text(TextContent {
            text: "No result provided".into(),
            text_signature: None,
        })],
        details: None,
        is_error: true,
        timestamp: now(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn now() -> f64 {
    #[cfg(test)]
    if let Some(mut clock) = TEST_CLOCK.with(|clock| clock.borrow_mut().take()) {
        let value = clock();
        TEST_CLOCK.with(|slot| *slot.borrow_mut() = Some(clock));
        return value;
    }
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as f64,
        Err(error) => -(error.duration().as_millis() as f64),
    }
}
#[cfg(target_arch = "wasm32")]
fn now() -> f64 {
    js_sys::Date::now()
}

fn insert_synthetic_tool_results(
    result: &mut Vec<Message>,
    pending: &mut Vec<std::sync::Arc<std::sync::RwLock<crate::ToolCall>>>,
    existing: &mut std::collections::HashSet<String>,
) {
    if pending.is_empty() {
        return;
    }
    for call in pending.drain(..) {
        let call = call.read().unwrap_or_else(|p| p.into_inner()).clone();
        if !existing.contains(&call.id) {
            result.push(synthetic(&call));
        }
    }
    existing.clear();
}

const NON_VISION_USER_IMAGE_PLACEHOLDER: &str = "(image omitted: model does not support images)";
const NON_VISION_TOOL_IMAGE_PLACEHOLDER: &str =
    "(tool image omitted: model does not support images)";

fn replace_images_with_placeholder(
    content: &[crate::InputContent],
    placeholder: &str,
) -> Vec<crate::InputContent> {
    let mut result = Vec::new();
    let mut previous_was_placeholder = false;
    for block in content {
        match block {
            crate::InputContent::Image(_) => {
                if !previous_was_placeholder {
                    result.push(crate::InputContent::Text(TextContent {
                        text: placeholder.into(),
                        text_signature: None,
                    }));
                }
                previous_was_placeholder = true;
            }
            crate::InputContent::Text(text) => {
                result.push(block.clone());
                previous_was_placeholder = text.text == placeholder;
            }
        }
    }
    result
}
fn downgrade_unsupported_images(messages: &[Message], model: &Model) -> Vec<Message> {
    if model.input.iter().any(|input| input == "image") {
        return messages.to_vec();
    }
    messages
        .iter()
        .map(|message| {
            let mut message = message.clone();
            match &mut message {
                Message::User(u) => {
                    if let crate::UserContent::Blocks(content) = &u.content {
                        u.content = crate::UserContent::Blocks(replace_images_with_placeholder(
                            content,
                            NON_VISION_USER_IMAGE_PLACEHOLDER,
                        ));
                    }
                }
                Message::ToolResult(r) => {
                    r.content = replace_images_with_placeholder(
                        &r.content,
                        NON_VISION_TOOL_IMAGE_PLACEHOLDER,
                    )
                }
                _ => {}
            }
            message
        })
        .collect()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
thread_local! {
    static TEST_CLOCK: std::cell::RefCell<Option<Box<dyn FnMut() -> f64>>> = const { std::cell::RefCell::new(None) };
}
#[cfg(all(test, not(target_arch = "wasm32")))]
struct ClockOverride(Option<Box<dyn FnMut() -> f64>>);
#[cfg(all(test, not(target_arch = "wasm32")))]
impl ClockOverride {
    fn new(clock: Box<dyn FnMut() -> f64>) -> Self {
        Self(TEST_CLOCK.with(|slot| slot.replace(Some(clock))))
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
impl Drop for ClockOverride {
    fn drop(&mut self) {
        let clock = TEST_CLOCK.with(|slot| slot.replace(self.0.take()));
        drop(clock);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn fixture() -> (Vec<Message>, Model) {
        let model = serde_json::from_value(json!({"id":"target","name":"Fixture","api":"api","provider":"provider","baseUrl":"fixture:","reasoning":true,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":0,"maxTokens":0})).unwrap();
        let messages = serde_json::from_value(json!([{"role":"assistant","api":"api","provider":"provider","model":"foreign","content":[{"type":"toolCall","id":"a","name":"lookup","arguments":{}},{"type":"toolCall","id":"b","name":"lookup","arguments":{}},{"type":"toolCall","id":"c","name":"lookup","arguments":{}}],"stopReason":"toolUse","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"timestamp":1}])).unwrap();
        (messages, model)
    }

    #[test]
    fn samples_each_missing_result_after_all_normalizers() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let clock_events = events.clone();
        let reads = Rc::new(Cell::new(0));
        let clock_reads = reads.clone();
        let _scope = ClockOverride::new(Box::new(move || {
            clock_events.borrow_mut().push("clock");
            clock_reads.set(clock_reads.get() + 1);
            100.0 + clock_reads.get() as f64
        }));
        let (messages, model) = fixture();
        let output = crate::transform_messages(
            &messages,
            &model,
            Some(&mut |id, _, _| {
                events.borrow_mut().push("normalize");
                id.into()
            }),
        );
        let times: Vec<_> = output
            .iter()
            .filter_map(|m| {
                if let Message::ToolResult(r) = m {
                    Some(r.timestamp)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(times, vec![101.0, 102.0, 103.0]);
        assert_eq!(reads.get(), 3);
        assert_eq!(
            *events.borrow(),
            vec![
                "normalize",
                "normalize",
                "normalize",
                "clock",
                "clock",
                "clock"
            ]
        );
        events.borrow_mut().clear();
        reads.set(0);
        assert_eq!(crate::transform_messages(&output, &model, None), output);
        assert_eq!(reads.get(), 0);
        assert!(events.borrow().is_empty());
    }
    #[test]
    fn clock_override_is_restored_after_scope() {
        assert!(TEST_CLOCK.with(|slot| slot.borrow().is_none()));
        {
            let _outer = ClockOverride::new(Box::new(|| 7.0));
            assert_eq!(now(), 7.0);
            {
                let _inner = ClockOverride::new(Box::new(|| 8.0));
                assert_eq!(now(), 8.0);
            }
            assert_eq!(now(), 7.0);
            let panic = std::panic::catch_unwind(|| {
                let _inner = ClockOverride::new(Box::new(|| 9.0));
                assert_eq!(now(), 9.0);
                panic!("controlled unwind");
            });
            assert!(panic.is_err());
            assert_eq!(now(), 7.0);
        }
        assert!(TEST_CLOCK.with(|slot| slot.borrow().is_none()));
    }
}
