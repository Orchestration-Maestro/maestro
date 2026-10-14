//! Shared response output identity observations.
use crate::chat::TestResult;
use maestro_models::{AssistantMessageEvent, AssistantMessageEventStream};
use serde_json::Value;
use std::sync::Arc;
/// Observe shared message identity across every update and the retained result.
pub async fn collect(stream: &AssistantMessageEventStream) -> TestResult<(Vec<Value>, Value)> {
    let mut events = Vec::new();
    let mut aliases = Vec::new();
    while let Some(event) = stream.next().await {
        match &event {
            AssistantMessageEvent::Start { partial }
            | AssistantMessageEvent::TextStart { partial, .. }
            | AssistantMessageEvent::TextDelta { partial, .. }
            | AssistantMessageEvent::TextEnd { partial, .. }
            | AssistantMessageEvent::ThinkingStart { partial, .. }
            | AssistantMessageEvent::ThinkingDelta { partial, .. }
            | AssistantMessageEvent::ThinkingEnd { partial, .. }
            | AssistantMessageEvent::ToolcallStart { partial, .. }
            | AssistantMessageEvent::ToolcallDelta { partial, .. }
            | AssistantMessageEvent::ToolcallEnd { partial, .. } => {
                aliases.push(Arc::clone(partial));
            }
            AssistantMessageEvent::Done { message, .. } => aliases.push(Arc::clone(message)),
            AssistantMessageEvent::Error { error, .. } => aliases.push(Arc::clone(error)),
        }
        let mut value = serde_json::to_value(&event)?;
        for key in ["partial", "message", "error"] {
            value.as_object_mut().ok_or("event")?.shift_remove(key);
        }
        events.push(value);
    }
    let result = stream.result().await;
    assert!(aliases.iter().all(|alias| Arc::ptr_eq(alias, &result)));
    let snapshot = result.read().map_err(|e| e.to_string())?.clone();
    let mut result = serde_json::to_value(&snapshot)?;
    result
        .as_object_mut()
        .ok_or("result")?
        .shift_remove("timestamp");
    Ok((events, result))
}
