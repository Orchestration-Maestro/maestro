//! Retry controls, tool recovery and the delivered event order of a complete session:
//! attempt limits, disabled and non-retryable failures, retry cancellation, extension order,
//! streamed content deltas and the terminal ends of failed and aborted runs.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, Scripted, agent_ended, agent_event_type, answer, corpus, delivered_events,
    get_assistant_texts, get_message_text, harness, on_paused_task_set, on_task_set, retry_order,
    script_responses, sdk_definition, session_run_ended, start_prompt, tool_definition,
};
use maestro_agent::{AgentEvent, AgentMessage, CustomAgentMessages};
use maestro_app::agent_session::ToolDefinitionHandle;
use maestro_app::{AgentSession, AgentSessionEvent};
use maestro_extensions::{
    ExtensionFactory, MessageEndEvent, MessageEndEventResult, MessageStartEvent, NativeAgentMessage,
};
use maestro_models::{AssistantMessageEvent, Local};
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::Notify;

/// The role a message reports, as the event labels spell it.
fn message_role(message: &NativeAgentMessage) -> String {
    match message {
        AgentMessage::User(_) => "user".into(),
        AgentMessage::Assistant(_) => "assistant".into(),
        AgentMessage::ToolResult(_) => "toolResult".into(),
        AgentMessage::Custom(custom) => custom.read().unwrap().role().to_owned(),
    }
}

/// Every session event `session` delivers from now on: an agent event's type, a message
/// qualified by its role and a tool call by its tool name, with adjacent stream updates
/// coalesced into one.
fn event_order(session: &AgentSession<Local>) -> Rc<RefCell<Vec<String>>> {
    let order = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&order);
    drop(session.subscribe(Rc::new(move |event| {
        let AgentSessionEvent::Agent(event) = event else {
            return;
        };
        let event = event.read().unwrap();
        let label = match &*event {
            AgentEvent::MessageStart { message } | AgentEvent::MessageEnd { message } => {
                format!("{}:{}", agent_event_type(&event), message_role(message))
            }
            AgentEvent::ToolExecutionStart { tool_call }
            | AgentEvent::ToolExecutionEnd { tool_call, .. } => {
                format!("{}:{}", agent_event_type(&event), tool_call.name)
            }
            other => agent_event_type(other).to_owned(),
        };
        let mut order = recorded.borrow_mut();
        if label == "message_update" && order.last().is_some_and(|last| *last == "message_update") {
            return;
        }
        order.push(label);
    })));
    order
}

/// Records each public message start and end as `<event>:<role>`.
fn record_public_messages(session: &AgentSession<Local>, order: Rc<RefCell<Vec<String>>>) {
    drop(session.subscribe(Rc::new(move |event| {
        let AgentSessionEvent::Agent(event) = event else {
            return;
        };
        let event = event.read().unwrap();
        let label = match &*event {
            AgentEvent::MessageStart { message } => {
                format!("public:message_start:{}", message_role(message))
            }
            AgentEvent::MessageEnd { message } => {
                format!("public:message_end:{}", message_role(message))
            }
            _ => return,
        };
        order.borrow_mut().push(label);
    })));
}

/// The extension that records each message start and end it hears as
/// `extension:<event>:<role>`.
fn recording_messages(order: Rc<RefCell<Vec<String>>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (started, ended) = (Rc::clone(&order), Rc::clone(&order));
        let start = api.on::<MessageStartEvent, ()>(
            "message_start".into(),
            Rc::new(move |event, _| {
                started.borrow_mut().push(format!(
                    "extension:message_start:{}",
                    message_role(&event.message)
                ));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let end = api.on::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
            "message_end".into(),
            Rc::new(move |event, _| {
                ended.borrow_mut().push(format!(
                    "extension:message_end:{}",
                    message_role(&event.message)
                ));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        Box::pin(std::future::ready(start.and(end)))
    })
}

/// The extension whose assistant `message_end` handler reports entering and then waits for a
/// release notification; other finished messages pass at once.
fn slow_assistant_end(entered: Rc<Notify>, release: Rc<Notify>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (entered, release) = (Rc::clone(&entered), Rc::clone(&release));
        let registered = api.on::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
            "message_end".into(),
            Rc::new(move |event, _| {
                let gate = matches!(event.message, AgentMessage::Assistant(_))
                    .then(|| (Rc::clone(&entered), Rc::clone(&release)));
                Box::pin(waits_for_release(gate))
            }),
        );
        Box::pin(std::future::ready(registered))
    })
}

