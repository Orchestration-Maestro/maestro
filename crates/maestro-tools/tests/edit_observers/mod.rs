//! Observe normalized calls separately from execution arguments through the loop.
#![cfg(test)]
use crate::read_support::Fixture;
use crate::runtime;
use crate::{loop_support, read_support};
use maestro_agent::{AgentContext, AgentEvent, ToolExecutionMode, run_agent_loop};
use maestro_tools::{EditToolOptions, create_edit_tool};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};

/// Check normalization and legacy folding in hooks, progress and retained history.
pub fn observe(mode: ToolExecutionMode) {
    for input in [
        json!({"path":"file","edits":"[{\"oldText\":\"hello\",\"newText\":\"world\"}]"}),
        json!({"path":"file","oldText":"hello","newText":"world"}),
    ] {
        let dir = read_support::Scratch::new().unwrap();
        let path = dir.file("file", b"hello");
        let mut normalized = input.clone();
        if input.get("edits").is_some() {
            normalized["edits"] = json!([{"oldText":"hello","newText":"world"}]);
        }
        let observations = Arc::new(Mutex::new(Vec::new()));
        let config = observer_config(mode, &observations);
        let mut tool = create_edit_tool(dir.text(), EditToolOptions::default());
        let execute = Arc::clone(&tool.execute);
        tool.execute = Arc::new(move |id, args, signal, update| {
            assert_eq!(
                args,
                json!({"path":"file","edits":[{"oldText":"hello","newText":"world"}]})
            );
            let execution = execute(id, args, signal, None);
            Box::pin(async move {
                let result = execution.await?;
                update.unwrap()(maestro_agent::AgentToolResult {
                    content: result.content.clone(),
                    details: result.details.clone(),
                    terminate: result.terminate,
                });
                Ok(result)
            })
        });
        let progress = Arc::clone(&observations);
        let emit = Arc::new(move |event| -> maestro_models::BoxFuture<_> {
            if let AgentEvent::ToolExecutionUpdate { tool_call, .. } = event {
                progress
                    .lock()
                    .unwrap()
                    .push(("progress", tool_call.arguments));
            }
            Box::pin(async { Ok(()) })
        });
        run(&config, tool, input, emit, &normalized);
        assert_eq!(std::fs::read(path).unwrap(), b"world");
        assert_eq!(
            *observations.lock().unwrap(),
            vec![
                ("before", normalized.clone()),
                ("progress", normalized.clone()),
                ("after", normalized)
            ]
        );
    }
}

/// Record the two hook observations without changing either argument value.
fn observer_config(
    mode: ToolExecutionMode,
    observations: &Arc<Mutex<Vec<(&'static str, Value)>>>,
) -> maestro_agent::AgentLoopConfig {
    let mut config = loop_support::config();
    config.tool_execution = Some(mode);
    let before = Arc::clone(observations);
    config.before_tool_call = Some(Arc::new(move |context, _| {
        before
            .lock()
            .unwrap()
            .push(("before", context.tool_call.arguments.clone()));
        Box::pin(async { Ok(None) })
    }));
    let after = Arc::clone(observations);
    config.after_tool_call = Some(Arc::new(move |context, _| {
        after
            .lock()
            .unwrap()
            .push(("after", context.tool_call.arguments.clone()));
        Box::pin(async { Ok(None) })
    }));
    config
}

/// Finish the public loop before checking its retained assistant call.
fn run(
    config: &maestro_agent::AgentLoopConfig,
    tool: maestro_agent::AgentTool,
    input: Value,
    emit: maestro_agent::AgentEventSink,
    normalized: &Value,
) {
    let requested = loop_support::assistant("call", &[("edit-id", "edit", input)]);
    let context = AgentContext {
        system_prompt: "instructions".into(),
        messages: Arc::new(RwLock::new(Vec::new())),
        tools: Some(Arc::new(RwLock::new(vec![Arc::new(RwLock::new(tool))]))),
    };
    runtime::run(run_agent_loop(
        vec![loop_support::user("go")],
        context,
        config,
        emit,
        loop_support::responses(vec![
            Arc::clone(&requested),
            loop_support::assistant("done", &[]),
        ]),
    ))
    .unwrap();
    let message = requested.read().unwrap();
    let maestro_models::AssistantContent::ToolCall(call) = &message.content[1] else {
        panic!("missing retained call");
    };
    assert_eq!(call.arguments, *normalized);
}
