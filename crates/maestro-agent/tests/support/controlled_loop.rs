//! A controlled agent loop: plain user entries, standard history conversion and
//! queued model responses, shared with other crates' loop tests.
use maestro_agent::{AgentLoopConfig, AgentLoopOptions, AgentMessage, CustomAgentMessages};
use maestro_models::{
    AssistantMessageEventStream, Context, JsonObject, Message, Model, ProviderObjects,
    SharedAssistantMessage, SimpleStreamOptions, UserContent, UserMessage,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
};

/// Construct a shared user entry of a conversation whose custom entries are `C`.
#[must_use]
pub fn typed_user<C: CustomAgentMessages>(text: &str) -> AgentMessage<C> {
    AgentMessage::User(Arc::new(RwLock::new(UserMessage {
        content: UserContent::Text(text.into()),
        timestamp: 1.0,
    })))
}
/// Construct a shared user entry.
#[must_use]
pub fn user(text: &str) -> AgentMessage {
    typed_user(text)
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
                .unwrap()
                .iter()
                .map(|message| match message {
                    AgentMessage::User(value) => Message::User(value.read().unwrap().clone()),
                    AgentMessage::Assistant(value) => {
                        Message::Assistant(value.read().unwrap().clone())
                    }
                    AgentMessage::ToolResult(value) => {
                        Message::ToolResult(value.read().unwrap().clone())
                    }
                    AgentMessage::Custom(_) => unreachable!(),
                })
                .collect();
            Box::pin(async { Ok(messages) })
        }),
        transform_context: None,
        get_api_key: None,
        before_tool_call: None,
        after_tool_call: None,
        should_stop_after_turn: None,
        prepare_next_turn: None,
        get_steering_messages: None,
        get_follow_up_messages: None,
        tool_execution: Some(maestro_agent::ToolExecutionMode::Sequential),
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
            let message = messages
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected turn");
            Box::pin(async {
                let stream = AssistantMessageEventStream::new();
                stream.end(Some(message));
                Ok(stream)
            })
        })),
    }
}
