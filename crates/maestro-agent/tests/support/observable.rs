//! Controlled execution and observations of raw streams.
#![cfg(test)]
use maestro_agent::{
    AgentContext, AgentEvent, AgentLoopConfig, AgentLoopOptions, AgentMessage, CustomAgentMessages,
    agent_loop,
};
use maestro_models::EventStream;
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Waker},
};

/// Ordered events and terminal entries from one completed invocation.
pub struct Recording<C: CustomAgentMessages = maestro_agent::NoCustomMessages> {
    /// Unmodified event payloads.
    pub events: Vec<AgentEvent<C>>,
    /// Retained terminal entries.
    pub messages: Vec<AgentMessage<C>>,
}
/// Drive admitted work without requiring a consumer to keep pace.
#[must_use]
pub fn observe<C: CustomAgentMessages + Send + Sync + 'static>(
    prompts: Vec<AgentMessage<C>>,
    context: AgentContext<C>,
    config: AgentLoopConfig<C>,
    options: AgentLoopOptions,
) -> Recording<C> {
    let tasks = Arc::new(super::owned::Tasks::default());
    let stream = agent_loop(prompts, context, Arc::new(config), options, tasks.spawner());
    super::run(tasks.take());
    drain(&stream)
}
/// Inspect a completed stream without transforming its payloads.
#[must_use]
pub fn drain<C: CustomAgentMessages + 'static>(
    stream: &EventStream<AgentEvent<C>, Vec<AgentMessage<C>>>,
) -> Recording<C> {
    let messages = super::run(stream.result());
    let mut events = Vec::new();
    while let Some(event) = super::run(stream.next()) {
        events.push(event);
    }
    Recording { events, messages }
}
/// Poll one operation at a deterministic suspension boundary.
pub fn poll<T>(future: std::pin::Pin<&mut (impl Future<Output = T> + ?Sized)>) -> Poll<T> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}
/// Select role tags without losing sequence or repeated occurrences.
///
/// # Panics
/// Panics on a custom entry.
#[must_use]
pub fn roles(messages: &[AgentMessage]) -> Vec<&'static str> {
    messages
        .iter()
        .map(|m| match m {
            AgentMessage::User(_) => "user",
            AgentMessage::Assistant(_) => "assistant",
            AgentMessage::ToolResult(_) => "toolResult",
            AgentMessage::Custom(_) => unreachable!(),
        })
        .collect()
}
/// Identify variants while retaining the original payloads separately.
#[must_use]
pub fn kind<C: CustomAgentMessages>(event: &AgentEvent<C>) -> &'static str {
    match event {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_execution_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
    }
}
/// Read an ordinary entry's text for assertions.
///
/// # Panics
/// Panics on poisoned entries or nontext content.
#[must_use]
pub fn text(message: &AgentMessage) -> String {
    match message {
        AgentMessage::User(value) => match &value.read().unwrap().content {
            maestro_models::UserContent::Text(text) => text.clone(),
            maestro_models::UserContent::Blocks(_) => panic!("expected text user"),
        },
        AgentMessage::Assistant(value) => match &value.read().unwrap().content[0] {
            maestro_models::AssistantContent::Text(text) => text.text.clone(),
            _ => panic!("expected text assistant"),
        },
        AgentMessage::ToolResult(value) => match &value.read().unwrap().content[0] {
            maestro_models::UserBlock::Text(text) => text.text.clone(),
            maestro_models::UserBlock::Image(_) => panic!("expected text artifact"),
        },
        AgentMessage::Custom(_) => panic!("expected ordinary entry"),
    }
}
/// Construct a tool output with distinguishable content and details.
#[must_use]
pub fn artifact(text: &str, terminate: bool) -> maestro_agent::AgentToolResult {
    maestro_agent::AgentToolResult {
        content: vec![maestro_models::UserBlock::Text(
            maestro_models::TextContent::new(text.into()),
        )],
        details: Some(serde_json::json!({"output":text})),
        terminate: Some(terminate),
    }
}
/// Construct tool calls in authored order.
///
/// # Panics
/// Panics on poisoned response state or nonobject arguments.
#[must_use]
pub fn calls(
    names: &[&str],
    arguments: &serde_json::Value,
) -> maestro_models::SharedAssistantMessage {
    let message = super::assistant("calls");
    message.write().unwrap().content = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            maestro_models::AssistantContent::ToolCall(maestro_models::ToolCall {
                id: format!("tool-{}", index + 1),
                name: (*name).into(),
                arguments: serde_json::from_value(arguments.clone()).unwrap(),
                thought_signature: None,
            })
        })
        .collect();
    message
}
/// Supply an executable tool using the existing declaration records.
#[must_use]
pub fn tool(name: &str, execute: maestro_agent::ExecuteTool) -> maestro_agent::SharedAgentTool {
    Arc::new(std::sync::RwLock::new(maestro_agent::AgentTool {
        description: None,
        definition: maestro_models::Tool {
            name: name.into(),
            description: name.into(),
            parameters: serde_json::json!({"type":"object"}).into(),
        },
        label: name.into(),
        prepare_arguments: None,
        execute,
        execution_mode: None,
    }))
}
/// Supply a mutable executable inventory.
#[must_use]
pub fn tools(tools: Vec<maestro_agent::SharedAgentTool>) -> AgentContext {
    let mut context = super::context();
    context.tools = Some(Arc::new(std::sync::RwLock::new(tools)));
    context
}
