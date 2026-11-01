//! Each recorded hook group realized by component handlers and dispatched by the native runner.
//!
//! A group returns its observed outcome and the recorded outcome projected onto the declared
//! types, in the recording's own shape.
use super::{Harness, Run, label};
use maestro_extensions::ExtensionRunner;
use maestro_extensions::{
    AfterProviderResponseEvent, AgentStartEvent, BashExecOptions, BashOperations, BashResult,
    BuildSystemPromptOptions, Cancellation, InputSource, Local, ModelSelectEvent,
    ModelSelectSource, Presence, ResourcesDiscoverReason, SessionStartEvent, SessionStartReason,
    ToolCallEvent, ToolResultEvent, TurnStartEvent, UserBashEvent, UserBashEventResult,
    runner::{RunnerEmitEvent, emit_session_shutdown_event},
};
use maestro_models::ImageContent;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::rc::Rc;

/// One step: a single member naming the action and holding its operand.
fn step(action: &str, operand: Value) -> Value {
    Value::Object([(action.to_owned(), operand)].into_iter().collect())
}

/// Reports `value` on the trace channel.
pub(super) fn trace(value: impl Into<Value>) -> Value {
    step("trace", value.into())
}

/// Fails with `message`.
pub(super) fn fail(message: &str) -> Value {
    json!({ "fail": message })
}

/// Returns `value` as the event's result.
pub(super) fn answer(value: Value) -> Value {
    step("return", value)
}

/// Reports what the handler reads.
pub(super) fn observe(what: &str) -> Value {
    json!({ "observe": what })
}

/// Replaces one event field.
pub(super) fn edit(field: &str, value: Value) -> Value {
    step("edit", step(field, value))
}

/// Decodes a native event or result from its JSON form.
pub(super) fn typed<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