/// Reports entering once `gate` holds, then answers after the release.
async fn waits_for_release(
    gate: Option<(Rc<Notify>, Rc<Notify>)>,
) -> Result<Option<MessageEndEventResult<NativeAgentMessage>>, maestro_extensions::EventError> {
    if let Some((entered, release)) = gate {
        entered.notify_one();
        release.notified().await;
    }
    Ok(None)
}

/// The echo tool that records each `text` argument and answers `echo:<text>`.
fn recorded_echo_tool(runs: Rc<RefCell<Vec<String>>>) -> ToolDefinitionHandle {
    let mut definition = tool_definition("echo", None, None);
    definition.definition.parameters = json!({
        "type": "object",
        "properties": {"text": {"type": "string"}},
        "required": ["text"],
    })
    .into();
    definition.execute = Rc::new(move |_, arguments: Value, _, _, _| {
        let text = arguments["text"].as_str().unwrap_or_default().to_owned();
        let runs = Rc::clone(&runs);
        Box::pin(async move {
            runs.borrow_mut().push(text.clone());
            Ok(maestro_agent::AgentToolResult {
                content: vec![maestro_models::UserBlock::Text(
                    maestro_models::TextContent::new(format!("echo:{text}")),
                )],
                details: Some(json!({"text": text})),
                terminate: None,
                usage: None,
            })
        })
    });
    sdk_definition(definition)
}

/// Harness options over the scripted stream answering `responses`.
fn scripted(responses: Vec<Scripted>) -> HarnessOptions {
    HarnessOptions {
        responses: Some(responses),
        ..HarnessOptions::default()
    }
}

/// The model stream updates of the session's first assistant response, in order.
fn first_response_updates(
    session: &AgentSession<Local>,
) -> Rc<RefCell<Vec<AssistantMessageEvent>>> {
    let updates = Rc::new(RefCell::new(Vec::new()));
    let recording = Rc::clone(&updates);
    let open = Rc::new(Cell::new(true));
    let listening = Rc::clone(&open);
    drop(session.subscribe(Rc::new(move |event| {
        let AgentSessionEvent::Agent(event) = event else {
            return;
        };
        let event = event.read().unwrap();
        match &*event {
            AgentEvent::MessageUpdate {
                assistant_message_event,
                ..
            } if listening.get() => recording.borrow_mut().push(assistant_message_event.clone()),
            AgentEvent::MessageEnd { .. } if !recording.borrow().is_empty() => listening.set(false),
            _ => {}
        }
    })));
    updates
}

#[test]
fn multiple_failures_recover_on_last_response() {
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 1}}),
            ..scripted(vec![
                Scripted::failure("overloaded_error"),
                Scripted::failure("overloaded_error"),
                Scripted::text("success"),
            ])
        })
        .await;
        let events = delivered_events(&h.session);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        let events = events.borrow();
        assert_eq!(retry_order(&events), ["start:1", "start:2", "end:true"]);
        let starts: Vec<&Value> = events
            .iter()
            .filter(|event| event["type"] == "auto_retry_start")
            .collect();
        let ends: Vec<&Value> = events
            .iter()
            .filter(|event| event["type"] == "auto_retry_end")
            .collect();
        assert_eq!(h.requests.0.borrow().len(), 3);
        assert_eq!(starts[0]["delayMs"], json!(1));
        assert_eq!(starts[1]["delayMs"], json!(2));
        assert_eq!(ends[0]["attempt"], json!(2));
        assert_eq!(get_assistant_texts(&h), ["success"]);
        assert!(!h.session.is_retrying());
    });
}

#[test]
fn registered_slow_end_keeps_prompt_waiting() {
    on_paused_task_set(|_| async {
        let (entered, release) = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let h = harness(HarnessOptions {
            factories: vec![slow_assistant_end(Rc::clone(&entered), Rc::clone(&release))],
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 1}}),
            ..scripted(vec![
                Scripted::failure("overloaded_error"),
                Scripted::text("recovered"),
            ])
        })
        .await;
        let ended = agent_ended(&h);
        let running = start_prompt(&h, "hi");
        entered.notified().await;
        ended.notified().await;
        assert_eq!(h.requests.0.borrow().len(), 1);
        assert!(h.session.is_streaming(), "the run is still active");
        assert!(!running.is_finished(), "the prompt still waits");
        release.notify_one();
        entered.notified().await;
        release.notify_one();
        running.await.unwrap().unwrap();
        assert_eq!(h.requests.0.borrow().len(), 2);
        assert_eq!(get_assistant_texts(&h), ["recovered"]);
        assert!(!h.session.is_retrying());
    });
}

