//! Prepared tool arguments as the agent loop hands them to execution.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use std::sync::{Arc, Mutex, RwLock};

use crate::support;
use maestro_agent::{AgentTool, AgentToolResult, run_agent_loop_continue};
use maestro_models::{AssistantContent, ParameterSchema, SchemaKind, Tool, ToolCall};

/// A schema declaring one authored and one plain member.
fn mixed_schema() -> ParameterSchema {
    ParameterSchema::authored(
        serde_json::json!({
            "type": "object",
            "required": ["authored", "plain"],
            "properties": {
                "authored": {"type": "string"},
                "plain": {"type": "string"},
            },
        }),
        [
            (String::new(), SchemaKind::Object),
            ("/properties/authored".to_owned(), SchemaKind::String),
        ],
    )
}

/// Drive one loop turn and return the arguments execution received.
async fn route(with_preparation: bool) -> Vec<serde_json::Value> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&seen);
    let call = support::assistant("call");
    call.write().expect("assistant").content = vec![AssistantContent::ToolCall(ToolCall {
        id: "id".into(),
        name: "echo".into(),
        arguments: serde_json::from_value(serde_json::json!({
            "authored": null,
            "plain": null,
        }))
        .expect("recorded arguments"),
        thought_signature: None,
    })];
    let tool = AgentTool {
        description: None,
        definition: Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: mixed_schema(),
        },
        label: "Echo".into(),
        execution_mode: None,
        prepare_arguments: with_preparation
            .then(|| Arc::new(|value: &mut serde_json::Value| Ok(value.clone())) as _),
        execute: Arc::new(move |_, args, _, _| {
            observed.lock().expect("arguments").push(args);
            Box::pin(async {
                Ok(AgentToolResult {
                    content: Vec::new(),
                    details: Some(serde_json::Value::Null),
                    terminate: Some(true),
                })
            })
        }),
    };
    let mut context = support::context();
    context.tools = Some(Arc::new(RwLock::new(vec![Arc::new(RwLock::new(tool))])));
    let emit = Arc::new(|event| {
        if let maestro_agent::AgentEvent::ToolExecutionStart { tool_call }
        | maestro_agent::AgentEvent::ToolExecutionEnd { tool_call, .. } = event
        {
            assert_eq!(tool_call.arguments["authored"], serde_json::Value::Null);
            assert_eq!(tool_call.arguments["plain"], serde_json::Value::Null);
        }
        Box::pin(async { Ok(()) }) as maestro_models::BoxFuture<_>
    });
    run_agent_loop_continue(
        context,
        &support::config(),
        emit,
        support::responses(vec![call]),
    )
    .await
    .expect("one loop turn");
    let arguments = seen.lock().expect("arguments").clone();
    assert_eq!(arguments.len(), 1, "one execution");
    arguments
}

#[test]
fn agent_execution_receives_prepared_mixed_arguments() {
    for with_preparation in [false, true] {
        let arguments = support::run(route(with_preparation));
        assert_eq!(arguments[0]["authored"], "null", "authored member");
        assert_eq!(arguments[0]["plain"], "", "plain member");
    }
}
