//! Finalized assistant costs through the session's message consumers.
#![cfg(not(target_arch = "wasm32"))]

use crate::support::session_harness::{HarnessOptions, Scripted, corpus, harness, on_task_set};
use maestro_agent::{AgentEvent, AgentMessage};
use maestro_app::{AgentSessionEvent, PromptOptions};
use maestro_extensions::{
    ExtensionFactory, MessageEndEvent, MessageEndEventResult, NativeAgentMessage,
};
use maestro_models::Local;
use maestro_session::{Presence, session_manager::ReadonlySessionManager};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Replaces only the finalized assistant's total cost, leaving user messages alone.
fn replace_cost() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        let registered = api.on::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
            "message_end".into(),
            Rc::new(|event, _| {
                let result = if let AgentMessage::Assistant(answer) = &event.message {
                    let mut message = answer.read().unwrap().clone();
                    assert_ne!(json!(message.usage.cost.total), json!(0.123));
                    message.usage.cost.total = 0.123;
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

/// Records each settled message and turn boundary until the session is disposed.
fn message_boundaries(session: &maestro_app::AgentSession<Local>) -> Rc<RefCell<Vec<Value>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&events);
    drop(session.subscribe(Rc::new(move |event| {
        let AgentSessionEvent::Agent(event) = event else {
            return;
        };
        let event = event.read().unwrap();
        if matches!(
            &*event,
            AgentEvent::MessageEnd { .. }
                | AgentEvent::TurnEnd { .. }
                | AgentEvent::AgentEnd { .. }
        ) {
            seen.borrow_mut()
                .push(serde_json::to_value(&*event).unwrap());
        }
    })));
    events
}

#[test]
fn finalized_cost_replacement_reaches_every_message_consumer() {
    let expected = corpus("replacement");
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            factories: vec![replace_cost()],
            responses: Some(vec![Scripted::text("hello")]),
            ..HarnessOptions::default()
        })
        .await;
        let events = message_boundaries(&h.session);
        h.session
            .prompt("hi", PromptOptions::default())
            .await
            .unwrap();
        let retained = serde_json::to_value(&*h.session.messages().read().unwrap()).unwrap();
        assert_eq!(retained[0]["role"], "user");
        assert_eq!(
            retained[0]["content"],
            json!([{"type": "text", "text": "hi"}])
        );
        assert_eq!(retained[1]["content"][0]["text"], "hello");
        let events = events.borrow();
        assert_eq!(
            events
                .iter()
                .map(|event| event["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["message_end", "message_end", "turn_end", "agent_end"]
        );
        assert_eq!(events[0]["message"], retained[0]);
        let stored: Vec<Value> = h
            .session
            .session_manager()
            .borrow()
            .get_entries()
            .iter()
            .filter(|entry| entry["type"] == "message")
            .map(|entry| entry["message"].clone())
            .collect();
        for (key, messages) in [
            ("retained", retained),
            ("messageEnd", json!([events[1]["message"]])),
            ("history", json!(stored)),
            ("turnEnd", json!([events[2]["message"]])),
            ("agentEnd", events[3]["messages"].clone()),
        ] {
            let costs: Vec<Value> = messages
                .as_array()
                .unwrap()
                .iter()
                .filter(|message| message["role"] == "assistant")
                .map(|message| message["usage"]["cost"]["total"].clone())
                .collect();
            assert_eq!(json!(costs), expected["cost"][key], "{key}");
        }
    });
}