#[test]
fn disabled_retry_makes_one_attempt() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": false}}),
            ..scripted(vec![Scripted::failure("overloaded_error")])
        })
        .await;
        let events = delivered_events(&h.session);
        let ended = session_run_ended(&h);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        ended.notified().await;
        assert_eq!(h.requests.0.borrow().len(), 1);
        assert!(retry_order(&events.borrow()).is_empty());
    });
}

#[test]
fn ordinary_provider_error_does_not_retry() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 1}}),
            ..scripted(vec![Scripted::failure("invalid_api_key")])
        })
        .await;
        let events = delivered_events(&h.session);
        let ended = session_run_ended(&h);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        ended.notified().await;
        assert_eq!(h.requests.0.borrow().len(), 1);
        assert!(retry_order(&events.borrow()).is_empty());
    });
}

#[test]
fn retry_sleep_cancellation_settles_prompt() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 100}}),
            ..scripted(vec![Scripted::failure("overloaded_error")])
        })
        .await;
        let events = delivered_events(&h.session);
        let (started, finished) = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let (start, end) = (Rc::clone(&started), Rc::clone(&finished));
        drop(h.session.subscribe(Rc::new(move |event| match event {
            AgentSessionEvent::AutoRetryStart { .. } => start.notify_one(),
            AgentSessionEvent::AutoRetryEnd { .. } => end.notify_one(),
            _ => {}
        })));
        let running = start_prompt(&h, "hi");
        started.notified().await;
        h.session.abort_retry();
        running.await.unwrap().unwrap();
        finished.notified().await;
        let events = events.borrow();
        let ends: Vec<&Value> = events
            .iter()
            .filter(|event| event["type"] == "auto_retry_end")
            .collect();
        assert_eq!(h.requests.0.borrow().len(), 1);
        assert!(!h.session.is_retrying());
        assert_eq!(ends.len(), 1);
        assert_eq!(ends[0]["attempt"], json!(1));
        assert_eq!(ends[0]["success"], json!(false));
        assert_eq!(ends[0]["finalError"], json!("Retry cancelled"));
    });
}

#[test]
fn recovered_tool_only_turn_settles_before_follow_up() {
    on_task_set(|_| async {
        let runs = Rc::new(RefCell::new(Vec::new()));
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 1}}),
            custom_tools: vec![recorded_echo_tool(Rc::clone(&runs))],
            ..scripted(vec![
                Scripted::failure("overloaded_error"),
                Scripted::echo_call("hello"),
                Scripted::text("final answer"),
                Scripted::text("follow-up answer"),
            ])
        })
        .await;
        start_prompt(&h, "hi").await.unwrap().unwrap();
        assert_eq!(h.requests.0.borrow().len(), 3);
        assert_eq!(*runs.borrow(), ["hello"]);
        let results: Vec<Value> = {
            let messages = h.session.messages();
            let messages = messages.read().unwrap();
            messages
                .iter()
                .map(|message| serde_json::to_value(message).unwrap())
                .filter(|record| record["role"] == "toolResult")
                .collect()
        };
        assert_eq!(results.len(), 1);
        assert_eq!(get_message_text(&results[0]), "echo:hello");
        assert_eq!(results[0]["details"], json!({"text": "hello"}));
        assert_eq!(
            get_assistant_texts(&h).last().map(String::as_str),
            Some("final answer")
        );
        assert!(!h.session.is_streaming());
        start_prompt(&h, "follow-up").await.unwrap().unwrap();
        assert_eq!(h.requests.0.borrow().len(), 4);
    });
}

#[test]
fn extensions_precede_public_message_events() {
    let expected = corpus("retry_event_sequences")["extension"].clone();
    on_task_set(|_| async {
        let order = Rc::new(RefCell::new(Vec::new()));
        let h = harness(HarnessOptions {
            factories: vec![recording_messages(Rc::clone(&order))],
            ..scripted(vec![Scripted::text("done")])
        })
        .await;
        record_public_messages(&h.session, Rc::clone(&order));
        let ended = session_run_ended(&h);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        ended.notified().await;
        assert_eq!(json!(*order.borrow()), expected);
    });
}

#[test]
fn plain_prompt_keeps_event_sequence() {
    let expected = corpus("retry_event_sequences")["plain"].clone();
    on_task_set(|_| async {
        let h = harness(scripted(vec![Scripted::text("hello")])).await;
        let order = event_order(&h.session);
        let ended = session_run_ended(&h);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        ended.notified().await;
        assert_eq!(json!(*order.borrow()), expected);
    });
}

