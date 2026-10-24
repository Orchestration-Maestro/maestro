//! A controlled agent loop: plain user entries, standard history conversion and
//! queued model responses, shared with other crates' loop tests.
use crate::{AgentLoopConfig, AgentLoopOptions, AgentMessage};
use maestro_models::{
    AssistantMessageEventStream, Context, JsonObject, Message, Model, ProviderObjects,
    SharedAssistantMessage, SimpleStreamOptions, UserContent, UserMessage,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
};

/// Construct a shared user entry.
#[must_use]
pub fn user(text: &str) -> AgentMessage {
    AgentMessage::User(Arc::new(RwLock::new(UserMessage {
        content: UserContent::Text(text.into()),
        timestamp: 1.0,
    })))
}
/// Convert ordinary records at the model boundary for `model`.
///
/// # Panics
/// Panics if a retained lock is poisoned or conversion encounters a custom entry.
#[must_use]
pub fn config(model: Model) -> AgentLoopConfig {
    AgentLoopConfig {
        thinking_budgets: None,
        model: Arc::new(RwLock::new(model)),
        options: SimpleStreamOptions::default(),
        extra: JsonObject::new(),
        objects: ProviderObjects::default(),
        convert_to_llm: Arc::new(|history| {
            let messages = history
                .read()
                .map(|history| history.iter().filter_map(converted).collect::<Vec<_>>());
            assert!(messages.is_ok());
            let messages = messages.unwrap_or_default();
            Box::pin(async { Ok(messages) })
        }),
        transform_context: None,
        get_api_key: None,
        before_tool_call: None,
        after_tool_call: None,
        should_stop_after_turn: None,
        get_steering_messages: None,
        get_follow_up_messages: None,
        tool_execution: Some(crate::ToolExecutionMode::Sequential),
    }
}
/// Publish one authoritative result for each requested turn.
///
/// # Panics
/// Panics if the controlled response queue is poisoned or exhausted.
#[must_use]
pub fn responses(messages: Vec<SharedAssistantMessage>) -> AgentLoopOptions {
    let messages = Arc::new(Mutex::new(VecDeque::from(messages)));
    AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |_, _: Context, _, _| {
            let message = messages.lock().map(|mut messages| messages.pop_front());
            assert!(message.is_ok());
            let message = message.ok().flatten();
            assert!(message.is_some(), "unexpected turn");
            Box::pin(async {
                let stream = AssistantMessageEventStream::new();
                stream.end(message);
                Ok(stream)
            })
        })),
    }
}

/// Convert one ordinary retained record without delaying a failed read.
fn converted(message: &AgentMessage) -> Option<Message> {
    assert!(!matches!(message, AgentMessage::Custom(_)));
    let result = match message {
        AgentMessage::User(value) => value
            .read()
            .map(|value| Message::User(value.clone()))
            .map_err(|_| ()),
        AgentMessage::Assistant(value) => value
            .read()
            .map(|value| Message::Assistant(value.clone()))
            .map_err(|_| ()),
        AgentMessage::ToolResult(value) => value
            .read()
            .map(|value| Message::ToolResult(value.clone()))
            .map_err(|_| ()),
        AgentMessage::Custom(_) => return None,
    };
    assert!(result.is_ok());
    result.ok()
}
