//! A custom message that does not trigger a turn, sent while a run is active, is recorded
//! once the current turn ends, in the same order in the history and in the message events.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, Scripted, harness, hold_first_answer, on_task_set, start_prompt, waiting_tool,
};
use indexmap::IndexMap;
use maestro_app::AgentSessionEvent;
use maestro_models::UserContent;
use maestro_session::Presence;
use maestro_session::messages::{CustomMessageInput, SendMessageOptions};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::Notify;

/// A custom message that only records `text`.
fn note(text: &str) -> CustomMessageInput {
    CustomMessageInput {
        custom_type: "note".into(),
        content: UserContent::Text(text.into()),
        display: true,
        details: Presence::Missing,
    }
}

/// Options for a message that starts no turn.
fn recording_only() -> SendMessageOptions {
    SendMessageOptions {
        trigger_turn: Some(false),
        deliver_as: None,
    }
}

/// The roles of the live history, in order.
fn history_roles(session: &maestro_app::AgentSession) -> Vec<String> {
    let messages = session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .map(|message| {
            serde_json::to_value(message).unwrap()["role"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// The roles of the messages the listeners heard finish, in order.
fn heard_roles(session: &maestro_app::AgentSession) -> Rc<RefCell<Vec<String>>> {
    let heard = Rc::new(RefCell::new(Vec::new()));
    let record = Rc::clone(&heard);
    drop(session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && let maestro_agent::AgentEvent::MessageEnd { message } = &*event.read().unwrap()
        {
            let role = serde_json::to_value(message).unwrap()["role"].clone();
            record.borrow_mut().push(role.as_str().unwrap().to_owned());
        }
    })));
    heard
}

#[test]
fn message_sent_during_a_response_is_recorded_without_another_turn() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let heard = heard_roles(&h.session);
        let (requested, release) = hold_first_answer(&h, "answer", &["an extra turn"]);
        let running = start_prompt(&h, "question");
        requested.notified().await;
        h.session
            .send_custom_message(note("aside"), recording_only())
            .await
            .unwrap();
        release.notify_one();
        running.await.unwrap().unwrap();
        assert_eq!(history_roles(&h.session), ["user", "assistant", "custom"]);
        assert_eq!(*heard.borrow(), ["user", "assistant", "custom"]);
    });
}

#[test]
fn message_sent_during_a_tool_follows_the_turns_tool_results() {
    on_task_set(|_| async {
        let (started, release) = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let tools = IndexMap::from([(
            "wait".to_owned(),
            waiting_tool(Rc::clone(&started), Rc::clone(&release)),
        )]);
        let h = harness(HarnessOptions {
            base_tools_override: Some(tools),
            responses: Some(vec![
                Scripted {
                    tool_calls: vec![("wait".into(), json!({}))],
                    ..Scripted::default()
                },
                Scripted::text("final"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let heard = heard_roles(&h.session);
        let running = start_prompt(&h, "question");
        started.notified().await;
        h.session
            .send_custom_message(note("aside"), recording_only())
            .await
            .unwrap();
        release.notify_one();
        running.await.unwrap().unwrap();
        let expected = ["user", "assistant", "toolResult", "custom", "assistant"];
        assert_eq!(history_roles(&h.session), expected);
        assert_eq!(*heard.borrow(), expected);
        assert_eq!(
            h.requests.0.borrow().len(),
            2,
            "no turn beyond the tool's follow-up"
        );
    });
}