/// Encodes a native value as JSON.
pub(super) fn encoded(value: &impl Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

/// A recorded or observed outcome with its diagnostics.
pub(super) fn outcome(output: Value, run: &Run) -> Value {
    let members = [
        ("output".to_owned(), output),
        ("errors".to_owned(), run.errors()),
    ];
    Value::Object(members.into_iter().collect())
}

/// A complete native user message with text content.
pub(super) fn user(text: &str) -> Value {
    json!({ "role": "user", "content": text, "timestamp": 1 })
}

/// A complete native assistant message with one text block.
pub(super) fn assistant(text: &str) -> Value {
    json!({
        "role": "assistant", "content": [{ "type": "text", "text": text }],
        "api": "fixture", "provider": "fixture", "model": "fixture",
        "usage": {
            "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
            "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0 },
        },
        "stopReason": "stop", "timestamp": 0,
    })
}

/// A message's role and text: declared content is text or text blocks, so a recorded string
/// where the declared type holds blocks compares by its text, and undeclared members drop.
fn text_view(message: &Value) -> Value {
    let text = match &message["content"] {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect(),
        _ => return message.clone(),
    };
    json!({ "role": message["role"], "text": text })
}

/// The values the components reported since the last read.
pub(super) fn labels(harness: &Harness) -> Vec<Value> {
    harness.bus.take()
}

/// Dispatches one recorded case.
pub(super) async fn replay(
    harness: &Harness,
    index: usize,
    name: &str,
    input: &Value,
    recorded: Value,
) -> (Value, Value) {
    match name {
        "notifications" => notifications(harness, input, recorded).await,
        "notification-live" => notification_live(harness, input, recorded).await,
        "session-veto" => super::sessions::veto(harness, input, recorded).await,
        "session-selected" => super::sessions::selected_result(harness, input, recorded).await,
        "abort-in-hook" => super::sessions::abort(harness, input, recorded).await,
        "shutdown-helper" => shutdown(harness, input, recorded).await,
        "live-registration" => live_registration(harness, recorded).await,
        _ => replay_tools(harness, index, name, input, recorded).await,
    }
}

/// Dispatches the tool, shell, resource and listener groups, then the value groups.
async fn replay_tools(
    harness: &Harness,
    index: usize,
    name: &str,
    input: &Value,
    recorded: Value,
) -> (Value, Value) {
    match name {
        "reporter-failure" => reporter_failure(harness, recorded).await,
        "work-count" => work_count(harness, input, recorded).await,
        "resources" | "resource-owner-path" => resources(harness, name, recorded).await,
        "user-bash" => user_bash(harness, input, recorded).await,
        "tool-call" => tool_call(harness, input, recorded).await,
        "tool-result" => tool_result(harness, input, recorded).await,
        _ => super::values::replay(harness, index, name, input, recorded).await,
    }
}

/// Emits a typed notification and reports whether a result came back.
async fn emit<V: RunnerEmitEvent<Result = ()>>(
    runner: &ExtensionRunner<Local>,
    mut event: V,
) -> bool {
    runner.emit(&mut event).await.unwrap().is_some()
}

/// Emits the named notification with a minimal typed event.
pub(super) async fn notify(runner: &ExtensionRunner<Local>, name: &str) -> bool {
    use maestro_extensions as domain;
    match name {
        "session_start" => emit(runner, SessionStartEvent { reason: SessionStartReason::Startup, previous_session_file: Presence::Missing }).await,
        "session_compact" => emit::<domain::SessionCompactEvent>(runner, typed(json!({"compactionEntry":{"type":"compaction","id":"compact","parentId":null,"timestamp":"2026-10-17T00:00:00Z","summary":"summary","firstKeptEntryId":"kept","tokensBefore":2.5},"fromExtension":false}))).await,
        "session_shutdown" => emit::<domain::SessionShutdownEvent>(runner, typed(json!({"reason":"quit"}))).await,
        "session_tree" => emit::<domain::SessionTreeEvent>(runner, typed(json!({"newLeafId":"new","oldLeafId":null}))).await,
        "after_provider_response" => emit(runner, AfterProviderResponseEvent { status: 200.0, headers: std::collections::BTreeMap::new() }).await,
        "agent_start" => emit(runner, AgentStartEvent {}).await,
        "agent_end" => emit::<domain::AgentEndEvent>(runner, typed(json!({ "messages": [user("done")] }))).await,
        "turn_start" => emit(runner, TurnStartEvent { turn_index: 2.0, timestamp: 3.0 }).await,
        "turn_end" => emit::<domain::TurnEndEvent>(runner, typed(json!({ "turnIndex": 2, "message": user("turn"), "toolResults": [] }))).await,
        _ => notify_messages(runner, name).await,
    }
}

/// Emits the named message, tool or selection notification with a minimal typed event.
async fn notify_messages(runner: &ExtensionRunner<Local>, name: &str) -> bool {
    use maestro_extensions as domain;
    let stream = json!({ "type": "start", "partial": assistant("partial") });
    match name {
        "message_start" => emit::<domain::MessageStartEvent>(runner, typed(json!({ "message": user("start") }))).await,
        "message_update" => emit::<domain::MessageUpdateEvent>(runner, typed(json!({ "message": assistant("partial"), "assistantMessageEvent": stream }))).await,
        "tool_execution_start" => emit::<domain::ToolExecutionStartEvent>(runner, typed(json!({ "toolCallId": "id", "toolName": "tool", "args": {} }))).await,
        "tool_execution_update" => emit::<domain::ToolExecutionUpdateEvent>(runner, typed(json!({ "toolCallId": "id", "toolName": "tool", "args": {}, "partialResult": false }))).await,
        "tool_execution_end" => emit::<domain::ToolExecutionEndEvent>(runner, typed(json!({ "toolCallId": "id", "toolName": "tool", "result": false, "isError": false }))).await,
        "model_select" => emit(runner, ModelSelectEvent { model: super::super::model_support::model("selected"), previous_model: Presence::Missing, source: ModelSelectSource::Set }).await,
        "thinking_level_select" => emit::<domain::ThinkingLevelSelectEvent>(runner, typed(json!({ "level": "high", "previousLevel": "off" }))).await,
        other => panic!("unrecorded notification {other}"),
    }
}

/// Handler order, ignored return values and isolated failures across two instances.
async fn notifications(harness: &Harness, input: &Value, recorded: Value) -> (Value, Value) {
    let event = input["event"].as_str().unwrap();
    let first = vec![
        json!([trace("a"), fail("first")]),
        json!([trace("b"), answer(json!({ "cancel": true }))]),
        json!([trace("c"), fail("middle")]),
    ];
    let second = vec![json!([trace("d"), fail("last")]), json!([trace("e")])];
    let run = harness.prepare(event, &[first, second]).await;
    let returned = if notify(&run.runner, event).await {
        "present"
    } else {
        "absent"
    };
    let observed = json!({ "trace": labels(harness), "returned": returned });
    (outcome(observed, &run), recorded)
}

/// Runs `dispatch` until a component waits, installs `added` meanwhile, then releases it.
pub(super) async fn while_waiting<F: Future + 'static>(
    harness: &Harness,
    run: &Run,
    event: &str,
    added: Value,
    dispatch: F,
) -> F::Output
where
    F::Output: 'static,
{
    let task = tokio::task::spawn_local(dispatch);
    harness.gate.entered.notified().await;
    harness.install(run, 0, event, &[added]).await;
    harness.gate.release.notify_one();
    task.await.unwrap()
}

