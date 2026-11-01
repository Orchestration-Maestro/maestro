//! The session's agent-event dispatch: queued texts removed on delivery, extensions told
//! before listeners, rewritten finished messages, turn numbering, failing handlers, the retry
//! observation, tool interception, persistence, the paused link during a manual compaction
//! and disposal.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, agent_event_type, echo_tools, free_to_change, harness,
    hold_first_answer, on_paused_task_set, on_task_set, start_prompt,
};
use maestro_agent::{AgentEvent, AgentMessage};
use maestro_app::agent_session::PromptOptions;
use maestro_app::{AgentSession, AgentSessionEvent};
use maestro_extensions::{
    ExtensionFactory, MessageEndEvent, MessageEndEventResult, NativeAgentMessage, ToolCallEvent,
    ToolCallEventResult, ToolResultEvent, ToolResultEventResult, TurnEndEvent, TurnStartEvent,
};
use maestro_models::{Local, TextContent, UserBlock};
use maestro_session::Presence;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::RwLock};
use tokio::sync::Notify;

/// An extension answering `event` with `answer`, which sees the event.
pub(super) fn handling<V: 'static, R: 'static>(
    event: &'static str,
    answer: impl Fn(&mut V) -> Result<Option<R>, &'static str> + 'static,
) -> ExtensionFactory<Local> {
    let answer = Rc::new(answer);
    Rc::new(move |api| {
        let answer = Rc::clone(&answer);
        let registered = api.on::<V, R>(
            event.into(),
            Rc::new(move |event, _| {
                let result = answer(event).map_err(Into::into);
                Box::pin(std::future::ready(result))
            }),
        );
        Box::pin(std::future::ready(registered))
    })
}

/// An extension whose first `event` handler call reports reaching it and answers once
/// released; later calls answer at once. Each answered call is logged.
fn gated<V: 'static, R: 'static>(
    event: &'static str,
    reached: Rc<Notify>,
    release: Rc<Notify>,
    log: Rc<RefCell<Vec<String>>>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (reached, release, log) = (Rc::clone(&reached), Rc::clone(&release), Rc::clone(&log));
        let first = Rc::new(std::cell::Cell::new(true));
        let registered = api.on::<V, R>(
            event.into(),
            Rc::new(move |_, _| {
                let gate = first
                    .replace(false)
                    .then(|| (Rc::clone(&reached), Rc::clone(&release)));
                Box::pin(answer_after(gate, Rc::clone(&log), event))
            }),
        );
        Box::pin(std::future::ready(registered))
    })
}

/// Logs `event` as handled once `gate`, when given, is reached and released.
async fn answer_after<R>(
    gate: Option<(Rc<Notify>, Rc<Notify>)>,
    log: Rc<RefCell<Vec<String>>>,
    event: &'static str,
) -> Result<Option<R>, maestro_extensions::EventError> {
    if let Some((reached, release)) = gate {
        reached.notify_one();
        release.notified().await;
    }
    log.borrow_mut().push(format!("{event} handled"));
    Ok(None)
}

/// A session over the scripted stream with `factories` and `responses`.
async fn session(factories: Vec<ExtensionFactory<Local>>, responses: Vec<Scripted>) -> Harness {
    harness(HarnessOptions {
        factories,
        responses: Some(responses),
        ..HarnessOptions::default()
    })
    .await
}

/// Sends `text` and waits for the session to settle.
async fn prompt(h: &Harness, text: &str) {
    h.session
        .prompt(text, PromptOptions::default())
        .await
        .unwrap();
}

/// The text of a user or assistant message.
fn text_of(message: &NativeAgentMessage) -> String {
    let record = serde_json::to_value(message).unwrap();
    match &record["content"] {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect(),
        _ => String::new(),
    }
}

