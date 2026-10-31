//! Absent, null and valued tool details through typed, erased and loop paths.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::{support, tool_callbacks::completed};
use maestro_agent::{
    AfterToolCall, AfterToolCallResult, AgentEvent, AgentMessage, AgentTool, AgentToolResult,
    ExecuteTool, ToolExecutionMode, run_agent_loop, run_agent_loop_continue,
};
use maestro_models::{
    AssistantContent, DiagnosticErrorInfo, TextContent, Tool, ToolCall, ToolResultMessage,
    UserBlock,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use support::*;

/// A structured typed detail.
#[derive(Serialize)]
struct Record {
    /// Payload.
    n: u32,
}
/// A typed result for the call at `index`: absent, typed null, then a record.
fn typed_result(index: usize, content: &str, terminate: bool) -> AgentToolResult<Option<Record>> {
    let details = match index {
        0 => None,
        1 => Some(None),
        _ => Some(Some(Record { n: 7 })),
    };
    AgentToolResult {
        content: text(content),
        details,
        terminate: Some(terminate),
        usage: None,
    }
}
/// The erased detail every call index must end with.
fn expected_state(index: usize) -> Option<Value> {
    match index {
        0 => None,
        1 => Some(Value::Null),
        _ => Some(json!({"n":7})),
    }
}
/// Text content naming a result.
fn text(text: &str) -> Vec<UserBlock> {
    vec![UserBlock::Text(TextContent::new(text.into()))]
}
/// The call index encoded in an id such as `c2`.
fn index_of(id: &str) -> usize {
    id[1..].parse().unwrap()
}
/// A caller failure with exact text.
fn diagnostic(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::new(None, message)
}
/// A declaration for the typed callback tests.
fn definition() -> Tool {
    Tool {
        name: "echo".into(),
        description: String::new(),
        parameters: json!({"type":"object"}).into(),
    }
}
/// A typed tool whose details follow `typed_result` and which reports every state as progress when supplied an observer.
fn typed_execute() -> ExecuteTool<Value, Option<Record>> {
    Arc::new(|id, _, _, update| {
        if let Some(update) = update {
            for state in 0..3 {
                update(typed_result(state, &format!("partial-{state}"), true));
            }
        }
        Box::pin(async move { Ok(typed_result(index_of(&id), "final", false)) })
    })
}
/// The same behavior as `typed_execute`, written against the erased interface.
fn erased_execute() -> ExecuteTool {
    Arc::new(|id, _, _, update| {
        if let Some(update) = update {
            for state in 0..3 {
                update(AgentToolResult {
                    content: text(&format!("partial-{state}")),
                    details: expected_state(state),
                    terminate: Some(true),
                    usage: None,
                });
            }
        }
        ready(Ok(AgentToolResult {
            content: text("final"),
            details: expected_state(index_of(&id)),
            terminate: Some(false),
            usage: None,
        }))
    })
}

/// Typed tool results keep absent, null and valued details, including progress.
#[test]
fn typed_callback_preserves_missing_null_and_value_details() {
    let tool = AgentTool::typed(definition(), "Echo".into(), typed_execute());
    for index in 0..3 {
        let partials = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&partials);
        let result = completed((tool.execute)(
            format!("c{index}"),
            json!({}),
            None,
            Some(Arc::new(move |partial| {
                captured.lock().unwrap().push(partial);
            })),
        ))
        .unwrap();
        assert_eq!(result.details, expected_state(index));
        assert_eq!(result.terminate, Some(false));
        assert_eq!(
            serde_json::to_value(&result.content).unwrap(),
            json!([{"type":"text","text":"final"}])
        );
        let partials = partials.lock().unwrap();
        let seen: Vec<_> = partials.iter().map(|p| p.details.clone()).collect();
        assert_eq!(seen, (0..3).map(expected_state).collect::<Vec<_>>());
        assert!(partials.iter().all(|p| p.terminate == Some(true)));
        let contents: Vec<_> = partials
            .iter()
            .map(|p| serde_json::to_value(&p.content).unwrap())
            .collect();
        assert_eq!(
            contents,
            (0..3)
                .map(|n| json!([{"type":"text","text":format!("partial-{n}")}]))
                .collect::<Vec<_>>()
        );
    }
}

