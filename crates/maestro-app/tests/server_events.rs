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
                    usage: None,
                });
            }
            Box::pin(std::future::ready(Ok(AgentToolResult {
                content: text("finished"),
                details: Some(json!({"phase": "done"})),
                terminate: None,
                usage: None,
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

/// Every session event reaches the client once, in emitted order, with the payload it had
/// when it was emitted: turns, streamed blocks, tool runs, retries, queues, compaction, the
/// session name and the thinking level.
#[test]
fn session_events_keep_every_payload() {
    on_paused_task_set(|_| async {
        let (h, runtime, _) = runtime_for("events", vec![(None, progress_tool())]).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        client.ask(r#"{"type":"get_state"}"#).await;
        let emitted = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&emitted);
        let _unsubscribe = runtime.session().subscribe(Rc::new(move |event| {
            let value = serde_json::to_value(event).unwrap();
            let line = maestro_app::compact_json(&value).unwrap();
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
        assert_eq!(streamed, *emitted.borrow());
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
        let update = streamed
            .iter()
            .find(|record| record["type"] == "tool_execution_update")
            .unwrap();
        assert_eq!(
            update["partialResult"]["details"],
            json!({"phase": "working"})
        );
        assert!(streamed.iter().all(|record| record.get("id").is_none()));
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