/// Records `label` with each delivered event's type and, for message events, its text.
fn log_events(h: &Harness, label: &'static str, log: &Rc<RefCell<Vec<String>>>) {
    let log = Rc::clone(log);
    drop(h.session.subscribe(Rc::new(move |event| {
        match event {
            AgentSessionEvent::Agent(event) => {
                let event = event.read().unwrap();
                let text = match &*event {
                    AgentEvent::MessageStart { message } | AgentEvent::MessageEnd { message } => {
                        format!(" {}", text_of(message))
                    }
                    _ => String::new(),
                };
                log.borrow_mut()
                    .push(format!("{label}:{}{text}", agent_event_type(&event)));
            }
            AgentSessionEvent::QueueUpdate {
                steering,
                follow_up,
            } => log
                .borrow_mut()
                .push(format!("{label}:queue {steering:?} {follow_up:?}")),
            _ => {}
        }
    })));
}

/// Queues `text` with `queue` while the first answer streams and returns what the listeners
/// recorded.
async fn delivered_queue(queue: fn(&Harness, &str)) -> (Harness, Vec<String>) {
    let h = harness(HarnessOptions::default()).await;
    let log = Rc::new(RefCell::new(Vec::new()));
    let (requested, release) = hold_first_answer(&h, "first", &["second"]);
    let running = start_prompt(&h, "first");
    requested.notified().await;
    queue(&h, "later");
    log_events(&h, "listener", &log);
    let mirrors = Rc::downgrade(&h.session);
    let seen = Rc::clone(&log);
    drop(h.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && let AgentEvent::MessageStart {
                message: AgentMessage::User(_),
            } = &*event.read().unwrap()
        {
            let session = mirrors
                .upgrade()
                .expect("the session outlives its listener");
            seen.borrow_mut().push(format!(
                "mirrors {:?} {:?}",
                session.get_steering_messages(),
                session.get_follow_up_messages()
            ));
        }
    })));
    release.notify_one();
    running.await.unwrap().unwrap();
    let log = log.borrow().clone();
    (h, log)
}

#[test]
fn removes_a_queued_steering_message_before_its_listeners_run() {
    on_task_set(|_| async {
        let (_h, log) = delivered_queue(|h, text| h.session.steer(text, &[]).unwrap()).await;
        let delivered = log
            .iter()
            .position(|line| line == "listener:message_start later")
            .unwrap();
        assert_eq!(log[delivered - 1], "listener:queue [] []");
        assert_eq!(log[delivered + 1], "mirrors [] []");
    });
}

#[test]
fn removes_a_queued_follow_up_after_steering() {
    on_task_set(|_| async {
        let (_h, log) = delivered_queue(|h, text| h.session.follow_up(text, &[]).unwrap()).await;
        let delivered = log
            .iter()
            .position(|line| line == "listener:message_start later")
            .unwrap();
        assert_eq!(log[delivered - 1], "listener:queue [] []");
        assert_eq!(log[delivered + 1], "mirrors [] []");
    });
}

#[test]
fn queue_listeners_can_change_the_delivered_user_message() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let (requested, release) = hold_first_answer(&h, "first", &["second"]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        h.session.steer("later", &[]).unwrap();
        let writable = Rc::new(RefCell::new(Vec::new()));
        let (state, seen) = (Rc::downgrade(h.session.state()), Rc::clone(&writable));
        drop(h.session.subscribe(Rc::new(move |event| {
            if let AgentSessionEvent::QueueUpdate { .. } = event
                && let Some(state) = state.upgrade()
                && let Some(AgentMessage::User(user)) = state.read().unwrap().streaming_message()
            {
                seen.borrow_mut().push(user.try_write().is_ok());
            }
        })));
        release.notify_one();
        running.await.unwrap().unwrap();
        assert_eq!(*writable.borrow(), [true]);
    });
}

#[test]
fn delivers_extension_events_before_session_listeners() {
    on_task_set(|_| async {
        let log = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&log);
        let extension = handling::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
            "message_end",
            move |event| {
                seen.borrow_mut()
                    .push(format!("extension:message_end {}", text_of(&event.message)));
                Ok(None)
            },
        );
        let h = session(vec![extension], vec![Scripted::text("answer")]).await;
        log_events(&h, "listener", &log);
        prompt(&h, "question").await;
        let ends: Vec<String> = log
            .borrow()
            .iter()
            .filter(|line| line.contains("message_end"))
            .cloned()
            .collect();
        assert_eq!(
            ends,
            [
                "extension:message_end question",
                "listener:message_end question",
                "extension:message_end answer",
                "listener:message_end answer",
            ]
        );
    });
}