/// Which loop entry point runs the turn.
#[derive(Clone, Copy)]
enum Entry {
    /// Start from a prompt.
    Prompt,
    /// Continue retained history.
    Continue,
}
/// The selected tool-result observations: progress and end details, hook inputs, messages and turn results.
#[derive(Default)]
struct Seen {
    /// Progress details by call id, in emission order.
    updates: Vec<(String, Option<Value>)>,
    /// Execution-end details by call id.
    ends: Vec<(String, Option<Value>)>,
    /// Details the after-hook was given, by call id.
    hook_inputs: Vec<(String, Option<Value>)>,
    /// Serialized tool-result entries at message start and end.
    messages: Vec<(String, Value)>,
    /// Tool results published at turn end.
    turn_results: Vec<ToolResultMessage>,
}
/// An assistant requesting one echo call per id.
fn calls(ids: &[&str]) -> maestro_models::SharedAssistantMessage {
    let message = assistant("call");
    message.write().unwrap().content = ids
        .iter()
        .map(|id| {
            AssistantContent::ToolCall(ToolCall {
                id: (*id).into(),
                name: "echo".into(),
                arguments: maestro_models::JsonObject::new().into(),
                thought_signature: None,
            })
        })
        .collect();
    message
}
/// Run one tool turn and report the observations plus the tool-result entries it returned.
async fn drive(
    calls: maestro_models::SharedAssistantMessage,
    execute: ExecuteTool,
    after: Option<AfterToolCall<maestro_agent::NoCustomMessages>>,
    mode: ToolExecutionMode,
    entry: Entry,
) -> (Arc<Mutex<Seen>>, Vec<ToolResultMessage>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let observed = Arc::clone(&seen);
    let emit = Arc::new(move |event: AgentEvent| {
        let mut seen = observed.lock().unwrap();
        match event {
            AgentEvent::ToolExecutionUpdate {
                tool_call,
                partial_result,
            } => seen.updates.push((tool_call.id, partial_result.details)),
            AgentEvent::ToolExecutionEnd {
                tool_call, result, ..
            } => seen.ends.push((tool_call.id, result.details)),
            AgentEvent::MessageStart {
                message: AgentMessage::ToolResult(message),
            }
            | AgentEvent::MessageEnd {
                message: AgentMessage::ToolResult(message),
            } => {
                let message = message.read().unwrap();
                seen.messages.push((
                    message.tool_call_id.clone(),
                    serde_json::to_value(&*message).unwrap(),
                ));
            }
            AgentEvent::TurnEnd { tool_results, .. } => seen.turn_results.extend(tool_results),
            _ => {}
        }
        ready(Ok(()))
    });
    let mut config = config();
    config.tool_execution = Some(mode);
    config.after_tool_call = after;
    let context = with_execute(execute);
    let history = Arc::clone(&context.messages);
    let responses = responses(vec![calls, assistant("done")]);
    let returned = match entry {
        Entry::Prompt => run_agent_loop(vec![user("go")], context, &config, emit, responses).await,
        Entry::Continue => run_agent_loop_continue(context, &config, emit, responses).await,
    }
    .unwrap();
    let results = |messages: &[AgentMessage]| {
        messages
            .iter()
            .filter_map(|message| match message {
                AgentMessage::ToolResult(message) => Some(message.read().unwrap().clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let results_returned = results(&returned);
    if matches!(entry, Entry::Continue) {
        assert_eq!(results(&history.read().unwrap()), results_returned);
    }
    (seen, results_returned)
}
/// Assert call `index` kept its state at every observation point.
fn assert_call_states(seen: &Seen, returned: &[ToolResultMessage], index: usize, hook: bool) {
    let id = format!("c{index}");
    let of = |entries: &[(String, Option<Value>)]| -> Vec<Option<Value>> {
        entries
            .iter()
            .filter(|(call, _)| *call == id)
            .map(|(_, details)| details.clone())
            .collect()
    };
    let expected = expected_state(index);
    assert_eq!(
        of(&seen.updates),
        (0..3).map(expected_state).collect::<Vec<_>>()
    );
    assert_eq!(of(&seen.ends), vec![expected.clone()]);
    assert_eq!(
        of(&seen.hook_inputs),
        if hook { vec![expected.clone()] } else { vec![] }
    );
    let messages: Vec<_> = seen
        .messages
        .iter()
        .filter(|(call, _)| *call == id)
        .collect();
    assert_eq!(messages.len(), 2);
    for (_, json) in messages {
        assert_eq!(json.get("details").cloned(), expected);
    }
    assert_eq!(seen.turn_results[index].details, expected);
    assert_eq!(returned[index].tool_call_id, id);
    let wire = serde_json::to_value(&returned[index]).unwrap();
    assert_eq!(wire.get("details").cloned(), expected);
    let decoded: ToolResultMessage = serde_json::from_value(wire).unwrap();
    assert_eq!(decoded.details, expected);
}
/// A hook that records its input and returns no override.
fn recording_hook(seen: &Arc<Mutex<Seen>>) -> AfterToolCall<maestro_agent::NoCustomMessages> {
    let seen = Arc::clone(seen);
    Arc::new(move |context, _| {
        seen.lock()
            .unwrap()
            .hook_inputs
            .push((context.tool_call.id.clone(), context.result.details.clone()));
        Box::pin(std::future::ready(Ok(None)))
    })
}
/// Run the three-call batch through both modes and entries, with and without a hook.
async fn three_state_matrix(execute: impl Fn() -> ExecuteTool) {
    let modes = [ToolExecutionMode::Sequential, ToolExecutionMode::Parallel];
    let entries = [Entry::Prompt, Entry::Continue];
    let combinations = modes.into_iter().flat_map(|mode| {
        entries
            .into_iter()
            .flat_map(move |entry| [false, true].map(|hook| (mode, entry, hook)))
    });
    for (mode, entry, hook) in combinations {
        let hook_seen = Arc::new(Mutex::new(Seen::default()));
        let after = hook.then(|| recording_hook(&hook_seen));
        let (seen, returned) =
            drive(calls(&["c0", "c1", "c2"]), execute(), after, mode, entry).await;
        let mut seen = Arc::try_unwrap(seen).ok().unwrap().into_inner().unwrap();
        seen.hook_inputs = std::mem::take(&mut hook_seen.lock().unwrap().hook_inputs);
        assert_eq!(returned.len(), 3);
        for index in 0..3 {
            assert_call_states(&seen, &returned, index, hook);
        }
    }
}

/// An ordinary erased tool keeps the three states through the raw loop.
#[test]
fn raw_tool_turn_preserves_missing_null_and_value_details() {
    run(three_state_matrix(erased_execute));
}

/// A typed tool keeps the three states through erasure and the raw loop.
#[test]
fn typed_tool_turn_preserves_missing_null_and_value_details() {
    run(three_state_matrix(|| {
        AgentTool::typed(definition(), "Echo".into(), typed_execute()).execute
    }));
}

/// One recorded original and override with the message it must produce.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The tool's result.
    input: Input,
    /// The after-hook's override.
    r#override: Override,
    /// The message without its timestamp.
    expected: Value,
}
/// Fields of the tool's result.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Result content.
    content: Vec<UserBlock>,
    /// Result details; absent when the member is missing.
    #[serde(default, deserialize_with = "present")]
    details: Option<Value>,
    /// Termination request.
    terminate: Option<bool>,
}
/// Fields of the hook's override.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Override {
    /// Replacement details; absent when the member is missing.
    #[serde(default, deserialize_with = "present")]
    details: Option<Value>,
}
/// Keep a member that is present, even when it is JSON null.
fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

/// An absent or null override keeps the original; every other value replaces it whole.
#[test]
fn after_tool_details_keep_nullish_inputs_and_replace_values() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/tool_details.json")).unwrap();
    assert_eq!(cases.len(), 27);
    run(async {
        for mode in [ToolExecutionMode::Sequential, ToolExecutionMode::Parallel] {
            for case in &cases {
                run_case(case, mode).await;
            }
        }
    });
}
/// Drive one fixture case and compare the hook input, returned message and end details with its expectation.
async fn run_case(case: &Case, mode: ToolExecutionMode) {
    let input = case.input.clone();
    let execute: ExecuteTool = Arc::new(move |_, _, _, _| {
        ready(Ok(AgentToolResult {
            content: input.content.clone(),
            details: input.details.clone(),
            terminate: input.terminate,
            usage: None,
        }))
    });
    let hook_seen = Arc::new(Mutex::new(Seen::default()));
    let observed = Arc::clone(&hook_seen);
    let replacement = case.r#override.details.clone();
    let after: AfterToolCall<maestro_agent::NoCustomMessages> = Arc::new(move |context, _| {
        observed
            .lock()
            .unwrap()
            .hook_inputs
            .push((context.tool_call.id.clone(), context.result.details.clone()));
        Box::pin(std::future::ready(Ok(Some(AfterToolCallResult {
            details: replacement.clone(),
            ..Default::default()
        }))))
    });
    let (seen, returned) = drive(
        calls(&["call-1"]),
        execute,
        Some(after),
        mode,
        Entry::Continue,
    )
    .await;
    assert_eq!(
        hook_seen.lock().unwrap().hook_inputs[0].1,
        case.input.details
    );
    let mut message = serde_json::to_value(&returned[0]).unwrap();
    assert!(message["timestamp"].as_f64().is_some_and(f64::is_finite));
    message.as_object_mut().unwrap().shift_remove("timestamp");
    assert_eq!(message, case.expected);
    let seen = seen.lock().unwrap();
    assert_eq!(seen.ends[0].1, case.expected.get("details").cloned());
}
/// Execution and after-hook failures become error results with present empty details.
#[test]
fn tool_failures_keep_present_empty_details() {
    run(async {
        let failing: ExecuteTool = Arc::new(|_, _, _, _| ready(Err(diagnostic("tool failed"))));
        let (_, returned) = drive(
            calls(&["c0"]),
            failing,
            None,
            ToolExecutionMode::Sequential,
            Entry::Prompt,
        )
        .await;
        assert_failure(&returned[0], "tool failed");
        let failing_hook: AfterToolCall<maestro_agent::NoCustomMessages> =
            Arc::new(|_, _| Box::pin(std::future::ready(Err(diagnostic("after failed")))));
        let (_, returned) = drive(
            calls(&["c0"]),
            Arc::new(|_, _, _, _| ready(Ok(result()))),
            Some(failing_hook),
            ToolExecutionMode::Sequential,
            Entry::Prompt,
        )
        .await;
        assert_failure(&returned[0], "after failed");
    });
}
/// A failed call carries the caller's text and present empty details.
fn assert_failure(message: &ToolResultMessage, text: &str) {
    assert!(message.is_error);
    assert_eq!(message.details, Some(json!({})));
    assert_eq!(
        serde_json::to_value(&message.content).unwrap(),
        json!([{"type":"text","text":text}])
    );
}
