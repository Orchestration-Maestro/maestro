//! What session listeners hear while `holdUncheckedText` is on, and that they hear the agent's
//! events unchanged while it is off.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, echo_tools, harness, on_task_set, prompt_to_end,
};
use indexmap::IndexMap;
use maestro_agent::{AgentMessage, AgentToolRecord, AgentToolResult, SharedAgentTool};
use maestro_app::{AgentSessionEvent, PromptOptions};
use maestro_extensions::{
    ExtensionFactory, MessageEndEvent, MessageEndEventResult, NativeAgentMessage,
};
use maestro_models::{Local, TextContent, Tool, UserBlock};
use maestro_session::Presence;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Reply text the model sends before any check.
const UNCHECKED: &str = "unchecked-reply";
/// Reasoning text the model sends before any check.
const REASONING: &str = "unchecked-reasoning";
/// Personal data a tool reports while it runs.
const SIN: &str = "046-454-286";

/// Options for a session answering `responses` with the hold switched `hold`.
fn options(hold: bool, responses: Vec<Scripted>) -> HarnessOptions {
    HarnessOptions {
        settings: json!({"holdUncheckedText": hold}),
        responses: Some(responses),
        ..HarnessOptions::default()
    }
}

/// A reply with reasoning and text, then one call of the `echo` tool, as the first answer.
fn reply_with_call() -> Scripted {
    Scripted {
        thinking: Some(REASONING.into()),
        text: Some(UNCHECKED.into()),
        tool_calls: vec![("echo".into(), json!({"text": "hi"}))],
        ..Scripted::default()
    }
}

/// Starts recording each agent event the session's listeners hear, serialized, in order.
fn record_heard(h: &Harness) -> Rc<RefCell<Vec<String>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    drop(h.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event {
            seen.borrow_mut()
                .push(serde_json::to_string(&*event.read().unwrap()).unwrap());
        }
    })));
    events
}

/// Starts recording each event the agent delivers to its own listeners, serialized, in order.
fn record_delivered(h: &Harness) -> Rc<RefCell<Vec<String>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    drop(h.session.agent().subscribe(Rc::new(move |event, _| {
        seen.borrow_mut()
            .push(serde_json::to_string(&*event.read().unwrap()).unwrap());
        Box::pin(std::future::ready(Ok(())))
    })));
    events
}

/// Runs one prompt on a session over `options` and returns every agent event its listeners
/// heard, serialized, in order.
async fn heard(options: HarnessOptions) -> Vec<String> {
    let h = harness(options).await;
    let events = record_heard(&h);
    prompt_to_end(&h, "hello", PromptOptions::default()).await;
    events.take()
}

/// The record of a serialized event.
fn record(event: &str) -> Value {
    serde_json::from_str(event).unwrap()
}

/// Each event's type, followed by its model event's type for a message update.
fn kinds(events: &[String]) -> Vec<String> {
    events
        .iter()
        .map(|event| {
            let record = record(event);
            let kind = record["type"].as_str().unwrap();
            match record["assistantMessageEvent"]["type"].as_str() {
                Some(inner) => format!("{kind}:{inner}"),
                None => kind.to_owned(),
            }
        })
        .collect()
}

/// The position of the first finished assistant message.
fn first_assistant_end(events: &[String]) -> usize {
    events
        .iter()
        .position(|event| {
            let record = record(event);
            record["type"] == "message_end" && record["message"]["role"] == "assistant"
        })
        .unwrap()
}

/// Fails when any of `events` carries `text`.
fn assert_none_carry(events: &[String], text: &str) {
    for event in events {
        assert!(
            !event.contains(text),
            "{text} reached a listener in {event}"
        );
    }
}

/// A tool that reports `SIN` in the text and the details of its progress, then answers
/// `checked result`.
fn reporting_tools() -> IndexMap<String, SharedAgentTool<Local>> {
    let tool = AgentToolRecord::<Local> {
        definition: Tool {
            name: "report".into(),
            description: "Reports progress".into(),
            parameters: json!({"type": "object", "properties": {}}).into(),
        },
        description: None,
        label: "report".into(),
        prepare_arguments: None,
        execute: Rc::new(|_, _, _, on_update| {
            if let Some(update) = on_update {
                update(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(format!("SIN {SIN}")))],
                    details: Some(json!({"sin": SIN})),
                    terminate: None,
                });
            }
            let done = TextContent::new("checked result".into());
            Box::pin(std::future::ready(Ok(AgentToolResult {
                content: vec![UserBlock::Text(done)],
                details: Some(json!({})),
                terminate: None,
            })))
        }),
        execution_mode: None,
    };
    IndexMap::from([("report".to_owned(), Rc::new(RwLock::new(tool)))])
}