#[test]
fn replaces_a_finalized_message_before_listeners_and_persistence() {
    on_task_set(|_| async {
        let extension = replacing_text();
        let h = session(vec![extension], vec![Scripted::text("original")]).await;
        let log = Rc::new(RefCell::new(Vec::new()));
        log_events(&h, "listener", &log);
        prompt(&h, "question").await;
        assert!(
            log.borrow()
                .contains(&"listener:message_end rewritten".to_owned())
        );
        let history = h.session.messages();
        assert_eq!(
            text_of(history.read().unwrap().last().unwrap()),
            "rewritten"
        );
        let manager = h.session.session_manager().borrow();
        let stored = manager.get_entries().last().unwrap()["message"]["content"][0]["text"].clone();
        assert_eq!(stored, json!("rewritten"));
    });
}

/// A scripted call of the echo tool.
fn echo_call() -> Scripted {
    Scripted {
        tool_calls: vec![("echo".into(), json!({"text": "hi"}))],
        ..Scripted::default()
    }
}

/// A session with the echo tool, `factories` and `responses`.
async fn echo_session(
    factories: Vec<ExtensionFactory<Local>>,
    responses: Vec<Scripted>,
) -> Harness {
    harness(HarnessOptions {
        factories,
        responses: Some(responses),
        base_tools_override: Some(echo_tools(Rc::default())),
        ..HarnessOptions::default()
    })
    .await
}

#[test]
fn increments_the_turn_index_across_turns() {
    on_task_set(|_| async {
        let turns = Rc::new(RefCell::new(Vec::new()));
        let (starts, ends) = (Rc::clone(&turns), Rc::clone(&turns));
        let start = handling::<TurnStartEvent, ()>("turn_start", move |event| {
            starts
                .borrow_mut()
                .push(format!("start {}", event.turn_index));
            Ok(None)
        });
        let end = handling::<TurnEndEvent, ()>("turn_end", move |event| {
            ends.borrow_mut().push(format!("end {}", event.turn_index));
            Ok(None)
        });
        let responses = vec![echo_call(), Scripted::text("done"), Scripted::text("again")];
        let h = echo_session(vec![start, end], responses).await;
        prompt(&h, "use the tool").await;
        prompt(&h, "once more").await;
        assert_eq!(
            *turns.borrow(),
            ["start 0", "end 0", "start 1", "end 1", "start 0", "end 0"]
        );
    });
}

#[test]
fn extension_handlers_run_with_the_session_free_to_change() {
    on_task_set(|_| async {
        let slot = Rc::new(RefCell::new(std::rc::Weak::<AgentSession<Local>>::new()));
        let free = Rc::new(RefCell::new(Vec::new()));
        let probe = |event: &'static str| {
            let (session, seen) = (Rc::clone(&slot), Rc::clone(&free));
            move || {
                let session = session.borrow().upgrade().unwrap();
                seen.borrow_mut().push((event, free_to_change(&session)));
            }
        };
        let (on_call, on_turn) = (probe("tool_call"), probe("turn_end"));
        let call = handling::<ToolCallEvent, ToolCallEventResult>("tool_call", move |_| {
            on_call();
            Ok(None)
        });
        let turn = handling::<TurnEndEvent, ()>("turn_end", move |_| {
            on_turn();
            Ok(None)
        });
        let h = echo_session(vec![call, turn], vec![echo_call(), Scripted::text("done")]).await;
        *slot.borrow_mut() = Rc::downgrade(&h.session);
        prompt(&h, "use the tool").await;
        assert_eq!(
            *free.borrow(),
            [("tool_call", true), ("turn_end", true), ("turn_end", true)]
        );
    });
}