/// A handler registered while another waits runs in the same dispatch.
async fn notification_live(harness: &Harness, input: &Value, recorded: Value) -> (Value, Value) {
    let event = input["event"].as_str().unwrap().to_owned();
    let waiting = json!([trace("start"), "wait", trace("resumed")]);
    let run = harness.prepare(&event, &[vec![waiting]]).await;
    let runner = run.runner.clone();
    let name = event.clone();
    while_waiting(harness, &run, &event, json!([trace("added")]), async move {
        notify(&runner, &name).await
    })
    .await;
    (json!({ "trace": labels(harness) }), recorded)
}

/// The same for input, whose added handler continues.
async fn live_registration(harness: &Harness, recorded: Value) -> (Value, Value) {
    let waiting = json!([trace("start"), "wait", trace("resumed")]);
    let run = harness.prepare("input", &[vec![waiting]]).await;
    let runner = run.runner.clone();
    let added = json!([trace("added"), answer(json!({ "action": "continue" }))]);
    while_waiting(harness, &run, "input", added, async move {
        let mut images = Presence::Missing;
        runner
            .emit_input("text", &mut images, InputSource::Rpc, None)
            .await
            .unwrap()
    })
    .await;
    (json!({ "trace": labels(harness) }), recorded)
}

/// Shutdown is emitted only when an instance handles it, with failures isolated.
async fn shutdown(harness: &Harness, input: &Value, recorded: Value) -> (Value, Value) {
    let handlers = if input["present"] == true {
        vec![vec![
            json!([trace("failed"), fail("shutdown fault")]),
            json!([trace("healthy")]),
        ]]
    } else {
        vec![vec![]]
    };
    let run = harness.prepare("session_shutdown", &handlers).await;
    let mut event = typed(json!({ "reason": "quit" }));
    let emitted = emit_session_shutdown_event(&run.runner, &mut event)
        .await
        .unwrap();
    let observed = json!({ "emitted": emitted, "trace": labels(harness) });
    (outcome(observed, &run), recorded)
}

/// A failing error listener stops the dispatch and later handlers.
async fn reporter_failure(harness: &Harness, recorded: Value) -> (Value, Value) {
    let handlers = vec![
        json!([fail("original handler failure")]),
        json!([trace("late handler")]),
    ];
    let run = harness.prepare("agent_start", &[handlers]).await;
    let _failing = run
        .runner
        .on_error(Rc::new(|_| Err("listener failure".into())));
    let error = run
        .runner
        .emit(&mut AgentStartEvent {})
        .await
        .unwrap_err()
        .to_string();
    (json!({ "error": error, "seen": labels(harness) }), recorded)
}

/// Every instance's handler runs once per dispatch.
async fn work_count(harness: &Harness, input: &Value, recorded: Value) -> (Value, Value) {
    let count = usize::try_from(input["n"].as_u64().unwrap()).unwrap();
    let handlers = vec![vec![json!([trace("call")])]; count];
    let run = harness.prepare("input", &handlers).await;
    let mut images = Presence::Missing;
    run.runner
        .emit_input("x", &mut images, InputSource::Rpc, None)
        .await
        .unwrap();
    (json!({ "calls": labels(harness).len() }), recorded)
}

