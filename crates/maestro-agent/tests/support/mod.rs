//! Controlled model responses for public-operation tests.
#![cfg(test)]
use maestro_agent::{AgentContext, AgentLoopConfig};
use maestro_models::{
    AssistantContent, AssistantMessage, Model, ModelCost, SharedAssistantMessage, StopReason,
    TextContent,
};
use std::sync::{Arc, RwLock};

/// Plain user entries, history conversion and queued responses shared with other crates.
mod controlled_loop;
pub use controlled_loop::{responses, typed_user, user};

/// Supply retained history and instructions.
#[must_use]
pub fn context() -> AgentContext {
    AgentContext {
        system_prompt: "instructions".into(),
        messages: Arc::new(RwLock::new(vec![user("earlier")])),
        tools: None,
    }
}
/// Supply a descriptor outside the catalog.
#[must_use]
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
/// The controlled loop configuration over the fixture model.
#[must_use]
pub fn config() -> AgentLoopConfig {
    controlled_loop::config(model())
}
/// Construct a completed text response.
///
/// # Panics
/// Panics if the fixed usage fixture is invalid.
#[must_use]
pub fn assistant(text: &str) -> SharedAssistantMessage {
    Arc::new(RwLock::new(AssistantMessage { content: vec![AssistantContent::Text(TextContent::new(text.into()))], api: "fixture".into(), provider: "fixture".into(), model: "fixture".into(), response_model: None, response_id: None, diagnostics: None, usage: serde_json::from_value(serde_json::json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}})).unwrap(), stop_reason: StopReason::Stop, error_message: None, timestamp: 2.0 }))
}
/// Execute local futures on the test-owned runtime.
///
/// # Panics
/// Panics if construction of the test runtime fails.
pub fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}
/// Execute local futures on a test-owned runtime with its I/O and time drivers enabled.
///
/// # Panics
/// Panics if construction of the test runtime fails.
pub fn run_io<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
/// Signal entry, then await the caller-owned release, when the callback holds the gate.
///
/// # Panics
/// Panics if either side of the gate was dropped.
pub async fn wait_gate(
    gate: Option<(
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Receiver<()>,
    )>,
) {
    if let Some((entered, released)) = gate {
        entered.send(()).unwrap();
        released.await.unwrap();
    }
}
/// A completed callback future without an extra nested async block.
#[must_use]
pub fn ready<T: Send + 'static>(value: T) -> maestro_models::BoxFuture<T> {
    Box::pin(std::future::ready(value))
}
/// Source-recorded operation scenarios.
pub mod corpus;
/// Construct a single echo call without schema operands.
///
/// # Panics
/// Panics if the retained response lock is poisoned.
#[must_use]
pub fn tool_call() -> SharedAssistantMessage {
    let message = assistant("call");
    message.write().unwrap().content = vec![maestro_models::AssistantContent::ToolCall(
        maestro_models::ToolCall {
            id: "id".into(),
            name: "echo".into(),
            arguments: maestro_models::JsonObject::new().into(),
            thought_signature: None,
        },
    )];
    message
}
/// Retain an executable echo callback in a complete conversation input.
#[must_use]
pub fn with_execute(execute: maestro_agent::ExecuteTool) -> AgentContext {
    let mut context = context();
    context.tools = Some(Arc::new(RwLock::new(vec![Arc::new(RwLock::new(
        maestro_agent::AgentTool {
            description: None,
            definition: maestro_models::Tool {
                name: "echo".into(),
                description: "Echo".into(),
                parameters: serde_json::json!({"type":"object"}).into(),
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
#[must_use]
pub fn result() -> maestro_agent::AgentToolResult {
    maestro_agent::AgentToolResult {
        content: vec![],
        details: Some(serde_json::Value::Null),
        terminate: Some(true),
    }
}
/// Recorded continuation and configuration queries.
pub mod continuation_corpus;
/// Caller-owned queued execution for lifecycle tests.
pub mod owned;
/// Recorded owned-run response and reduction evidence.
pub mod owner_corpus;
/// Controlled proxy transport and observations.
pub mod proxy;
/// Registered scripted-model fixtures.
pub mod scripted;
/// Test-only helper tools.
pub mod utils;

/// Native captured-run binding fixtures.
pub mod binding_corpus;
/// Controlled raw-stream execution and payload observations.
pub mod observable;
/// Native queue effect and retirement witnesses.
pub mod queue_effects;

/// Shared lossless event projections.
pub mod observations;
