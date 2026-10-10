//! Controlled model responses for public-operation tests.
use maestro_agent::{AgentContext, AgentLoopConfig, AgentLoopOptions, AgentMessage};
use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEventStream, Context, JsonObject, Message,
    Model, ModelCost, ProviderObjects, SharedAssistantMessage, SimpleStreamOptions, StopReason,
    TextContent, UserContent, UserMessage,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
};

/// Construct a shared user entry.
pub fn user(text: &str) -> AgentMessage {
    AgentMessage::User(Arc::new(RwLock::new(UserMessage {
        content: UserContent::Text(text.into()),
        timestamp: 1.0,
    })))
}
/// Supply retained history and instructions.
pub fn context() -> AgentContext {
    AgentContext {
        system_prompt: "instructions".into(),
        messages: Arc::new(RwLock::new(vec![user("earlier")])),
        tools: None,
    }
}
/// Supply a descriptor outside the catalog.
pub fn model() -> Model {
    Model {
        id: "fixture".into(),
        name: "Fixture".into(),
        api: "fixture".into(),
        provider: "fixture".into(),
        base_url: "https://example.invalid".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![],
        cost: ModelCost::default(),
        context_window: 100.0,
        max_tokens: 20.0,
        headers: None,
        compat: None,
    }
}
/// Convert ordinary records at the model boundary.
pub fn config() -> AgentLoopConfig {
    AgentLoopConfig {
        model: model(),
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
    }
}
/// Construct a completed text response.
pub fn assistant(text: &str) -> SharedAssistantMessage {
    Arc::new(RwLock::new(AssistantMessage { content: vec![AssistantContent::Text(TextContent { text: text.into(), text_signature: None })], api: "fixture".into(), provider: "fixture".into(), model: "fixture".into(), response_model: None, response_id: None, diagnostics: None, usage: serde_json::from_value(serde_json::json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}})).unwrap(), stop_reason: StopReason::Stop, error_message: None, timestamp: 2.0 }))
}
/// Publish one authoritative result for each requested turn.
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
/// Execute local futures on the test-owned runtime.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}
/// A completed callback future without an extra nested async block.
pub fn ready<T: Send + 'static>(value: T) -> maestro_models::BoxFuture<T> {
    Box::pin(std::future::ready(value))
}
/// Source-recorded operation scenarios.
pub mod corpus;
/// Construct a single echo call without schema operands.
pub fn tool_call() -> SharedAssistantMessage {
    let message = assistant("call");
    message.write().unwrap().content = vec![maestro_models::AssistantContent::ToolCall(
        maestro_models::ToolCall {
            id: "id".into(),
            name: "echo".into(),
            arguments: maestro_models::JsonObject::new(),
            thought_signature: None,
        },
    )];
    message
}
/// Retain an executable echo callback in a complete conversation input.
pub fn with_execute(execute: maestro_agent::ExecuteTool) -> AgentContext {
    let mut context = context();
    context.tools = Some(Arc::new(RwLock::new(vec![Arc::new(RwLock::new(
        maestro_agent::AgentTool {
            definition: maestro_models::Tool {
                name: "echo".into(),
                description: "Echo".into(),
                parameters: serde_json::json!({"type":"object"}),
            },
            label: "Echo".into(),
            prepare_arguments: None,
            execute,
            execution_mode: None,
        },
    ))])));
    context
}
/// Return a completed terminating artifact for controlled execution.
pub fn result() -> maestro_agent::AgentToolResult {
    maestro_agent::AgentToolResult {
        content: vec![],
        details: serde_json::Value::Null,
        terminate: Some(true),
    }
}