/// Ordered contributions with duplicates, attributed to their owner's current path.
async fn resources(harness: &Harness, name: &str, recorded: Value) -> (Value, Value) {
    let renamed = name == "resource-owner-path";
    let handlers = if renamed {
        vec![vec![json!([
            trace("rename"),
            answer(json!({ "skillPaths": ["exact"] }))
        ])]]
    } else {
        vec![
            vec![json!([answer(
                json!({ "skillPaths": ["z", "a", "z"], "promptPaths": [], "themePaths": ["theme"] })
            )])],
            vec![
                json!([fail("resource fault")]),
                json!([answer(json!({ "skillPaths": ["a"], "promptPaths": ["p"] }))]),
            ],
        ]
    };
    let run = harness.prepare("resources_discover", &handlers).await;
    let owner = harness.owner(0).clone();
    let _rename = harness.bus.controller.on(
        "trace",
        Rc::new(move |data, _| {
            if *data == json!("rename") {
                owner.write().unwrap().path = "<renamed>".into();
            }
            Ok(None)
        }),
    );
    let reason = if renamed {
        ResourcesDiscoverReason::Startup
    } else {
        ResourcesDiscoverReason::Reload
    };
    let result = run
        .runner
        .emit_resources_discover("/work".into(), reason)
        .await
        .unwrap();
    let paths = |paths: Vec<maestro_extensions::runner::ResourcePath>| -> Value {
        paths
            .into_iter()
            .map(|path| json!({ "path": path.path, "extensionPath": path.extension_path }))
            .collect()
    };
    let observed = json!({
        "skillPaths": paths(result.skill_paths),
        "promptPaths": paths(result.prompt_paths),
        "themePaths": paths(result.theme_paths),
    });
    harness.bus.take();
    harness.owner(0).write().unwrap().path = label(0);
    if renamed {
        return (observed, recorded);
    }
    (outcome(observed, &run), recorded)
}

/// The first shell verdict wins; returned operations stay callable.
async fn user_bash(harness: &Harness, input: &Value, recorded: Value) -> (Value, Value) {
    let mode = input["mode"].as_str().unwrap();
    let winner = match mode {
        "none" => json!([trace("winner")]),
        "empty" => json!([trace("winner"), answer(json!({}))]),
        "result" => json!([
            trace("winner"),
            answer(
                json!({ "result": { "output": "replacement", "exitCode": 0, "cancelled": false, "truncated": false } })
            )
        ]),
        _ => json!([trace("winner"), "operations"]),
    };
    let handlers = vec![
        json!([trace("failed"), fail("shell fault")]),
        winner,
        json!([trace("later")]),
    ];
    let run = harness.prepare("user_bash", &[handlers]).await;
    let mut event = UserBashEvent {
        command: "controlled".into(),
        cwd: "/work".into(),
        exclude_from_context: false,
    };
    let result: Option<UserBashEventResult<Rc<dyn BashOperations<Local>>, BashResult>> =
        run.runner.emit_user_bash(&mut event).await.unwrap();
    let result = match result {
        Some(UserBashEventResult {
            operations: Some(operations),
            ..
        }) => invoke(operations).await,
        other => other.map_or(Value::Null, |result| encoded(&result)),
    };
    let observed = json!({ "result": result, "trace": labels(harness) });
    let mut recorded = recorded;
    if let Some(shell) = recorded["output"]["result"].get_mut("result") {
        // The declared exit code is a number; compare it in the native number form.
        *shell = encoded(&typed::<BashResult>(shell.clone()));
    }
    (outcome(observed, &run), recorded)
}

/// Calls returned operations with the recorded operands and checks their streamed output.
async fn invoke(operations: Rc<dyn BashOperations<Local>>) -> Value {
    let mut chunks = Vec::new();
    let mut sink = |bytes: &[u8]| chunks.push(bytes.to_vec());
    let sink: &mut dyn FnMut(&[u8]) = &mut sink;
    let options = BashExecOptions {
        on_data: sink,
        signal: None,
        timeout: None,
        env: None,
    };
    let result = operations
        .exec("controlled", "/work", options)
        .await
        .unwrap();
    assert_eq!(
        (result.exit_code, chunks),
        (Some(0.0), vec![b"bytes".to_vec()])
    );
    json!({ "operations": "retained invoked handle" })
}

