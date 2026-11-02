//! The delivery choice input extensions hear: present only for input that arrives while a
//! response streams.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, Scripted, harness, input_extension, on_task_set, start_prompt, waiting_tool,
};
use indexmap::IndexMap;
use maestro_app::PromptOptions;
use maestro_session::Presence;
use maestro_session::messages::UserMessageDelivery;
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::Notify;

/// The delivery choice each input event carried, in arrival order; absent as `None`.
type Choices = Rc<RefCell<Vec<Option<&'static str>>>>;

/// An extension recording the delivery choice of every input event.
fn recording(choices: &Choices) -> maestro_extensions::ExtensionFactory<maestro_models::Local> {
    let choices = Rc::clone(choices);
    input_extension(move |event| {
        let choice = match event.streaming_behavior {
            Presence::Present(UserMessageDelivery::Steer) => Some("steer"),
            Presence::Present(UserMessageDelivery::FollowUp) => Some("followUp"),
            _ => None,
        };
        choices.borrow_mut().push(choice);
        Ok(None)
    })
}

/// Options asking for `behavior` when the input arrives during a response.
fn choosing(behavior: UserMessageDelivery) -> PromptOptions {
    PromptOptions {
        streaming_behavior: Some(behavior),
        ..PromptOptions::default()
    }
}

#[test]
fn idle_input_carries_no_delivery_choice() {
    on_task_set(|_| async {
        let choices = Choices::default();
        let h = harness(HarnessOptions {
            factories: vec![recording(&choices)],
            responses: Some(vec![Scripted::text("done")]),
            ..HarnessOptions::default()
        })
        .await;
        let options = choosing(UserMessageDelivery::Steer);
        h.session.prompt("hello", options).await.unwrap();
        assert_eq!(*choices.borrow(), [None]);
    });
}

#[test]
fn streaming_input_carries_its_delivery_choice() {
    on_task_set(|_| async {
        let choices = Choices::default();
        let (started, release) = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let tools = IndexMap::from([(
            "wait".to_owned(),
            waiting_tool(Rc::clone(&started), Rc::clone(&release)),
        )]);
        let h = harness(HarnessOptions {
            factories: vec![recording(&choices)],
            base_tools_override: Some(tools),
            responses: Some(vec![
                Scripted {
                    tool_calls: vec![("wait".into(), json!({}))],
                    ..Scripted::default()
                },
                Scripted::text("steered"),
                Scripted::text("followed up"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let running = start_prompt(&h, "first");
        started.notified().await;
        for behavior in [UserMessageDelivery::Steer, UserMessageDelivery::FollowUp] {
            h.session.prompt("more", choosing(behavior)).await.unwrap();
        }
        release.notify_one();
        running.await.unwrap().unwrap();
        assert_eq!(*choices.borrow(), [None, Some("steer"), Some("followUp")]);
    });
}
