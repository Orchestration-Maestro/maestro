//! Shared response output identity observations.
use crate::chat::TestResult;
use maestro_models::{AssistantMessageEvent, AssistantMessageEventStream};
use serde_json::Value;
use std::sync::Arc;
/// Observe the same shared handle at Start, terminal event and result.
pub async fn collect(stream: &AssistantMessageEventStream) -> TestResult<(Vec<Value>, Value)> {
    let mut events = Vec::new();
    let mut aliases = Vec::new();
    while let Some(event) = stream.next().await {
        match &event {
            AssistantMessageEvent::Start { partial } => aliases.push(Arc::clone(partial)),
            AssistantMessageEvent::Done { message, .. } => aliases.push(Arc::clone(message)),
            AssistantMessageEvent::Error { error, .. } => aliases.push(Arc::clone(error)),
            _ => {}
        }
        let mut value = serde_json::to_value(&event)?;
        for key in ["partial", "message", "error"] {
            value.as_object_mut().ok_or("event")?.remove(key);
        }
        events.push(value);
    }
    let result = stream.result().await;
    assert!(aliases.iter().all(|alias| Arc::ptr_eq(alias, &result)));
    let snapshot = result.read().map_err(|e| e.to_string())?.clone();
    let mut result = serde_json::to_value(&snapshot)?;
    result.as_object_mut().ok_or("result")?.remove("timestamp");
    Ok((events, result))
}
