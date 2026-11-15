//! Continuations retain actual generated assistants and tool identities.

use super::{Context, Input, Row, TestResult, Wire, canonical, invocation, without_clocks};
use maestro_models::{
    AssistantContent, AssistantMessage, Message, StopReason, TextContent, ToolCall, UserBlock,
};
use std::collections::HashMap;

/// Actual outputs retained only within one test's operation sequence.
pub(super) type Results = HashMap<String, AssistantMessage>;

/// Resolve recorded prerequisites through public dispatch, then assemble the caller history.
pub(super) fn context<'a>(
    row: &'a Row,
    wire: &'a Wire,
    all: &'a [Row],
    results: &'a mut Results,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = TestResult<Context>> + 'a>> {
    Box::pin(async move {
        let mut context = crate::chat::context(&wire.context)?;
        let mut ids = HashMap::new();
        for message in &mut context.messages {
            fill_message(row, message, all, results, &mut ids).await?;
        }
        assert!(
            canonical(without_clocks(serde_json::to_value(&context)?))
                == canonical(wire.context.clone()),
            "{} generated input history",
            row.id
        );
        Ok(context)
    })
}

/// Fill a generated assistant or its following result from this operation sequence.
async fn fill_message(
    row: &Row,
    message: &mut Message,
    all: &[Row],
    results: &mut Results,
    ids: &mut HashMap<String, ToolCall>,
) -> TestResult {
    match message {
        Message::Assistant(assistant) => {
            let Some(id) = assistant.response_id.clone() else {
                return Ok(());
            };
            let generated = generate(&id, all, results).await?;
            remember_ids(assistant, &generated, ids);
            *assistant = generated;
            if row.id == "replay_aborted" {
                assistant
                    .content
                    .retain(|block| matches!(block, AssistantContent::Thinking(_)));
                assistant.stop_reason = StopReason::Aborted;
            }
        }
        Message::ToolResult(result) => {
            if let Some(call) = ids.get(&result.tool_call_id) {
                result.tool_call_id.clone_from(&call.id);
                if row.test.contains("model_streaming/") {
                    result.tool_name.clone_from(&call.name);
                    result.content = vec![UserBlock::Text(TextContent::new(arithmetic(call)?))];
                }
            }
        }
        Message::User(_) => {}
    }
    Ok(())
}

/// One tool result binds to the actual returned call rather than a fabricated identifier.
fn remember_ids(
    expected: &AssistantMessage,
    actual: &AssistantMessage,
    ids: &mut HashMap<String, ToolCall>,
) {
    let calls = |message: &AssistantMessage| {
        message
            .content
            .iter()
            .filter_map(|block| {
                if let AssistantContent::ToolCall(call) = block {
                    Some(call.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    for (old, new) in calls(expected).into_iter().zip(calls(actual)) {
        ids.insert(old.id, new);
    }
}

/// Reuse a result from this test or invoke its source row, never its expected assistant.
async fn generate(id: &str, all: &[Row], results: &mut Results) -> TestResult<AssistantMessage> {
    if let Some(result) = results.get(id) {
        return Ok(result.clone());
    }
    let source_id = id
        .strip_prefix("response_")
        .ok_or("recorded response identity")?;
    let source = all
        .iter()
        .find(|row| row.id == source_id)
        .ok_or("prerequisite row")?;
    let Input::Wire(wire) = &source.input else {
        return Err("prerequisite invocation".into());
    };
    let context = context(source, wire, all, results).await?;
    let result = invocation::invoke(source, wire, context).await?;
    let message = result.read().map_err(|_| "result poisoned")?.clone();
    results.insert(id.into(), message.clone());
    Ok(message)
}

/// Compute witnessed addition or multiplication from the actual returned call operands.
fn arithmetic(call: &ToolCall) -> TestResult<String> {
    assert_eq!(call.name, "math_operation", "selected arithmetic tool");
    let a = call.arguments["a"].as_f64().ok_or("first operand")?;
    let b = call.arguments["b"].as_f64().ok_or("second operand")?;
    let result = match call.arguments["operation"].as_str().ok_or("operation")? {
        "add" => a + b,
        "multiply" => a * b,
        _ => return Err("unknown arithmetic operation".into()),
    };
    Ok(result.to_string())
}
