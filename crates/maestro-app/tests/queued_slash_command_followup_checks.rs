//! A queued slash-command follow-up sent by an extension stays raw user text: it reaches the
//! model after the ordinary continuation instead of running the registered command, compared
//! with the recorded operation case.
#![cfg(not(target_arch = "wasm32"))]
use crate::agent_session_queue_checks::Held;
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, Transcript, command_extension, expected, input_sources,
    on_task_set, transcribed, waiting_tool,
};
use indexmap::IndexMap;
use maestro_app::AgentSessionEvent;
use maestro_extensions::{ExtensionAPI, ExtensionFactory};
use maestro_models::{Local, UserContent};
use maestro_session::messages::{SendUserMessageOptions, UserMessageDelivery};
use serde_json::json;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::Notify;

/// The extension that keeps its own API alive and registers `testcmd`, which records the
/// arguments it receives.
fn capturing_command_extension(
    saved: &Rc<RefCell<Option<ExtensionAPI<Local>>>>,
    heard: &Rc<RefCell<Option<String>>>,
) -> ExtensionFactory<Local> {
    let keep = Rc::clone(saved);
    let command = command_extension("testcmd", Rc::clone(heard), None);
    Rc::new(move |api| {
        *keep.borrow_mut() = Some(api.clone());
        command(api)
    })
}

/// A session over `factory` whose first answer calls the waiting tool.
async fn held_session(
    factory: ExtensionFactory<Local>,
    started: &Rc<Notify>,
    release: &Rc<Notify>,
) -> (Harness, Transcript) {
    transcribed(HarnessOptions {
        factories: vec![factory],
        base_tools_override: Some(IndexMap::from([(
            "wait".to_owned(),
            waiting_tool(Rc::clone(started), Rc::clone(release)),
        )])),
        responses: Some(vec![
            Scripted {
                tool_calls: vec![("wait".into(), json!({}))],
                ..Scripted::default()
            },
            Scripted::text("first turn complete"),
            Scripted::text("queued follow-up handled by model"),
        ]),
        ..HarnessOptions::default()
    })
    .await
}

/// Notified once the session queues `text` as a follow-up.
fn follow_up_queued(h: &Harness, text: &'static str) -> Rc<Notify> {
    let queued = Rc::new(Notify::new());
    let signal = Rc::clone(&queued);
    drop(h.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::QueueUpdate { follow_up, .. } = event
            && follow_up.iter().any(|queued| queued == text)
        {
            signal.notify_one();
        }
    })));
    queued
}

#[test]
fn extension_slash_follow_up_stays_user_text() {
    on_task_set(|_| async {
        let expected = expected("extension_slash_follow_up_stays_user_text");
        let saved = Rc::new(RefCell::new(None));
        let heard = Rc::new(RefCell::new(None));
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let factory = capturing_command_extension(&saved, &heard);
        let (h, transcript) = held_session(factory, &started, &release).await;
        let queued = follow_up_queued(&h, "/testcmd queued");
        let held = Held::begin(h, started, release).await;
        saved
            .borrow_mut()
            .take()
            .unwrap()
            .send_user_message(
                UserContent::Text("/testcmd queued".into()),
                Some(SendUserMessageOptions {
                    deliver_as: Some(UserMessageDelivery::FollowUp),
                }),
            )
            .unwrap();
        queued.notified().await;
        let mut observed = held.finish().await;
        let runs: Vec<String> = heard.borrow().iter().cloned().collect();
        observed["runs"] = json!(runs);
        observed["sources"] = json!(input_sources(&transcript));
        assert_eq!(observed, expected);
    });
}