/// The extension that rewrites each finished assistant message to the text `checked-reply`.
fn rewrite_reply() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        let registered = api.on::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
            "message_end".into(),
            Rc::new(|event, _| {
                let result = if let AgentMessage::Assistant(answer) = &event.message {
                    let mut message = answer.read().unwrap().clone();
                    message.content = vec![maestro_models::AssistantContent::Text(
                        TextContent::new("checked-reply".into()),
                    )];
                    Some(MessageEndEventResult {
                        message: Presence::Present(AgentMessage::Assistant(Arc::new(RwLock::new(
                            message,
                        )))),
                    })
                } else {
                    None
                };
                Box::pin(std::future::ready(Ok(result)))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

#[test]
fn text_and_thinking_deltas_wait_for_the_message_end() {
    on_task_set(|_| async {
        let reply = Scripted {
            thinking: Some(REASONING.into()),
            text: Some(UNCHECKED.into()),
            ..Scripted::default()
        };
        let events = heard(options(true, vec![reply])).await;
        let end = first_assistant_end(&events);
        assert_none_carry(&events[..end], UNCHECKED);
        assert_none_carry(&events[..end], REASONING);
        assert!(events[end].contains(UNCHECKED) && events[end].contains(REASONING));
        assert!(kinds(&events[..end]).contains(&"message_start".to_owned()));
    });
}

#[test]
fn a_response_that_starts_whole_is_withheld_until_the_message_end() {
    on_task_set(|_| async {
        let reply = Scripted {
            thinking: Some(REASONING.into()),
            text: Some(UNCHECKED.into()),
            terminal_only: true,
            ..Scripted::default()
        };
        let events = heard(options(true, vec![reply])).await;
        let end = first_assistant_end(&events);
        assert_none_carry(&events[..end], UNCHECKED);
        assert_none_carry(&events[..end], REASONING);
        assert!(events[end].contains(UNCHECKED) && events[end].contains(REASONING));
        assert!(kinds(&events[..end]).contains(&"message_start".to_owned()));
    });
}

#[test]
fn tool_call_updates_keep_flowing_without_the_text_beside_the_call() {
    on_task_set(|_| async {
        let executed = Rc::new(RefCell::new(false));
        let mut held = options(true, vec![reply_with_call(), Scripted::text("done")]);
        held.base_tools_override = Some(echo_tools(executed));
        let events = heard(held).await;
        let end = first_assistant_end(&events);
        assert_none_carry(&events[..end], UNCHECKED);
        assert_none_carry(&events[..end], REASONING);
        let ends: Vec<Value> = events[..end]
            .iter()
            .map(|event| record(event))
            .filter(|event| event["assistantMessageEvent"]["type"] == "toolcall_end")
            .collect();
        assert_eq!(ends.len(), 1);
        assert_eq!(ends[0]["assistantMessageEvent"]["toolCall"]["name"], "echo");
        let blocks = ends[0]["message"]["content"].as_array().unwrap();
        assert!(blocks.iter().any(|block| block["type"] == "toolCall"));
        assert!(events[end].contains(UNCHECKED));
    });
}

#[test]
fn partial_tool_results_wait_for_the_completed_result() {
    on_task_set(|_| async {
        let call = Scripted {
            tool_calls: vec![("report".into(), json!({}))],
            ..Scripted::default()
        };
        let mut held = options(true, vec![call, Scripted::text("done")]);
        held.base_tools_override = Some(reporting_tools());
        let events = heard(held).await;
        assert_none_carry(&events, SIN);
        let kinds = kinds(&events);
        assert!(kinds.contains(&"tool_execution_start".to_owned()));
        assert!(kinds.contains(&"tool_execution_update".to_owned()));
        let finished = events
            .iter()
            .find(|event| record(event)["type"] == "tool_execution_end")
            .unwrap();
        assert!(finished.contains("checked result"));
    });
}

#[test]
fn listeners_hear_only_the_message_an_extension_rewrote() {
    on_task_set(|_| async {
        let reply = Scripted {
            thinking: Some(REASONING.into()),
            text: Some(UNCHECKED.into()),
            ..Scripted::default()
        };
        let mut held = options(true, vec![reply]);
        held.factories = vec![rewrite_reply()];
        let events = heard(held).await;
        assert_none_carry(&events, UNCHECKED);
        assert_none_carry(&events, REASONING);
        let end = first_assistant_end(&events);
        assert!(events[end].contains("checked-reply"));
        let last = events.last().unwrap();
        assert_eq!(record(last)["type"], "agent_end");
        assert!(last.contains("checked-reply"));
    });
}

#[test]
fn tool_call_and_status_events_arrive_while_text_is_held() {
    on_task_set(|_| async {
        let script = || vec![reply_with_call(), Scripted::text("done")];
        let run = |hold| {
            let mut run = options(hold, script());
            run.base_tools_override = Some(echo_tools(Rc::new(RefCell::new(false))));
            run
        };
        let open = kinds(&heard(run(false)).await);
        let held = kinds(&heard(run(true)).await);
        let withheld = |kind: &&String| {
            kind.starts_with("message_update:text_") || kind.starts_with("message_update:thinking_")
        };
        let expected: Vec<&String> = open.iter().filter(|kind| !withheld(kind)).collect();
        assert!(open.iter().any(|kind| withheld(&kind)));
        assert!(expected.iter().any(|kind| kind.ends_with("toolcall_end")));
        assert_eq!(held.iter().collect::<Vec<_>>(), expected);
    });
}

#[test]
fn listeners_hear_the_agent_events_unchanged_while_nothing_is_held() {
    on_task_set(|_| async {
        for settings in [json!({}), json!({"holdUncheckedText": false})] {
            let mut open = options(false, vec![reply_with_call(), Scripted::text("done")]);
            open.settings = settings;
            open.base_tools_override = Some(echo_tools(Rc::new(RefCell::new(false))));
            let h = harness(open).await;
            let (agent, session) = (record_delivered(&h), record_heard(&h));
            prompt_to_end(&h, "hello", PromptOptions::default()).await;
            assert!(
                kinds(&agent.borrow())
                    .iter()
                    .any(|kind| kind.contains("text_delta"))
            );
            assert_eq!(*agent.borrow(), *session.borrow());
        }
    });
}