#[test]
fn keeps_the_turn_index_when_the_turn_end_emission_fails() {
    on_task_set(|_| async {
        let starts = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&starts);
        let start = handling::<TurnStartEvent, ()>("turn_start", move |event| {
            seen.borrow_mut().push(event.turn_index);
            Ok(None)
        });
        let failing = handling::<TurnEndEvent, ()>("turn_end", |_| Err("turn end failed"));
        let responses = vec![echo_call(), Scripted::text("done")];
        let h = echo_session(vec![start, failing], responses).await;
        let _unsubscribe = h
            .runner()
            .on_error(Rc::new(|_| Err("error listener failed".into())));
        prompt(&h, "use the tool").await;
        assert_eq!(*starts.borrow(), [0.0, 0.0]);
    });
}

#[test]
fn keeps_the_retry_attempt_while_successful_retry_listeners_run() {
    on_task_set(|_| async {
        let failure = Scripted {
            error: Some("503 service unavailable".into()),
            ..Scripted::default()
        };
        let h = harness(HarnessOptions {
            responses: Some(vec![failure, Scripted::text("recovered")]),
            settings: json!({"retry": {"enabled": true, "maxRetries": 1, "baseDelayMs": 1}}),
            ..HarnessOptions::default()
        })
        .await;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (session, observed) = (Rc::downgrade(&h.session), Rc::clone(&seen));
        drop(h.session.subscribe(Rc::new(move |event| {
            if let AgentSessionEvent::AutoRetryEnd { attempt, .. } = event
                && let Some(session) = session.upgrade()
            {
                observed
                    .borrow_mut()
                    .push((*attempt, session.retry_attempt()));
            }
        })));
        prompt(&h, "question").await;
        assert_eq!(*seen.borrow(), [(1, 1)]);
        assert_eq!(h.session.retry_attempt(), 0);
    });
}

#[test]
fn reports_extension_handler_failures_without_stopping_the_queue() {
    on_task_set(|_| async {
        let failing = handling::<TurnStartEvent, ()>("turn_start", |_| Err("turn handler failed"));
        let h = session(vec![failing], vec![Scripted::text("answer")]).await;
        let errors = Rc::new(RefCell::new(Vec::new()));
        let reported = Rc::clone(&errors);
        let _unsubscribe = h.runner().on_error(Rc::new(move |error| {
            reported
                .borrow_mut()
                .push(format!("{} {}", error.event, error.error));
            Ok(())
        }));
        let log = Rc::new(RefCell::new(Vec::new()));
        log_events(&h, "listener", &log);
        prompt(&h, "question").await;
        assert_eq!(*errors.borrow(), ["turn_start turn handler failed"]);
        assert_eq!(log.borrow().last().unwrap(), "listener:agent_end");
        assert!(
            log.borrow()
                .contains(&"listener:message_end answer".to_owned())
        );
    });
}

/// Notifies `ended` once the agent ends, recording whether the session's run was active.
fn observe_run_at_agent_end(h: &Harness, observed: Rc<RefCell<Option<bool>>>, ended: Rc<Notify>) {
    let session = Rc::downgrade(&h.session);
    drop(h.session.agent().subscribe(Rc::new(move |event, _| {
        let ending = matches!(*event.read().unwrap(), AgentEvent::AgentEnd { .. });
        if ending && observed.borrow().is_none() {
            let session = session
                .upgrade()
                .expect("the session outlives its listener");
            *observed.borrow_mut() = Some(session.is_streaming());
            ended.notify_one();
        }
        Box::pin(std::future::ready(Ok(())))
    })));
}