/// Arguments evolve across handlers; the first block stops and a failure propagates.
async fn tool_call(harness: &Harness, input: &Value, recorded: Value) -> (Value, Value) {
    let ending = match input["outcome"].as_str().unwrap() {
        "throw" => fail("tool fault"),
        "block" => answer(json!({ "block": true, "reason": "stop" })),
        _ => answer(json!({})),
    };
    let handlers = vec![
        json!([
            trace("first"),
            edit("toolInput", json!({ "path": "after" })),
            answer(json!({ "block": false, "reason": "first" }))
        ]),
        json!([observe("toolPath"), ending]),
        json!([trace("later")]),
    ];
    let run = harness.prepare("tool_call", &[handlers]).await;
    let mut event: ToolCallEvent =
        typed(json!({ "toolCallId": "id", "toolName": "read", "input": { "path": "before" } }));
    let (result, thrown) = match run.runner.emit_tool_call(&mut event).await {
        Ok(result) => (
            result.map_or(Value::Null, |result| encoded(&result)),
            Value::Null,
        ),
        Err(error) => (Value::Null, json!(error.to_string())),
    };
    let observed = json!({ "result": result, "thrown": thrown, "trace": labels(harness), "input": event.input });
    (outcome(observed, &run), recorded)
}

/// Present patch fields apply; the recorded original is passed by shared reference.
async fn tool_result(harness: &Harness, input: &Value, mut recorded: Value) -> (Value, Value) {
    let mode = input["mode"].as_str().unwrap();
    let first = match mode {
        "none" => json!([]),
        "empty" => json!([answer(json!({}))]),
        "content" => json!([answer(json!({ "content": [] }))]),
        "all-fields" => json!([answer(
            json!({ "content": [], "details": null, "isError": false })
        )]),
        _ => json!([answer(
            json!({ "content": [{ "type": "text", "text": "after" }] })
        )]),
    };
    let partial = mode == "partial-after-error";
    let second = if partial {
        json!([fail("patch fault")])
    } else {
        json!([])
    };
    let third = if partial {
        json!([answer(json!({ "details": null }))])
    } else {
        json!([])
    };
    let run = harness
        .prepare("tool_result", &[vec![first, second, third]])
        .await;
    let event: ToolResultEvent = typed(json!({
        "toolName": "read", "toolCallId": "id", "input": { "path": "original" },
        "content": [{ "type": "text", "text": "before" }], "details": { "old": true }, "isError": true,
    }));
    let result = run.runner.emit_tool_result(&event).await.unwrap();
    let observed = json!({
        "result": result.as_ref().map_or(Value::Null, encoded),
        "modified": result.is_some(),
    });
    // The caller's event is borrowed immutably, so its unchanged original is ownership-enforced.
    recorded["output"]
        .as_object_mut()
        .unwrap()
        .shift_remove("original");
    (outcome(observed, &run), recorded)
}

/// Images supplied to an agent start, as a caller passes them.
pub(super) fn images() -> Presence<Vec<ImageContent>> {
    Presence::Present(vec![typed(
        json!({ "type": "image", "data": "old", "mimeType": "image/png" }),
    )])
}

/// Prompt options with the recorded values.
pub(super) fn options() -> BuildSystemPromptOptions {
    typed(
        json!({ "cwd": "/work", "selectedTools": ["z", "a", "z"], "customPrompt": "", "contextFiles": [], "skills": [] }),
    )
}

/// A cancellation the case can abort through a context.
pub(super) fn cancellation(harness: &Harness) -> Cancellation {
    let cancellation = Cancellation::new();
    harness.cancellation.replace(Some(cancellation.clone()));
    cancellation
}

/// The text view of every message in a list.
pub(super) fn text_views(messages: &Value) -> Value {
    messages.as_array().unwrap().iter().map(text_view).collect()
}

/// The text view of a message, or null.
pub(super) fn message_view(message: &Value) -> Value {
    if message.is_null() {
        Value::Null
    } else {
        text_view(message)
    }
}
