//! Session events and extension failures as the protocol streams them.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::rpc_connection::{Client, connect, response_to};
use crate::support::runtime_fixtures::runtime_for;
use crate::support::session_harness::{
    answer, on_paused_task_set, on_task_set, script_responses, tool_definition,
};
use maestro_agent::AgentToolResult;
use maestro_extensions::{ExtensionFactory, SessionStartEvent};
use maestro_models::{AssistantMessage, Local, StopReason, TextContent, UserBlock};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

/// An extension registering `progress`, a tool that reports one update before its result.
fn progress_tool() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        let mut tool = tool_definition("progress", None, None);
        tool.execute = Rc::new(|_, _, _, on_update, _| {
            let text = |text: &str| vec![UserBlock::Text(TextContent::new(text.into()))];
            if let Some(update) = on_update {
                update(AgentToolResult {
                    content: text("halfway"),
                    details: Some(json!({"phase": "working"})),
                    terminate: None,
                });
            }
            Box::pin(std::future::ready(Ok(AgentToolResult {
                content: text("finished"),
                details: Some(json!({"phase": "done"})),
                terminate: None,
            })))
        });
        Box::pin(std::future::ready(api.register_tool(tool)))
    })
}

/// An answer that thinks, writes and calls `progress`.
fn tool_calling_answer() -> AssistantMessage {
    let mut message = answer("calling", None, None);
    let mut value = serde_json::to_value(&message).unwrap();
    value["content"] = json!([
        {"type": "thinking", "thinking": "considering"},
        {"type": "text", "text": "calling"},
        {"type": "toolCall", "id": "call-1", "name": "progress", "arguments": {"step": 1}},
    ]);
    value["stopReason"] = json!("toolUse");
    message = serde_json::from_value(value).unwrap();
    message
}

/// The 17 session event types.
const SESSION_EVENT_TYPES: [&str; 17] = [
    "agent_end",
    "agent_start",
    "auto_retry_end",
    "auto_retry_start",
    "compaction_end",
    "compaction_start",
    "message_end",
    "message_start",
    "message_update",
    "queue_update",
    "session_info_changed",
    "thinking_level_changed",
    "tool_execution_end",
    "tool_execution_start",
    "tool_execution_update",
    "turn_end",
    "turn_start",
];

/// Each streamed event type's members in the order its producer writes them, for the
/// optional members this exchange produces.
const MEMBER_ORDER: [(&str, &[&str]); 17] = [
    ("agent_end", &["type", "messages"]),
    ("agent_start", &["type"]),
    ("auto_retry_end", &["type", "success", "attempt"]),
    (
        "auto_retry_start",
        &["type", "attempt", "maxAttempts", "delayMs", "errorMessage"],
    ),
    (
        "compaction_end",
        &["type", "reason", "result", "aborted", "willRetry"],
    ),
    ("compaction_start", &["type", "reason"]),
    ("message_end", &["type", "message"]),
    ("message_start", &["type", "message"]),
    (
        "message_update",
        &["type", "assistantMessageEvent", "message"],
    ),
    ("queue_update", &["type", "steering", "followUp"]),
    ("session_info_changed", &["type", "name"]),
    ("thinking_level_changed", &["type", "level"]),
    (
        "tool_execution_end",
        &["type", "toolCallId", "toolName", "result", "isError"],
    ),
    (
        "tool_execution_start",
        &["type", "toolCallId", "toolName", "args"],
    ),
    (
        "tool_execution_update",
        &["type", "toolCallId", "toolName", "args", "partialResult"],
    ),
    ("turn_end", &["type", "message", "toolResults"]),
    ("turn_start", &["type"]),
];