#[test]
fn the_run_is_active_when_the_agent_ends() {
    on_paused_task_set(|_| async {
        for (error, retries) in [("503 service unavailable", 1), ("overloaded_error", 3)] {
            let (reached, release) = (Rc::new(Notify::new()), Rc::new(Notify::new()));
            let log = Rc::new(RefCell::new(Vec::new()));
            let busy = gated::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
                "message_end",
                Rc::clone(&reached),
                Rc::clone(&release),
                Rc::clone(&log),
            );
            let h = harness(HarnessOptions {
                factories: vec![busy],
                responses: Some(vec![Scripted::failure(error), Scripted::text("recovered")]),
                settings: json!({"retry": {"enabled": true, "maxRetries": retries, "baseDelayMs": 1}}),
                ..HarnessOptions::default()
            })
            .await;
            let observed = Rc::new(RefCell::new(None));
            let ended = Rc::new(Notify::new());
            observe_run_at_agent_end(&h, Rc::clone(&observed), Rc::clone(&ended));
            let running = start_prompt(&h, "question");
            reached.notified().await;
            ended.notified().await;
            assert_eq!(
                *observed.borrow(),
                Some(true),
                "the agent end found the run active"
            );
            assert!(
                log.borrow().is_empty(),
                "the queued processing still waited"
            );
            assert!(
                !running.is_finished(),
                "the prompt still waits for the queued handler"
            );
            release.notify_one();
            running.await.unwrap().unwrap();
            assert_eq!(h.requests.0.borrow().len(), 2);
            assert!(!h.session.is_retrying(), "the retry settled");
        }
    });
}

#[test]
fn waits_for_queued_session_events_before_intercepting_a_tool_call() {
    on_task_set(|_| async {
        let (reached, release) = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let log = Rc::new(RefCell::new(Vec::new()));
        let busy = gated::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>(
            "message_end",
            Rc::clone(&reached),
            Rc::clone(&release),
            Rc::clone(&log),
        );
        let seen = Rc::clone(&log);
        let interceptor = handling::<ToolCallEvent, ToolCallEventResult>("tool_call", move |_| {
            seen.borrow_mut().push("tool_call intercepted".into());
            Ok(None)
        });
        let h = echo_session(
            vec![busy, interceptor],
            vec![echo_call(), Scripted::text("done")],
        )
        .await;
        let running = start_prompt(&h, "use the tool");
        reached.notified().await;
        release.notify_one();
        running.await.unwrap().unwrap();
        let order: Vec<String> = log.borrow().iter().take(3).cloned().collect();
        assert_eq!(
            order,
            [
                "message_end handled",
                "message_end handled",
                "tool_call intercepted"
            ]
        );
    });
}

#[test]
fn keeps_content_details_and_error_from_tool_result_interception() {
    on_task_set(|_| async {
        let replacing = handling::<ToolResultEvent, ToolResultEventResult>("tool_result", |_| {
            Ok(Some(ToolResultEventResult {
                content: Presence::Present(vec![UserBlock::Text(TextContent::new(
                    "replaced".into(),
                ))]),
                details: Presence::Present(json!({"replaced": true})),
                is_error: Presence::Present(true),
            }))
        });
        let h = echo_session(vec![replacing], vec![echo_call(), Scripted::text("done")]).await;
        prompt(&h, "use the tool").await;
        let history = h.session.messages();
        let result = history
            .read()
            .unwrap()
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .find(|record| record["role"] == "toolResult")
            .unwrap();
        assert_eq!(
            result["content"],
            json!([{"type": "text", "text": "replaced"}])
        );
        assert_eq!(result["details"], json!({"replaced": true}));
        assert_eq!(result["isError"], json!(true));
        assert!(result.get("terminate").is_none());
    });
}

#[test]
fn persists_application_and_llm_messages_at_message_end() {
    on_task_set(|_| async {
        let h = echo_session(
            Vec::new(),
            vec![echo_call(), Scripted::text("done"), Scripted::text("noted")],
        )
        .await;
        prompt(&h, "use the tool").await;
        h.session
            .send_custom_message(
                maestro_session::messages::CustomMessageInput {
                    custom_type: "note".into(),
                    content: maestro_models::UserContent::Text("remember".into()),
                    display: true,
                    details: Presence::Missing,
                },
                maestro_session::messages::SendMessageOptions {
                    trigger_turn: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        h.session.agent().wait_for_idle().await;
        let manager = h.session.session_manager().borrow();
        let kinds: Vec<Value> = manager
            .get_entries()
            .into_iter()
            .map(|entry| match entry["type"].as_str() {
                Some("message") => entry["message"]["role"].clone(),
                _ => entry["type"].clone(),
            })
            .collect();
        assert_eq!(
            json!(kinds),
            json!([
                "user",
                "assistant",
                "toolResult",
                "assistant",
                "custom_message",
                "assistant"
            ])
        );
    });
}

#[test]
fn compact_aborts_and_disconnects_before_observation() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let (requested, release) = hold_first_answer(&h, "first", &[]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let signal = h.session.agent().signal().unwrap();
        let log = Rc::new(RefCell::new(Vec::new()));
        log_events(&h, "listener", &log);
        let observer = h.session.compact(None);
        let aborted_at_call = signal.is_aborted();
        release.notify_one();
        drop(running.await);
        let forwarded_ends = log
            .borrow()
            .iter()
            .filter(|line| line.as_str() == "listener:agent_end")
            .count();
        drop(observer.await);
        assert_eq!((aborted_at_call, forwarded_ends), (true, 0));
    });
}