#[test]
fn tool_turn_keeps_event_sequence() {
    let expected = corpus("retry_event_sequences")["tool"].clone();
    on_task_set(|_| async {
        let runs = Rc::new(RefCell::new(Vec::new()));
        let h = harness(HarnessOptions {
            custom_tools: vec![recorded_echo_tool(Rc::clone(&runs))],
            ..scripted(vec![Scripted::echo_call("hello"), Scripted::text("done")])
        })
        .await;
        let order = event_order(&h.session);
        let ended = session_run_ended(&h);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        ended.notified().await;
        assert_eq!(*runs.borrow(), ["hello"]);
        assert_eq!(json!(*order.borrow()), expected);
    });
}

#[test]
fn stream_updates_keep_all_content_delta_kinds() {
    let expected = corpus("retry_event_sequences")["deltas"].clone();
    on_task_set(|_| async {
        let h = harness(scripted(vec![
            Scripted {
                thinking: Some("plan".into()),
                text: Some("answer".into()),
                tool_calls: vec![("echo".into(), json!({"text": "hello"}))],
                ..Scripted::default()
            },
            Scripted::text("done"),
        ]))
        .await;
        let updates = first_response_updates(&h.session);
        let ended = session_run_ended(&h);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        ended.notified().await;
        let updates = updates.borrow();
        let mut kinds: Vec<Value> = Vec::new();
        for update in updates.iter() {
            let kind = serde_json::to_value(update).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned();
            if kind != "start" && !kinds.iter().any(|seen| *seen == json!(kind)) {
                kinds.push(json!(kind));
            }
        }
        assert_eq!(json!(kinds), expected);
        let (mut thinking, mut text, mut toolcall) = (String::new(), String::new(), String::new());
        for update in updates.iter() {
            match update {
                AssistantMessageEvent::ThinkingDelta { delta, .. } => thinking.push_str(delta),
                AssistantMessageEvent::TextDelta { delta, .. } => text.push_str(delta),
                AssistantMessageEvent::ToolcallDelta { delta, .. } => toolcall.push_str(delta),
                _ => {}
            }
        }
        assert_eq!(thinking, "plan");
        assert_eq!(text, "answer");
        assert_eq!(
            serde_json::from_str::<Value>(&toolcall).unwrap(),
            json!({"text": "hello"})
        );
    });
}

#[test]
fn error_response_finishes_with_agent_end() {
    on_task_set(|_| async {
        let h = harness(scripted(vec![Scripted::failure("broken")])).await;
        let events = delivered_events(&h.session);
        let ended = session_run_ended(&h);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        ended.notified().await;
        let last = events.borrow().last().map(|event| event["type"].clone());
        assert_eq!(last, Some(json!("agent_end")));
    });
}

#[test]
fn abort_finishes_and_stores_partial_assistant() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let streamed = "x".repeat(20_000);
        script_responses(&h, vec![answer(&streamed, None, None)]);
        let events = delivered_events(&h.session);
        let updated = Rc::new(Notify::new());
        let notify = Rc::clone(&updated);
        drop(h.session.subscribe(Rc::new(move |event| {
            let AgentSessionEvent::Agent(event) = event else {
                return;
            };
            if matches!(*event.read().unwrap(), AgentEvent::MessageUpdate { .. }) {
                notify.notify_one();
            }
        })));
        let ended = session_run_ended(&h);
        let running = start_prompt(&h, "hi");
        updated.notified().await;
        h.session.abort().await;
        running.await.unwrap().unwrap();
        ended.notified().await;
        let last = events.borrow().last().map(|event| event["type"].clone());
        assert_eq!(last, Some(json!("agent_end")));
        let live = h.session.messages();
        let live = live.read().unwrap();
        let last = serde_json::to_value(live.last().unwrap()).unwrap();
        assert_eq!(last["role"], json!("assistant"));
        // The provider reports its own cancellation, so a broken route cannot pass as an abort.
        assert_eq!(last["errorMessage"], json!("Request was aborted"));
        assert_eq!(last["stopReason"], json!("aborted"));
        let partial = get_message_text(&last);
        assert!(
            streamed.starts_with(&partial),
            "it stops inside the response"
        );
        let manager = h.session.session_manager().borrow();
        let stored = manager
            .get_entries()
            .into_iter()
            .rfind(|entry| entry["type"] == "message")
            .cloned()
            .unwrap();
        assert_eq!(stored["message"]["role"], json!("assistant"));
        assert_eq!(stored["message"]["stopReason"], json!("aborted"));
    });
}
