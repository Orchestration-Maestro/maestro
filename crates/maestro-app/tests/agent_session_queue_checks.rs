//! Steering and follow-up delivery through a real application session: one-at-a-time and
//! batched queues, custom messages, the pending mirrors and the command refusals, compared
//! with the recorded operation cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, command_extension, delivery_requests, delivery_view,
    expected, get_message_text, harness, input_sources, observed_preflight, on_task_set,
    session_run_ended, start_prompt, waiting_tool,
};
use indexmap::IndexMap;
use maestro_agent::{AgentEvent, QueueMode, SharedAgentTool};
use maestro_app::{AgentSessionEvent, PromptOptions};
use maestro_models::{DiagnosticErrorInfo, Local, UserContent};
use maestro_session::Presence;
use maestro_session::messages::{
    CustomMessageInput, MessageDelivery, SendMessageOptions, UserMessageDelivery,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use tokio::sync::Notify;

/// The tools holding a run: the waiting tool alone.
fn waiting_tools(
    started: &Rc<Notify>,
    release: &Rc<Notify>,
) -> IndexMap<String, SharedAgentTool<Local>> {
    IndexMap::from([(
        "wait".to_owned(),
        waiting_tool(Rc::clone(started), Rc::clone(release)),
    )])
}

/// The first answer of a held run: a call to the waiting tool.
fn waiting_answer() -> Scripted {
    Scripted {
        tool_calls: vec![("wait".into(), json!({}))],
        ..Scripted::default()
    }
}

/// A run whose first answer calls the waiting tool, held until the test releases it.
pub struct Held {
    /// The session under the held run.
    h: Harness,
    /// Releases the tool.
    release: Rc<Notify>,
    /// Notified once the session delivered the run's end.
    ended: Rc<Notify>,
    /// The running prompt.
    running: tokio::task::JoinHandle<Result<(), DiagnosticErrorInfo>>,
}

impl Held {
    /// Starts the run on the current task set and waits for the tool to begin.
    pub async fn begin(h: Harness, started: Rc<Notify>, release: Rc<Notify>) -> Self {
        let ended = session_run_ended(&h);
        let running = start_prompt(&h, "start");
        started.notified().await;
        Self {
            h,
            release,
            ended,
            running,
        }
    }

    /// The session under the held run.
    pub fn harness(&self) -> &Harness {
        &self.h
    }

    /// Releases the tool and returns the delivered history, the queue mirrors while it was
    /// held and the actual provider requests.
    pub async fn finish(self) -> Value {
        let during = json!({
            "pending": self.h.session.pending_message_count(),
            "steering": self.h.session.get_steering_messages(),
            "followUp": self.h.session.get_follow_up_messages(),
        });
        self.release.notify_one();
        self.running.await.unwrap().unwrap();
        self.h.session.agent().wait_for_idle().await;
        self.ended.notified().await;
        let mut observed = delivery_view(&self.h);
        observed["during"] = during;
        observed["requests"] = delivery_requests(&self.h);
        self.h.session.dispose();
        observed
    }
}

#[test]
fn idle_command_needs_no_scripted_answer() {
    on_task_set(|_| async {
        let expected = expected("idle_command_needs_no_scripted_answer");
        let heard = Rc::new(RefCell::new(None));
        let h = harness(HarnessOptions {
            factories: vec![command_extension("testcmd", Rc::clone(&heard), None)],
            ..HarnessOptions::default()
        })
        .await;
        let (accepted, options) = observed_preflight();
        let result = h.session.prompt("/testcmd hello world", options).await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["runs"] = json!([*heard.borrow()]);
        observed["response_pending"] = json!(h.faux.get_pending_response_count());
        observed["preflight"] = json!(*accepted.borrow());
        observed["calls"] = json!(h.faux.state.call_count());
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn extension_steering_precedes_the_next_request() {
    on_task_set(|_| async {
        let expected = expected("extension_steering_precedes_the_next_request");
        let saved = Rc::new(RefCell::new(None));
        let keep = Rc::clone(&saved);
        let factory: maestro_extensions::ExtensionFactory<Local> = Rc::new(move |api| {
            *keep.borrow_mut() = Some(api);
            Box::pin(std::future::ready(Ok(())))
        });
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let (h, t) = crate::support::session_harness::transcribed(HarnessOptions {
            factories: vec![factory],
            base_tools_override: Some(waiting_tools(&started, &release)),
            responses: Some(vec![waiting_answer(), Scripted::text("saw steer")]),
            ..HarnessOptions::default()
        })
        .await;
        let queued = Rc::new(Notify::new());
        let signal = Rc::clone(&queued);
        drop(h.session.subscribe(Rc::new(move |event| {
            if let AgentSessionEvent::QueueUpdate { steering, .. } = event
                && steering.iter().any(|text| text == "steer now")
            {
                signal.notify_one();
            }
        })));
        let held = Held::begin(h, started, release).await;
        saved
            .borrow_mut()
            .take()
            .unwrap()
            .send_user_message(
                UserContent::Text("steer now".into()),
                Some(maestro_session::messages::SendUserMessageOptions {
                    deliver_as: Some(UserMessageDelivery::Steer),
                }),
            )
            .unwrap();
        queued.notified().await;
        let mut observed = held.finish().await;
        observed["sources"] = json!(input_sources(&t));
        assert_eq!(observed, expected);
    });
}

#[test]
fn follow_up_waits_for_ordinary_continuation() {
    on_task_set(|_| async {
        let expected = expected("follow_up_waits_for_ordinary_continuation");
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let h = harness(HarnessOptions {
            base_tools_override: Some(waiting_tools(&started, &release)),
            responses: Some(vec![
                waiting_answer(),
                Scripted::text("follow-up response"),
                Scripted::default(),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let held = Held::begin(h, started, release).await;
        let result = held.harness().session.follow_up("after current run", &[]);
        assert!(result.is_ok());
        assert_eq!(held.finish().await, expected);
    });
}

/// Runs `order` as steering messages, then returns the delivered observations.
async fn steering_run(order: &[&str]) -> Value {
    let started = Rc::new(Notify::new());
    let release = Rc::new(Notify::new());
    let mut responses = vec![waiting_answer()];
    responses.extend((1..=order.len()).map(|n| Scripted::text(&format!("handled steer {n}"))));
    let h = harness(HarnessOptions {
        base_tools_override: Some(waiting_tools(&started, &release)),
        responses: Some(responses),
        ..HarnessOptions::default()
    })
    .await;
    let held = Held::begin(h, started, release).await;
    held.harness()
        .session
        .set_steering_mode(QueueMode::OneAtATime)
        .unwrap();
    held.harness()
        .session
        .set_follow_up_mode(QueueMode::All)
        .unwrap();
    for text in order {
        assert!(held.harness().session.steer(text, &[]).is_ok());
    }
    held.finish().await
}

#[test]
fn steering_one_at_a_time_keeps_fifo() {
    on_task_set(|_| async {
        let expected = expected("steering_one_at_a_time_keeps_fifo");
        assert_eq!(steering_run(&["steer 1", "steer 2"]).await, expected);
        let reversed = steering_run(&["steer 2", "steer 1"]).await;
        assert_ne!(reversed["users"], expected["users"]);
        assert_ne!(reversed["requests"], expected["requests"]);
        let dropped = steering_run(&["steer 1"]).await;
        assert_ne!(dropped["users"], expected["users"]);
        assert_ne!(dropped["requests"], expected["requests"]);
    });
}

#[test]
fn follow_up_one_at_a_time_keeps_fifo() {
    on_task_set(|_| async {
        let expected = expected("follow_up_one_at_a_time_keeps_fifo");
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let h = harness(HarnessOptions {
            base_tools_override: Some(waiting_tools(&started, &release)),
            responses: Some(vec![
                waiting_answer(),
                Scripted::text("original turn complete"),
                Scripted::text("handled follow-up 1"),
                Scripted::text("handled follow-up 2"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let held = Held::begin(h, started, release).await;
        held.harness()
            .session
            .set_follow_up_mode(QueueMode::OneAtATime)
            .unwrap();
        held.harness()
            .session
            .set_steering_mode(QueueMode::All)
            .unwrap();
        assert!(held.harness().session.follow_up("follow-up 1", &[]).is_ok());
        assert!(held.harness().session.follow_up("follow-up 2", &[]).is_ok());
        assert_eq!(held.finish().await, expected);
    });
}

#[test]
fn steering_all_keeps_one_batch() {
    on_task_set(|_| async {
        let expected = expected("steering_all_keeps_one_batch");
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let h = harness(HarnessOptions {
            base_tools_override: Some(waiting_tools(&started, &release)),
            responses: Some(vec![
                waiting_answer(),
                Scripted::text("batched steer response"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let held = Held::begin(h, started, release).await;
        held.harness()
            .session
            .set_steering_mode(QueueMode::All)
            .unwrap();
        held.harness()
            .session
            .set_follow_up_mode(QueueMode::OneAtATime)
            .unwrap();
        assert!(held.harness().session.steer("steer 1", &[]).is_ok());
        assert!(held.harness().session.steer("steer 2", &[]).is_ok());
        assert_eq!(held.finish().await, expected);
    });
}

#[test]
fn follow_up_all_keeps_one_batch() {
    on_task_set(|_| async {
        let expected = expected("follow_up_all_keeps_one_batch");
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let h = harness(HarnessOptions {
            base_tools_override: Some(waiting_tools(&started, &release)),
            responses: Some(vec![
                waiting_answer(),
                Scripted::text("original turn complete"),
                Scripted::text("batched follow-up response"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let held = Held::begin(h, started, release).await;
        held.harness()
            .session
            .set_follow_up_mode(QueueMode::All)
            .unwrap();
        held.harness()
            .session
            .set_steering_mode(QueueMode::OneAtATime)
            .unwrap();
        assert!(held.harness().session.follow_up("follow-up 1", &[]).is_ok());
        assert!(held.harness().session.follow_up("follow-up 2", &[]).is_ok());
        assert_eq!(held.finish().await, expected);
    });
}

/// A custom message of the two streaming cases.
fn custom(text: &str) -> CustomMessageInput {
    CustomMessageInput {
        custom_type: "queue-test".into(),
        content: UserContent::Text(text.into()),
        display: true,
        details: Presence::Present(json!({"value": 1})),
    }
}

/// Options delivering a custom message as `delivery`.
fn custom_options(delivery: MessageDelivery) -> SendMessageOptions {
    SendMessageOptions {
        trigger_turn: None,
        deliver_as: Some(delivery),
    }
}

#[test]
fn streaming_custom_steering_reaches_the_provider() {
    on_task_set(|_| async {
        let expected = expected("streaming_custom_steering_reaches_the_provider");
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let h = harness(HarnessOptions {
            base_tools_override: Some(waiting_tools(&started, &release)),
            responses: Some(vec![waiting_answer(), Scripted::text("done")]),
            ..HarnessOptions::default()
        })
        .await;
        let held = Held::begin(h, started, release).await;
        let result = held
            .harness()
            .session
            .send_custom_message(
                custom("steer custom"),
                custom_options(MessageDelivery::Steer),
            )
            .await;
        assert!(result.is_ok());
        assert_eq!(held.finish().await, expected);
    });
}

#[test]
fn streaming_custom_follow_up_waits_for_continuation() {
    on_task_set(|_| async {
        let expected = expected("streaming_custom_follow_up_waits_for_continuation");
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let h = harness(HarnessOptions {
            base_tools_override: Some(waiting_tools(&started, &release)),
            responses: Some(vec![
                waiting_answer(),
                Scripted::text("original turn complete"),
                Scripted::text("done"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let held = Held::begin(h, started, release).await;
        let result = held
            .harness()
            .session
            .send_custom_message(
                custom("follow-up custom"),
                custom_options(MessageDelivery::FollowUp),
            )
            .await;
        assert!(result.is_ok());
        assert_eq!(held.finish().await, expected);
    });
}

#[test]
fn next_turn_custom_message_follows_the_user() {
    on_task_set(|_| async {
        let expected = expected("next_turn_custom_message_follows_the_user");
        let h = harness(HarnessOptions {
            responses: Some(vec![Scripted::text("done")]),
            ..HarnessOptions::default()
        })
        .await;
        let note = CustomMessageInput {
            custom_type: "next-turn".into(),
            content: UserContent::Text("carry this".into()),
            display: true,
            details: Presence::Present(json!({})),
        };
        let result = h
            .session
            .send_custom_message(note, custom_options(MessageDelivery::NextTurn))
            .await;
        assert!(result.is_ok());
        let before = delivery_view(&h);
        let result = h
            .session
            .prompt("normal prompt", PromptOptions::default())
            .await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["requests"] = delivery_requests(&h);
        observed["before"] = before;
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

/// Records the pending count each time the session starts the user message `text`.
fn message_start_counts(h: &Harness, text: &'static str) -> Rc<RefCell<Vec<usize>>> {
    let counts = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&counts);
    let session: Weak<_> = Rc::downgrade(&h.session);
    drop(h.session.subscribe(Rc::new(move |event| {
        let started = match event {
            AgentSessionEvent::Agent(run) => match &*run.read().unwrap() {
                AgentEvent::MessageStart { message } => Some(message.clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(message) = started {
            let record = serde_json::to_value(message).unwrap();
            if record["role"] == "user"
                && get_message_text(&record) == text
                && let Some(session) = session.upgrade()
            {
                recorded.borrow_mut().push(session.pending_message_count());
            }
        }
    })));
    counts
}

#[test]
fn queued_text_leaves_the_mirror_before_start() {
    on_task_set(|_| async {
        let expected = expected("queued_text_leaves_the_mirror_before_start");
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let h = harness(HarnessOptions {
            base_tools_override: Some(waiting_tools(&started, &release)),
            responses: Some(vec![waiting_answer(), Scripted::text("done")]),
            ..HarnessOptions::default()
        })
        .await;
        let counts = message_start_counts(&h, "queued");
        let held = Held::begin(h, started, release).await;
        assert!(held.harness().session.steer("queued", &[]).is_ok());
        assert_eq!(held.harness().session.pending_message_count(), 1);
        let mut observed = held.finish().await;
        observed["counts"] = json!(*counts.borrow());
        assert_eq!(observed, expected);
    });
}

#[test]
fn registered_command_cannot_be_steered() {
    on_task_set(|_| async {
        let expected = expected("registered_command_cannot_be_steered");
        let heard = Rc::new(RefCell::new(None));
        let h = harness(HarnessOptions {
            factories: vec![command_extension("testcmd", Rc::clone(&heard), None)],
            ..HarnessOptions::default()
        })
        .await;
        let refused = h.session.steer("/testcmd queued", &[]);
        assert!(heard.borrow().is_none());
        let mut observed = delivery_view(&h);
        observed["error"] = json!(refused.unwrap_err().message);
        observed["calls"] = json!(h.faux.state.call_count());
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn registered_command_cannot_be_followed_up() {
    on_task_set(|_| async {
        let expected = expected("registered_command_cannot_be_followed_up");
        let heard = Rc::new(RefCell::new(None));
        let h = harness(HarnessOptions {
            factories: vec![command_extension("testcmd", Rc::clone(&heard), None)],
            ..HarnessOptions::default()
        })
        .await;
        let refused = h.session.follow_up("/testcmd queued", &[]);
        assert!(heard.borrow().is_none());
        let mut observed = delivery_view(&h);
        observed["error"] = json!(refused.unwrap_err().message);
        observed["calls"] = json!(h.faux.state.call_count());
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}