#[test]
fn disconnects_and_reconnects_around_a_manual_compaction() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        let (requested, release) = hold_first_answer(&h, "first", &["summary", "after"]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let log = Rc::new(RefCell::new(Vec::new()));
        log_events(&h, "listener", &log);
        let mut compacting = Box::pin(h.session.compact(None));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(std::future::Future::poll(compacting.as_mut(), &mut context).is_pending());
        release.notify_one();
        let compacted = compacting.await;
        drop(running.await);
        assert!(
            log.borrow()
                .iter()
                .all(|line| !line.starts_with("listener:agent_end")),
            "the aborted run's events stayed with the agent: {:?}",
            log.borrow()
        );
        drop(compacted);
        log.borrow_mut().clear();
        prompt(&h, "after compaction").await;
        assert!(log.borrow().contains(&"listener:agent_end".to_owned()));
    });
}

#[test]
fn dispose_invalidates_contexts_and_clears_listeners() {
    on_task_set(|_| async {
        let h = session(Vec::new(), vec![Scripted::text("answer")]).await;
        let context = h.runner().create_command_context();
        let log = Rc::new(RefCell::new(Vec::new()));
        log_events(&h, "listener", &log);
        h.session.dispose();
        let stale = context.cwd().unwrap_err().to_string();
        assert!(stale.starts_with("This extension ctx is stale"), "{stale}");
        h.session
            .agent()
            .prompt(maestro_agent::PromptInput::Text {
                text: "after".into(),
                images: Vec::new(),
            })
            .await
            .unwrap();
        assert_eq!(*log.borrow(), Vec::<String>::new());
    });
}

/// Replaces a completed text response without changing tool-call messages.
fn replacing_text() -> ExtensionFactory<Local> {
    handling::<MessageEndEvent, MessageEndEventResult<NativeAgentMessage>>("message_end", |event| {
        let AgentMessage::Assistant(answer) = &event.message else {
            return Ok(None);
        };
        if answer
            .read()
            .unwrap()
            .content
            .iter()
            .any(|block| matches!(block, maestro_models::AssistantContent::ToolCall(_)))
        {
            return Ok(None);
        }
        let mut rewritten = answer.read().unwrap().clone();
        rewritten.content = vec![maestro_models::AssistantContent::Text(TextContent::new(
            "rewritten".into(),
        ))];
        Ok(Some(MessageEndEventResult {
            message: Presence::Present(AgentMessage::Assistant(std::sync::Arc::new(RwLock::new(
                rewritten,
            )))),
        }))
    })
}

/// One independently recorded extension projection.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ArgumentEventCase {
    /// The projection phase.
    kind: String,
    /// Original call arguments.
    input: Value,
    /// Complete event apart from its generated call identity.
    expected: Value,
}