/// Drives the commands that emit each session event type and returns every record up to
/// the last one's response.
async fn stream_every_kind(client: &mut Client) -> Vec<Value> {
    let mut streamed = Vec::new();
    for (line, last) in [
        (r#"{"type":"prompt","message":"use the tool"}"#, "agent_end"),
        (r#"{"type":"prompt","message":"again"}"#, "auto_retry_end"),
        (r#"{"type":"compact"}"#, "compaction_end"),
        (
            r#"{"type":"set_session_name","name":"named"}"#,
            "session_info_changed",
        ),
        (
            r#"{"type":"set_thinking_level","level":"high"}"#,
            "thinking_level_changed",
        ),
        (r#"{"type":"follow_up","message":"later"}"#, "queue_update"),
    ] {
        client.send(line);
        streamed.extend(client.until(|record| record["type"] == last).await);
    }
    streamed.extend(client.until(response_to("follow_up")).await);
    streamed
}

/// Runs every exchange of `stream_every_kind` against one session and returns the records the
/// client received, without responses, and those a subscription on the session saw.
async fn exchange() -> (Vec<Value>, Vec<Value>) {
    let (h, runtime, _) = runtime_for("events", vec![(None, progress_tool())]).await;
    let runtime = Rc::new(runtime);
    let mut client = connect(Rc::clone(&runtime));
    client.ask(r#"{"type":"get_state"}"#).await;
    let emitted = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&emitted);
    let _unsubscribe = runtime.session().subscribe(Rc::new(move |event| {
        let value = serde_json::to_value(event).unwrap();
        let line = maestro_json::compact_json(&value).unwrap();
        recorded
            .borrow_mut()
            .push(serde_json::from_str::<Value>(&line).unwrap());
    }));
    script_responses(
        &h,
        vec![
            tool_calling_answer(),
            answer("done", None, None),
            answer("", Some(StopReason::Error), Some("overloaded_error")),
            answer("recovered", None, None),
            answer("summary", None, None),
        ],
    );
    let mut streamed = stream_every_kind(&mut client).await;
    streamed.retain(|record| record["type"] != "response");
    let emitted = emitted.borrow().clone();
    (streamed, emitted)
}

/// Every session event reaches the client once, in emitted order, with the payload it had
/// when it was emitted: turns, streamed blocks, tool runs, retries, queues, compaction, the
/// session name and the thinking level.
#[test]
fn session_events_keep_every_payload() {
    on_paused_task_set(|_| async {
        let (streamed, emitted) = exchange().await;
        assert_eq!(streamed, emitted);
        let kinds: BTreeSet<&str> = streamed
            .iter()
            .map(|record| record["type"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, BTreeSet::from(SESSION_EVENT_TYPES));
        // Records keep the written member order, which record equality ignores.
        for record in &streamed {
            let members: Vec<&str> = record
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            let expected = MEMBER_ORDER
                .iter()
                .find(|(kind, _)| record["type"] == *kind);
            assert_eq!(
                Some(members.as_slice()),
                expected.map(|(_, order)| *order),
                "{record}"
            );
        }
        assert!(streamed.iter().all(|record| record.get("id").is_none()));
    });
}

/// Types of the shared payload table: the event type, a JSON pointer into its first record
/// and the value there. The empty pointer names the whole record.
type Payload = (&'static str, &'static str, Value);

/// The one user prompt every exchange starts with.
fn prompt() -> Value {
    json!([{"type": "text", "text": "use the tool"}])
}

/// The result of the finished `progress` call.
fn finished() -> Value {
    json!({"content": [{"type": "text", "text": "finished"}], "details": {"phase": "done"}})
}

/// The payloads of the turn and message events. Timestamps and model names vary between runs,
/// so records that hold them are read member by member.
fn message_payloads() -> Vec<Payload> {
    vec![
        ("agent_start", "", json!({"type": "agent_start"})),
        ("turn_start", "", json!({"type": "turn_start"})),
        ("message_start", "/message/role", json!("user")),
        ("message_start", "/message/content", prompt()),
        ("message_end", "/message/role", json!("user")),
        ("message_end", "/message/content", prompt()),
        (
            "message_update",
            "/assistantMessageEvent/type",
            json!("thinking_start"),
        ),
        (
            "message_update",
            "/assistantMessageEvent/contentIndex",
            json!(0),
        ),
        (
            "message_update",
            "/message/content",
            json!([{"type": "thinking", "thinking": ""}]),
        ),
    ]
}

/// The payloads of the tool execution events.
fn tool_payloads() -> Vec<Payload> {
    let result = finished();
    vec![
        (
            "tool_execution_start",
            "",
            json!({"type": "tool_execution_start", "toolCallId": "call-1", "toolName": "progress", "args": {"step": 1}}),
        ),
        (
            "tool_execution_update",
            "",
            json!({"type": "tool_execution_update", "toolCallId": "call-1", "toolName": "progress",
                "args": {"step": 1},
                "partialResult": {"content": [{"type": "text", "text": "halfway"}], "details": {"phase": "working"}}}),
        ),
        (
            "tool_execution_end",
            "",
            json!({"type": "tool_execution_end", "toolCallId": "call-1", "toolName": "progress",
                "result": result, "isError": false}),
        ),
    ]
}

/// The payloads that close a turn and the agent run.
fn run_payloads() -> Vec<Payload> {
    let result = finished();
    vec![
        (
            "turn_end",
            "/message/content",
            json!([
                {"type": "thinking", "thinking": "considering"},
                {"type": "text", "text": "calling"},
                {"type": "toolCall", "id": "call-1", "name": "progress", "arguments": {"step": 1}},
            ]),
        ),
        ("turn_end", "/message/stopReason", json!("toolUse")),
        ("turn_end", "/toolResults/0/toolCallId", json!("call-1")),
        (
            "turn_end",
            "/toolResults/0/content",
            result["content"].clone(),
        ),
        (
            "turn_end",
            "/toolResults/0/details",
            result["details"].clone(),
        ),
        ("turn_end", "/toolResults/0/isError", json!(false)),
        ("agent_end", "/messages/0/content", prompt()),
        ("agent_end", "/messages/2/toolName", json!("progress")),
        (
            "agent_end",
            "/messages/3/content",
            json!([{"type": "text", "text": "done"}]),
        ),
    ]
}

/// The payloads of the retry, compaction, name, thinking level and queue events.
fn session_payloads() -> Vec<Payload> {
    vec![
        (
            "auto_retry_start",
            "",
            json!({"type": "auto_retry_start", "attempt": 1, "maxAttempts": 3, "delayMs": 2000,
                "errorMessage": "overloaded_error"}),
        ),
        (
            "auto_retry_end",
            "",
            json!({"type": "auto_retry_end", "success": true, "attempt": 1}),
        ),
        (
            "compaction_start",
            "",
            json!({"type": "compaction_start", "reason": "manual"}),
        ),
        ("compaction_end", "/reason", json!("manual")),
        ("compaction_end", "/result/summary", json!("summary")),
        (
            "compaction_end",
            "/result/details",
            json!({"readFiles": [], "modifiedFiles": []}),
        ),
        ("compaction_end", "/aborted", json!(false)),
        ("compaction_end", "/willRetry", json!(false)),
        (
            "session_info_changed",
            "",
            json!({"type": "session_info_changed", "name": "named"}),
        ),
        (
            "thinking_level_changed",
            "",
            json!({"type": "thinking_level_changed", "level": "high"}),
        ),
        (
            "queue_update",
            "",
            json!({"type": "queue_update", "steering": [], "followUp": ["later"]}),
        ),
    ]
}

/// Each streamed event carries the payload its producer wrote, read from the client's records
/// alone: a changed member fails here even when the session emits the same changed record.
#[test]
fn streamed_events_carry_their_payload() {
    on_paused_task_set(|_| async {
        let (streamed, _) = exchange().await;
        for (kind, pointer, expected) in [
            message_payloads(),
            tool_payloads(),
            run_payloads(),
            session_payloads(),
        ]
        .concat()
        {
            let record = streamed
                .iter()
                .find(|record| record["type"] == kind)
                .unwrap_or_else(|| panic!("no {kind} record"));
            assert_eq!(record.pointer(pointer), Some(&expected), "{kind} {pointer}");
        }
    });
}

/// An extension's failure is an uncorrelated record with its path, event and message, and
/// the connection keeps serving.
#[test]
fn extension_errors_stay_uncorrelated() {
    on_task_set(|_| async {
        let failing: ExtensionFactory<Local> = Rc::new(|api| {
            let started = api.on::<SessionStartEvent, ()>(
                "session_start".into(),
                Rc::new(|_, _| Box::pin(std::future::ready(Err("start failed".into())))),
            );
            Box::pin(std::future::ready(started))
        });
        let (_h, runtime, _) =
            runtime_for("failing", vec![(Some("/extensions/failing.ts"), failing)]).await;
        let mut client = connect(Rc::new(runtime));
        assert_eq!(
            client.next().await,
            json!({"type": "extension_error", "extensionPath": "/extensions/failing.ts",
                "event": "session_start", "error": "start failed"})
        );
        let state = client.ask(r#"{"id":"after","type":"get_state"}"#).await;
        assert_eq!(
            (&state["id"], &state["success"]),
            (&json!("after"), &json!(true))
        );
    });
}
