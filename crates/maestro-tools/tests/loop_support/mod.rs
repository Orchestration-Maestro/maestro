//! A deterministic agent loop that runs tools for the wrapped-tool tests.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

#[path = "../../../maestro-agent/tests/support/controlled_loop.rs"]
mod controlled_loop;
use crate::read_support::model;
pub use controlled_loop::{responses, user};
use maestro_agent::{
    AgentContext, AgentEvent, AgentLoopConfig, AgentLoopOptions, AgentMessage, AgentTool,
    run_agent_loop,
};
use maestro_models::{
    AssistantContent, AssistantMessage, Message, SharedAssistantMessage, StopReason, TextContent,
    ToolCall,
};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicUsize, Ordering},
};

/// A completed assistant response, optionally requesting `calls` as `(id, tool, arguments)`.
pub fn assistant(text: &str, calls: &[(&str, &str, Value)]) -> SharedAssistantMessage {
    let mut content = vec![AssistantContent::Text(TextContent::new(text.into()))];
    content.extend(calls.iter().map(|(id, name, arguments)| {
        AssistantContent::ToolCall(ToolCall {
            id: (*id).into(),
            name: (*name).into(),
            arguments: arguments.clone(),
            thought_signature: None,
        })
    }));
    Arc::new(RwLock::new(AssistantMessage {
        content,
        api: "fixture".into(),
        provider: "fixture".into(),
        model: "fixture".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: serde_json::from_value(json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}})).unwrap(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 2.0,
    }))
}

/// A loop configuration that converts history without a provider.
pub fn config() -> AgentLoopConfig {
    controlled_loop::config(model(vec![]).as_ref().clone())
}

/// The tool-result messages one turn produced, as the model receives them, and its progress events.
pub struct Turn {
    /// Tool-result messages in call order.
    pub results: Vec<Value>,
    /// Progress events emitted by tools.
    pub updates: usize,
    /// Tool arguments retained by the assistant message after execution.
    pub arguments: Vec<Value>,
    /// Retained tool arguments replayed into the next model request.
    pub request_arguments: Vec<Value>,
}

/// Run one turn of tool calls, given as `(id, tool, arguments)`, through the raw loop.
pub fn run_turn(tools: Vec<AgentTool>, calls: &[(&str, &str, Value)]) -> Turn {
    let context = AgentContext {
        system_prompt: "instructions".into(),
        messages: Arc::new(RwLock::new(Vec::new())),
        tools: Some(Arc::new(RwLock::new(
            tools
                .into_iter()
                .map(|tool| Arc::new(RwLock::new(tool)))
                .collect(),
        ))),
    };
    let updates = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&updates);
    let emit = Arc::new(move |event: AgentEvent| -> maestro_models::BoxFuture<_> {
        if matches!(event, AgentEvent::ToolExecutionUpdate { .. }) {
            counter.fetch_add(1, Ordering::SeqCst);
        }
        Box::pin(std::future::ready(Ok(())))
    });
    let requested = assistant("call", calls);
    let replay = Arc::new(Mutex::new(Vec::new()));
    let options = replay_responses(
        vec![Arc::clone(&requested), assistant("done", &[])],
        Arc::clone(&replay),
    );
    let returned = crate::runtime::run(run_agent_loop(
        vec![user("go")],
        context,
        &config(),
        emit,
        options,
    ))
    .unwrap();
    let results = returned
        .iter()
        .filter_map(|message| match message {
            AgentMessage::ToolResult(message) => {
                Some(serde_json::to_value(&*message.read().unwrap()).unwrap())
            }
            _ => None,
        })
        .collect();
    Turn {
        request_arguments: replay.lock().unwrap().clone(),
        results,
        updates: updates.load(Ordering::SeqCst),
        arguments: call_arguments(&requested.read().unwrap().content),
    }
}

/// Tool argument values in content order.
fn call_arguments(content: &[AssistantContent]) -> Vec<Value> {
    content
        .iter()
        .filter_map(|block| match block {
            AssistantContent::ToolCall(call) => Some(call.arguments.clone()),
            _ => None,
        })
        .collect()
}

/// Capture assistant call arguments delivered to each controlled model request.
fn replay_responses(
    messages: Vec<SharedAssistantMessage>,
    captured: Arc<Mutex<Vec<Value>>>,
) -> AgentLoopOptions {
    let mut options = responses(messages);
    let stream = options.stream_fn.take().unwrap();
    options.stream_fn = Some(Arc::new(move |model, context, options, objects| {
        let arguments = context
            .messages
            .iter()
            .filter_map(|message| match message {
                Message::Assistant(message) => Some(call_arguments(&message.content)),
                _ => None,
            })
            .flatten();
        captured.lock().unwrap().extend(arguments);
        stream(model, context, options, objects)
    }));
    options
}