/// An executable permissive tool recording its arguments and publishing progress.
fn inspecting_tools(
    executed: Rc<RefCell<Vec<Value>>>,
) -> indexmap::IndexMap<String, maestro_agent::SharedAgentTool<Local>> {
    let tool = maestro_agent::AgentToolRecord::<Local> {
        definition: maestro_models::Tool {
            name: "inspect".into(),
            description: "Inspect the supplied arguments".into(),
            parameters: json!({}).into(),
        },
        description: None,
        label: "Inspect".into(),
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(move |_, args, _, update| {
            executed.borrow_mut().push(args);
            if let Some(update) = update {
                update(maestro_agent::AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new("progress".into()))],
                    details: Some(json!({"kept": true})),
                    terminate: None,
                });
            }
            Box::pin(std::future::ready(Ok(maestro_agent::AgentToolResult {
                content: vec![UserBlock::Text(TextContent::new("finished".into()))],
                details: Some(json!({"final": true})),
                terminate: None,
            })))
        }),
    };
    indexmap::IndexMap::from([("inspect".into(), Rc::new(RwLock::new(tool)))])
}

#[test]
fn tool_execution_events_keep_argument_root_values() {
    on_task_set(|_| async {
        let cases: Vec<ArgumentEventCase> =
            serde_json::from_str(include_str!("fixtures/session_tool_argument_roots.json"))
                .unwrap();
        assert_eq!(cases.len(), 24);
        for start in cases.iter().filter(|case| case.kind == "start") {
            let update = cases
                .iter()
                .find(|case| case.kind == "update" && case.input == start.input)
                .unwrap();
            check_argument_events(start, update).await;
        }
    });
}

/// Runs one complete session, retaining ordered extension events and the original call.
async fn check_argument_events(start: &ArgumentEventCase, update: &ArgumentEventCase) {
    use maestro_extensions::{ToolExecutionStartEvent, ToolExecutionUpdateEvent};
    let observed = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&observed);
    let starts = handling::<ToolExecutionStartEvent, ()>("tool_execution_start", move |event| {
        let mut record = serde_json::to_value(event).unwrap();
        record["type"] = json!("tool_execution_start");
        seen.borrow_mut().push(record);
        Ok(None)
    });
    let seen = Rc::clone(&observed);
    let updates = handling::<ToolExecutionUpdateEvent, ()>("tool_execution_update", move |event| {
        let mut record = serde_json::to_value(event).unwrap();
        record["type"] = json!("tool_execution_update");
        seen.borrow_mut().push(record);
        Ok(None)
    });
    let executed = Rc::new(RefCell::new(Vec::new()));
    let h = harness(HarnessOptions {
        factories: vec![starts, updates, replacing_text()],
        responses: Some(vec![
            Scripted {
                tool_calls: vec![("inspect".into(), start.input.clone())],
                ..Scripted::default()
            },
            Scripted::text("original"),
        ]),
        base_tools_override: Some(inspecting_tools(Rc::clone(&executed))),
        ..HarnessOptions::default()
    })
    .await;
    let listeners = Rc::new(RefCell::new(Vec::new()));
    log_events(&h, "listener", &listeners);
    prompt(&h, "inspect the arguments").await;
    h.session.agent().wait_for_idle().await;
    assert_eq!(
        executed.borrow().as_slice(),
        std::slice::from_ref(&start.input)
    );
    let call = retained_argument_call(&h);
    assert_eq!(call.name, "inspect");
    assert_eq!(call.arguments, start.input);
    let mut expected_start = start.expected.clone();
    let mut expected_update = update.expected.clone();
    expected_start["toolCallId"] = json!(call.id);
    expected_update["toolCallId"] = json!(call.id);
    assert_eq!(*observed.borrow(), [expected_start, expected_update]);
    assert!(
        listeners
            .borrow()
            .contains(&"listener:message_end rewritten".into())
    );
    let manager = h.session.session_manager().borrow();
    assert_eq!(
        manager.get_entries().last().unwrap()["message"]["content"][0]["text"],
        "rewritten"
    );
}

/// Reads the original assistant call retained by the completed session.
fn retained_argument_call(h: &Harness) -> maestro_models::ToolCall {
    h.session
        .messages()
        .read()
        .unwrap()
        .iter()
        .find_map(|message| {
            let AgentMessage::Assistant(message) = message else {
                return None;
            };
            message.read().unwrap().content.iter().find_map(|block| {
                if let maestro_models::AssistantContent::ToolCall(call) = block {
                    Some(call.clone())
                } else {
                    None
                }
            })
        })
        .unwrap()
}
