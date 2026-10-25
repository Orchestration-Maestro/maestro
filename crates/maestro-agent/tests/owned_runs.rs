//! Owned prompt admission and completion through the public Agent.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{Agent, PromptInput};
use std::sync::Arc;

/// Inert construction rejects execution without changing queued input or state.
#[test]
fn construction_stays_inert_and_prompt_requires_admission() {
    support::run(async {
        let agent: Arc<Agent> = Arc::new(Agent::default());
        agent.steer(support::user("queued"));
        agent.abort();
        agent.wait_for_idle().await;
        let error = agent
            .prompt(PromptInput::Messages(vec![]))
            .await
            .unwrap_err();
        assert_eq!(
            error.message,
            "Agent requires a task spawner before prompting."
        );
        let state = agent.state().read().unwrap();
        assert!(!state.is_streaming());
        assert!(state.messages().read().unwrap().is_empty());
        assert!(agent.has_queued_messages());
        assert!(agent.signal().is_none());
    });
}
use futures_util::FutureExt;
use maestro_agent::{AgentEvent, AgentMessage};
use maestro_models::StopReason;
use std::sync::Mutex;
use support::owned::{agent, failure, input};
use tokio::sync::Notify;
/// Exercise the public owned-run boundary with controlled settlement.
#[test]
fn prompt_observation_drop_does_not_cancel() {
    support::run(prompt_observation_drop_does_not_cancel_case());
}
/// Execute the controlled public operation.
async fn prompt_observation_drop_does_not_cancel_case() {
    let (agent, tasks) = agent();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let effect = Arc::new(Mutex::new(false));
    let (e, r, f) = (entered.clone(), release.clone(), effect.clone());
    agent.subscribe(Arc::new(move |event, signal| {
        let (e, r, f) = (e.clone(), r.clone(), f.clone());
        Box::pin(async move {
            if matches!(&*event.read().unwrap(), AgentEvent::AgentEnd { .. }) {
                e.notify_one();
                r.notified().await;
                assert!(signal.is_aborted());
                *f.lock().unwrap() = true;
            }
            Ok(())
        })
    }));
    drop(agent.prompt(input("hello")));
    let task = tokio::spawn(tasks.take());
    entered.notified().await;
    assert!(agent.state().read().unwrap().is_streaming());
    assert_unaborted(&agent);
    assert!(agent.wait_for_idle().now_or_never().is_none());
    let busy = agent.prompt(input("busy")).await.unwrap_err();
    assert_eq!(
        busy.message,
        "Agent is already processing a prompt. Use steer() or followUp() to queue messages, or wait for completion."
    );
    let idle = agent.wait_for_idle();
    agent.abort();
    assert!(agent.signal().unwrap().is_aborted());
    assert!(agent.state().read().unwrap().is_streaming());
    release.notify_one();
    idle.await;
    assert!(*effect.lock().unwrap());
    assert!(!agent.state().read().unwrap().is_streaming());
    assert!(agent.signal().is_none());
    agent.wait_for_idle().await;
    agent.wait_for_idle().await;
    task.await.unwrap();
    assert_dropped_run_history(&agent);
}
/// Exercise the public owned-run boundary with controlled settlement.
#[test]
fn failure_listener_rejection_preserves_finally() {
    support::run(failure_listener_rejection_preserves_finally_case());
}
/// Execute the controlled public operation.
async fn failure_listener_rejection_preserves_finally_case() {
    let (agent, tasks) = agent();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(|_, _, _, _| {
        Box::pin(async { Err(failure("original")) })
    }));
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let subscription = agent.subscribe(rejecting_end(entered.clone(), release.clone()));
    let observer = agent.prompt(input("hello"));
    let task = tokio::spawn(tasks.take());
    entered.notified().await;
    let mut idle = agent.wait_for_idle();
    let mut idle2 = agent.wait_for_idle();
    assert!(idle.as_mut().now_or_never().is_none());
    assert!(idle2.as_mut().now_or_never().is_none());
    release.notify_one();
    assert_eq!(observer.await.unwrap_err().message, "listener");
    idle.await;
    idle2.await;
    task.await.unwrap();
    assert_failed_state(&agent);
    assert!(agent.signal().is_none());
    subscription.unsubscribe();
    *agent.stream_fn.write().unwrap() =
        support::responses(vec![support::assistant("recovered")]).stream_fn;
    let next = agent.prompt(input("next"));
    assert!(agent.state().read().unwrap().error_message().is_none());
    tasks.take().await;
    next.await.unwrap();
}
/// Exercise the public owned-run boundary with controlled settlement.
#[test]
fn reset_does_not_abort_or_release_admission() {
    support::run(reset_does_not_abort_or_release_admission_case());
}
/// Execute the controlled public operation.
async fn reset_does_not_abort_or_release_admission_case() {
    let (agent, tasks) = agent();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let (e, r) = (entered.clone(), release.clone());
    agent.subscribe(Arc::new(move |event, _| {
        let (e, r) = (e.clone(), r.clone());
        Box::pin(async move {
            if matches!(&*event.read().unwrap(), AgentEvent::AgentStart) {
                e.notify_one();
                r.notified().await;
            }
            Ok(())
        })
    }));
    agent
        .state()
        .write()
        .unwrap()
        .set_messages(vec![support::user("retained history")]);
    agent.steer(support::user("steering"));
    agent.follow_up(support::user("follow-up"));
    let old = agent.state().read().unwrap().messages().clone();
    let run = agent.prompt(input("hello"));
    let task = tokio::spawn(tasks.take());
    entered.notified().await;
    agent.reset();
    assert!(!agent.has_queued_messages());
    assert!(!agent.signal().unwrap().is_aborted());
    assert!(agent.prompt(input("busy")).await.is_err());
    release.notify_one();
    run.await.unwrap();
    assert_eq!(old.read().unwrap().len(), 1);
    assert_eq!(
        agent
            .state()
            .read()
            .unwrap()
            .messages()
            .read()
            .unwrap()
            .len(),
        2
    );
    task.await.unwrap();
}
/// Reservation precedes caller task admission, including concurrent and reentrant calls.
#[test]
fn admission_is_atomic_before_task_execution() {
    support::run(admission_is_atomic_before_task_execution_case());
}
/// Execute the controlled public operation.
async fn admission_is_atomic_before_task_execution_case() {
    let (agent, tasks) = agent();
    let entered = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    let streaming = Arc::new(std::sync::atomic::AtomicBool::new(false));
    *agent.spawn.write().unwrap() = Some(admission_spawner(
        Arc::downgrade(&agent),
        (Arc::clone(&entered), Arc::clone(&release)),
        (Arc::clone(&tasks), streaming.clone()),
    ));
    let caller = Arc::clone(&agent);
    let first = std::thread::spawn(move || caller.prompt(input("winner")));
    entered.wait();
    let caller = Arc::clone(&agent);
    let second = std::thread::spawn(move || caller.prompt(input("loser")));
    assert_eq!(
        second.join().unwrap().await.unwrap_err().message,
        support::owned::BUSY
    );
    agent.reset();
    assert_eq!(
        agent
            .prompt(input("reset loser"))
            .await
            .unwrap_err()
            .message,
        support::owned::BUSY
    );
    assert!(
        agent
            .state()
            .read()
            .unwrap()
            .messages()
            .read()
            .unwrap()
            .is_empty()
    );
    release.wait();
    let observer = first.join().unwrap();
    let streaming = streaming.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(tasks.0.lock().unwrap().len(), 1);
    tasks.take().await;
    observer.await.unwrap();
    assert!(
        streaming,
        "streaming must be published before task submission"
    );
    assert_winning_history(&agent);
}

/// Pause caller submission while proving reentrant admission is refused.
fn admission_spawner(
    owner: std::sync::Weak<Agent>,
    barriers: (Arc<std::sync::Barrier>, Arc<std::sync::Barrier>),
    queue: (
        Arc<support::owned::Tasks>,
        Arc<std::sync::atomic::AtomicBool>,
    ),
) -> maestro_agent::AgentSpawner {
    Arc::new(move |task| {
        let owner = owner.upgrade().unwrap();
        assert!(owner.state().try_write().is_ok());
        queue.1.store(
            owner.state().read().unwrap().is_streaming(),
            std::sync::atomic::Ordering::SeqCst,
        );
        assert_eq!(
            owner
                .prompt(input("reentrant"))
                .now_or_never()
                .unwrap()
                .unwrap_err()
                .message,
            support::owned::BUSY
        );
        barriers.0.wait();
        barriers.1.wait();
        queue.0.0.lock().unwrap().push(task);
    })
}

/// Text overloads preserve whitespace and image order; supplied batches preserve identities.
#[test]
fn prompt_normalizes_text_images_messages_and_empty_batches() {
    support::run(async {
        for text in ["", "\u{feff}\u{0085}\nhello", " \t hello \n "] {
            for count in [0, 2] {
                normalize_text_case(text, count).await;
            }
        }
        for batch in [
            vec![],
            vec![support::user("zulu")],
            vec![
                support::user("zulu"),
                support::user("alpha"),
                support::user("zulu"),
            ],
        ] {
            normalize_batch_case(batch).await;
        }
    });
}

/// Normal end effects finish before every completion observer.
#[test]
fn end_listener_finishes_before_prompt_and_idle() {
    support::run(listener_barrier(false));
}
/// Assistant completion is published before its awaited message listener.
#[test]
fn message_listener_blocks_idle() {
    support::run(listener_barrier(true));
}
/// Drive the named public listener boundary with a producer-owned gate.
async fn listener_barrier(message_end: bool) {
    let (agent, tasks) = agent();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let effect = Arc::new(Mutex::new(false));
    let (e, r, f) = (entered.clone(), release.clone(), effect.clone());
    let owner = Arc::downgrade(&agent);
    agent.subscribe(Arc::new(move |event, signal| {
        let (e, r, f, owner) = (e.clone(), r.clone(), f.clone(), owner.clone());
        Box::pin(async move {
            let selected = if message_end {
                matches!(
                    &*event.read().unwrap(),
                    AgentEvent::MessageEnd {
                        message: AgentMessage::Assistant(_)
                    }
                )
            } else {
                matches!(&*event.read().unwrap(), AgentEvent::AgentEnd { .. })
            };
            if selected {
                let agent = owner.upgrade().unwrap();
                assert_listener_state(&agent);
                assert!(!signal.is_aborted());
                e.notify_one();
                r.notified().await;
                *f.lock().unwrap() = true;
            }
            Ok(())
        })
    }));
    let mut observation = agent.prompt(input("barrier"));
    let task = tokio::spawn(tasks.take());
    entered.notified().await;
    let mut idle1 = agent.wait_for_idle();
    let mut idle2 = agent.wait_for_idle();
    assert!(observation.as_mut().now_or_never().is_none());
    assert!(idle1.as_mut().now_or_never().is_none());
    assert!(idle2.as_mut().now_or_never().is_none());
    assert_eq!(
        agent.prompt(input("busy")).await.unwrap_err().message,
        support::owned::BUSY
    );
    release.notify_one();
    observation.await.unwrap();
    idle1.await;
    idle2.await;
    task.await.unwrap();
    assert!(*effect.lock().unwrap());
    assert!(!agent.state().read().unwrap().is_streaming());
    assert!(agent.signal().is_none());
}

/// Default conversion filters only custom entries, without sorting or deduplication.
#[test]
fn default_conversion_keeps_standard_roles_in_order() {
    support::run(async {
        use maestro_agent::{AgentInitialState, AgentOptions};
        use maestro_models::ToolResultMessage;
        use std::sync::RwLock;
        let tasks = Arc::new(support::owned::Tasks::default());
        let AgentMessage::User(user) = support::user("zulu") else {
            unreachable!()
        };
        let assistant = support::assistant("alpha");
        let tool: ToolResultMessage = serde_json::from_value(serde_json::json!({
            "role":"toolResult", "toolCallId":"call", "toolName":"echo", "content":[], "isError":false, "timestamp":9
        })).unwrap();
        let entries = vec![
            AgentMessage::User(user.clone()),
            AgentMessage::Custom(Arc::new(RwLock::new(Notice("notice".into())))),
            AgentMessage::Assistant(assistant),
            AgentMessage::ToolResult(Arc::new(RwLock::new(tool))),
            AgentMessage::User(user),
        ];
        let agent: Arc<Agent<Notice>> = Arc::new(Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                messages: entries,
                ..Default::default()
            },
            spawn: Some(tasks.spawner()),
            stream_fn: Some(Arc::new(|_, context, _, _| {
                assert_converted(&context.messages);
                let stream = maestro_models::AssistantMessageEventStream::new();
                stream.end(Some(support::assistant("result")));
                support::ready(Ok(stream))
            })),
            ..Default::default()
        }));
        let run = agent.prompt(PromptInput::Messages(vec![]));
        tasks.take().await;
        run.await.unwrap();
        let state = agent.state().read().unwrap();
        let history = state.messages().read().unwrap();
        assert_eq!(history.len(), 6);
        assert!(
            matches!(&history[1], AgentMessage::Custom(value) if value.read().unwrap().0 == "notice")
        );
    });
}

/// A caller-owned custom message filtered from default model conversion.
struct Notice(String);
impl maestro_agent::CustomAgentMessages for Notice {
    fn role(&self) -> &'static str {
        "notice"
    }
}

/// Registered default model dispatch produces the same public transcript as custom dispatch.
#[test]
fn default_stream_uses_model_dispatch() {
    use maestro_models::{ApiProvider, register_api_provider, unregister_api_providers};
    register_api_provider(
        ApiProvider {
            api: "owned-controlled".into(),
            stream: Arc::new(UnexpectedRaw),
            stream_simple: Arc::new(OwnedDispatch),
        },
        Some("owned-controlled-test".into()),
    );
    support::run(async {
        let (agent, tasks) = agent();
        *agent.stream_fn.write().unwrap() = None;
        let mut model = support::model();
        model.api = "owned-controlled".into();
        agent.state().write().unwrap().model = Arc::new(std::sync::RwLock::new(model));
        let run = agent.prompt(input("dispatch"));
        tasks.take().await;
        run.await.unwrap();
        let state = agent.state().read().unwrap();
        let history = state.messages().read().unwrap();
        assert_eq!(history.len(), 2);
        let AgentMessage::Assistant(result) = &history[1] else {
            panic!("missing response")
        };
        assert!(
            matches!(&result.read().unwrap().content[0], maestro_models::AssistantContent::Text(text) if text.text == "registered")
        );
    });
    unregister_api_providers("owned-controlled-test");
}

/// Native executors own both low-level entry points and the Agent's observation future.
#[test]
fn native_prompt_and_continuation_futures_are_send() {
    support::run(async {
        let (agent, tasks) = agent();
        let observer = tokio::spawn(agent.prompt(input("native")));
        tokio::spawn(tasks.take()).await.unwrap();
        observer.await.unwrap().unwrap();
        let prompt = tokio::spawn(async {
            maestro_agent::run_agent_loop(
                vec![support::user("prompt")],
                support::context(),
                &support::config(),
                Arc::new(|_| support::ready(Ok(()))),
                support::responses(vec![support::assistant("prompt-result")]),
            )
            .await
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(prompt.len(), 2);
        native_tool_progress(false).await;
        let continuation = tokio::spawn(async {
            maestro_agent::run_agent_loop_continue(
                support::context(),
                &support::config(),
                Arc::new(|_| support::ready(Ok(()))),
                support::responses(vec![support::assistant("continued")]),
            )
            .await
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(continuation.len(), 1);
        native_tool_progress(true).await;
        let AgentMessage::Assistant(response) = &continuation[0] else {
            panic!("missing continuation")
        };
        assert!(
            matches!(&response.read().unwrap().content[0], maestro_models::AssistantContent::Text(text) if text.text == "continued")
        );
    });
}

/// Abort reaches a cooperating stream, not a blocked listener or a later independent run.
#[test]
fn abort_is_cooperative_and_signals_are_fresh() {
    support::run(async {
        let (agent, tasks) = agent();
        agent.abort();
        assert!(agent.signal().is_none());
        let entered = Arc::new(Notify::new());
        let e = entered.clone();
        *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, _, options, _| {
            let e = e.clone();
            Box::pin(async move {
                let signal = options.common.signal.unwrap();
                e.notify_one();
                signal.cancelled().await;
                Err(failure("cancelled"))
            })
        }));
        let observer = agent.prompt(input("abort"));
        let first_signal = agent.signal().unwrap();
        let task = tokio::spawn(tasks.take());
        entered.notified().await;
        // Entry steering has already been polled; this input remains unconsumed.
        agent.steer(support::user("queued"));
        agent.abort();
        agent.abort();
        observer.await.unwrap();
        task.await.unwrap();
        assert!(first_signal.is_aborted());
        assert!(agent.has_queued_messages());
        assert!(agent.signal().is_none());
        *agent.stream_fn.write().unwrap() = Some(Arc::new(|_, _, options, _| {
            assert!(!options.common.signal.unwrap().is_aborted());
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.end(Some(support::assistant("fresh")));
            support::ready(Ok(stream))
        }));
        let owner = Arc::downgrade(&agent);
        let late_input = agent.subscribe(Arc::new(move |event, _| {
            if matches!(&*event.read().unwrap(), AgentEvent::AgentEnd { .. }) {
                owner.upgrade().unwrap().steer(support::user("late"));
            }
            support::ready(Ok(()))
        }));
        let next = agent.prompt(input("next"));
        assert!(!agent.signal().unwrap().is_aborted());
        tasks.take().await;
        next.await.unwrap();
        late_input.unsubscribe();
        assert!(first_signal.is_aborted());
        assert!(agent.has_queued_messages());
    });
}

/// Every reached callback failure appends the complete recorded synthetic result.
#[test]
fn failed_callbacks_append_synthetic_message_and_settle() {
    support::run(support::owner_corpus::replay("failures"));
}

/// Recorded starts, ends and all nine updates publish runtime state before listeners.
#[test]
fn reducers_publish_state_before_listeners() {
    support::run(support::owner_corpus::replay("reducers"));
}

/// Pending snapshots retain insertion order across duplicate starts and repeated removals.
#[test]
fn pending_call_sets_preserve_old_snapshots() {
    support::run(async {
        support::owner_corpus::replay("pending").await;
        let (agent, tasks) = agent();
        let response = support::assistant("calls");
        response.write().unwrap().content = ["zulu", "alpha", "zulu"]
            .into_iter()
            .map(|id| {
                maestro_models::AssistantContent::ToolCall(maestro_models::ToolCall {
                    id: id.into(),
                    name: "echo".into(),
                    arguments: maestro_models::JsonObject::new().into(),
                    thought_signature: None,
                })
            })
            .collect();
        *agent.stream_fn.write().unwrap() = support::responses(vec![response]).stream_fn;
        let context =
            support::with_execute(Arc::new(|_, _, _, _| support::ready(Ok(support::result()))));
        agent
            .state()
            .write()
            .unwrap()
            .set_tools(context.tools.unwrap().read().unwrap().clone());
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let retained = snapshots.clone();
        let owner = Arc::downgrade(&agent);
        agent.subscribe(Arc::new(move |event, _| {
            if matches!(
                &*event.read().unwrap(),
                AgentEvent::ToolExecutionStart { .. } | AgentEvent::ToolExecutionEnd { .. }
            ) {
                retained.lock().unwrap().push(
                    owner
                        .upgrade()
                        .unwrap()
                        .state()
                        .read()
                        .unwrap()
                        .pending_tool_calls()
                        .clone(),
                );
            }
            support::ready(Ok(()))
        }));
        let run = agent.prompt(input("pending"));
        tasks.take().await;
        run.await.unwrap();
        assert_pending_snapshots(&agent, &snapshots);
    });
}

/// Inspect one text overload after its submitted run finishes.
async fn normalize_text_case(text: &str, count: usize) {
    let (agent, tasks) = agent();
    let images: Vec<_> = (0..count)
        .map(|i| maestro_models::ImageContent {
            data: format!("image-{i}"),
            mime_type: "image/png".into(),
        })
        .collect();
    let before = maestro_models::timestamp_now();
    let run = agent.prompt(PromptInput::Text {
        text: text.into(),
        images: images.clone(),
    });
    let after = maestro_models::timestamp_now();
    tasks.take().await;
    run.await.unwrap();
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    let AgentMessage::User(user) = &messages[0] else {
        panic!("expected user")
    };
    let user = user.read().unwrap();
    assert!(user.timestamp >= before && user.timestamp <= after);
    let maestro_models::UserContent::Blocks(blocks) = &user.content else {
        panic!("expected blocks")
    };
    assert_eq!(blocks.len(), count + 1);
    assert!(matches!(&blocks[0], maestro_models::UserBlock::Text(value) if value.text == text));
    for (block, image) in blocks[1..].iter().zip(&images) {
        assert!(matches!(block, maestro_models::UserBlock::Image(value) if value == image));
    }
}

/// Inspect supplied-entry identity and authored timestamps.
async fn normalize_batch_case(batch: Vec<AgentMessage>) {
    let (agent, tasks) = agent();
    let run = if batch.len() == 1 {
        agent.prompt(PromptInput::Message(batch[0].clone()))
    } else {
        agent.prompt(PromptInput::Messages(batch.clone()))
    };
    tasks.take().await;
    run.await.unwrap();
    let state = agent.state().read().unwrap();
    let history = state.messages().read().unwrap();
    assert_eq!(history.len(), batch.len() + 1);
    for (actual, supplied) in history.iter().zip(&batch) {
        let (AgentMessage::User(actual), AgentMessage::User(supplied)) = (actual, supplied) else {
            panic!("expected users")
        };
        assert!(Arc::ptr_eq(actual, supplied));
        assert_eq!(
            actual.read().unwrap().timestamp.to_bits(),
            1.0_f64.to_bits()
        );
    }
}

/// State is published before entering the awaited completion listener.
fn assert_listener_state(agent: &Agent) {
    let state = agent.state().read().unwrap();
    assert!(state.streaming_message().is_none());
    assert_eq!(state.messages().read().unwrap().len(), 2);
    assert!(state.is_streaming());
}

/// Compare the ordered standard-role conversion at its public model boundary.
fn assert_converted(messages: &[maestro_models::Message]) {
    assert_eq!(messages.len(), 4);
    assert_converted_user(&messages[0]);
    assert!(
        matches!(&messages[1], maestro_models::Message::Assistant(value) if matches!(&value.content[0], maestro_models::AssistantContent::Text(text) if text.text == "alpha"))
    );
    assert!(
        matches!(&messages[2], maestro_models::Message::ToolResult(value) if value.tool_call_id == "call")
    );
    assert_converted_user(&messages[3]);
}

/// Retained sets cannot be sorted, deduplicated again or mutated in place.
fn assert_pending_snapshots(agent: &Agent, snapshots: &Mutex<Vec<Arc<[String]>>>) {
    let snapshots = snapshots.lock().unwrap();
    let values: Vec<_> = snapshots
        .iter()
        .map(|ids| ids.iter().map(String::as_str).collect::<Vec<_>>())
        .collect();
    assert_eq!(
        values,
        [
            vec!["zulu"],
            vec!["zulu", "alpha"],
            vec!["zulu", "alpha"],
            vec!["alpha"],
            vec![],
            vec![]
        ]
    );
    for pair in snapshots.windows(2) {
        assert!(!Arc::ptr_eq(&pair[0], &pair[1]));
    }
    assert!(
        agent
            .state()
            .read()
            .unwrap()
            .pending_tool_calls()
            .is_empty()
    );
    assert!(!Arc::ptr_eq(
        snapshots.last().unwrap(),
        agent.state().read().unwrap().pending_tool_calls()
    ));
}

/// Both repeated user entries survive standard conversion.
fn assert_converted_user(message: &maestro_models::Message) {
    assert!(
        matches!(message, maestro_models::Message::User(value) if matches!(&value.content, maestro_models::UserContent::Text(text) if text == "zulu"))
    );
}

/// Finally retains original history while clearing runtime observations.
fn assert_failed_state(agent: &Agent) {
    let state = agent.state().read().unwrap();
    assert!(!state.is_streaming());
    assert_eq!(state.error_message(), Some("original"));
    let history = state.messages().read().unwrap();
    let AgentMessage::Assistant(last) = history.last().unwrap() else {
        panic!()
    };
    let last = last.read().unwrap();
    assert_eq!(last.stop_reason, StopReason::Error);
    assert_eq!(last.error_message.as_deref(), Some("original"));
    assert_eq!(last.model, "unknown");
    assert_eq!(
        last.usage.total_tokens.present().unwrap().to_bits(),
        0.0_f64.to_bits()
    );
    assert!(state.streaming_message().is_none());
    assert!(state.pending_tool_calls().is_empty());
}

/// Dropping observation does not signal cancellation.
fn assert_unaborted(agent: &Agent) {
    assert!(!agent.signal().unwrap().is_aborted());
}

/// Fail synthetic end only after its admitted asynchronous effect is released.
fn rejecting_end(entered: Arc<Notify>, release: Arc<Notify>) -> maestro_agent::AgentListener {
    Arc::new(move |event, _| {
        let (entered, release) = (entered.clone(), release.clone());
        Box::pin(async move {
            if matches!(&*event.read().unwrap(), AgentEvent::AgentEnd { .. }) {
                entered.notify_one();
                release.notified().await;
                return Err(failure("listener"));
            }
            Ok(())
        })
    })
}

/// Only the admitted winner reaches history.
fn assert_winning_history(agent: &Agent) {
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    assert_eq!(messages.len(), 2);
    let AgentMessage::User(user) = &messages[0] else {
        panic!("expected winner")
    };
    assert!(
        matches!(&user.read().unwrap().content, maestro_models::UserContent::Blocks(blocks) if matches!(&blocks[0], maestro_models::UserBlock::Text(text) if text.text == "winner"))
    );
}

/// Both spawned entry points await actual tool progress and publish the final artifact.
async fn native_tool_progress(continuation: bool) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = events.clone();
    let messages = tokio::spawn(async move {
        let context = support::with_execute(Arc::new(|_, _, _, update| {
            update.unwrap()(maestro_agent::AgentToolResult {
                content: vec![maestro_models::UserBlock::Text(
                    maestro_models::TextContent::new("native partial".into()),
                )],
                details: Some(serde_json::json!({"phase":"progress"})),
                terminate: None,
            });
            support::ready(Ok(maestro_agent::AgentToolResult {
                content: vec![maestro_models::UserBlock::Text(
                    maestro_models::TextContent::new("native final".into()),
                )],
                details: Some(serde_json::json!({"phase":"final"})),
                terminate: Some(true),
            }))
        }));
        let listener: maestro_agent::AgentEventSink = Arc::new(move |event| {
            let seen = seen.clone();
            Box::pin(async move {
                seen.lock().unwrap().push(event);
                Ok(())
            })
        });
        let options = support::responses(vec![support::tool_call()]);
        let config = support::config();
        if continuation {
            maestro_agent::run_agent_loop_continue(context, &config, listener, options).await
        } else {
            maestro_agent::run_agent_loop(
                vec![support::user("native tool")],
                context,
                &config,
                listener,
                options,
            )
            .await
        }
    })
    .await
    .unwrap()
    .unwrap();
    assert_native_artifacts(&events, &messages);
}

/// The same dropped-observer run retains both its prompt and completed response.
fn assert_dropped_run_history(agent: &Agent) {
    let state = agent.state().read().unwrap();
    let history = state.messages().read().unwrap();
    assert_eq!(history.len(), 2);
    assert!(
        matches!(&history[0], AgentMessage::User(user) if matches!(&user.read().unwrap().content, maestro_models::UserContent::Blocks(blocks) if matches!(&blocks[0], maestro_models::UserBlock::Text(text) if text.text == "hello")))
    );
    assert!(
        matches!(&history[1], AgentMessage::Assistant(response) if matches!(&response.read().unwrap().content[0], maestro_models::AssistantContent::Text(text) if text.text == "done"))
    );
}

/// Joined native execution publishes progress before its final result and history artifact.
fn assert_native_artifacts(events: &Mutex<Vec<AgentEvent>>, messages: &[AgentMessage]) {
    let events = events.lock().unwrap();
    let artifacts: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolExecutionUpdate { partial_result, .. } => {
                Some(("update", partial_result))
            }
            AgentEvent::ToolExecutionEnd {
                result, is_error, ..
            } => {
                assert!(!is_error);
                Some(("end", result))
            }
            _ => None,
        })
        .collect();
    assert_eq!(artifacts.len(), 2);
    assert_eq!(artifacts[0].0, "update");
    assert_eq!(
        artifacts[0].1.details,
        Some(serde_json::json!({"phase":"progress"}))
    );
    assert!(
        matches!(&artifacts[0].1.content[0], maestro_models::UserBlock::Text(text) if text.text == "native partial")
    );
    assert_eq!(artifacts[1].0, "end");
    assert_eq!(
        artifacts[1].1.details,
        Some(serde_json::json!({"phase":"final"}))
    );
    assert!(
        matches!(&artifacts[1].1.content[0], maestro_models::UserBlock::Text(text) if text.text == "native final")
    );
    assert_native_history(messages);
}
/// The completed transcript carries the final artifact, not the partial update.
fn assert_native_history(messages: &[AgentMessage]) {
    assert!(
        matches!(messages.last().unwrap(), AgentMessage::ToolResult(result) if matches!(&result.read().unwrap().content[0], maestro_models::UserBlock::Text(text) if text.text == "native final"))
    );
}

/// Callback rejection retains the complete synthetic assistant record.
#[test]
fn synthetic_failure_defaults_preserve_content() {
    support::run(synthetic_failure_case());
}

/// Exercise both callback error classifications on admitted producers.
async fn synthetic_failure_case() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/assistant-output.json")).unwrap();
    for row in rows.iter().filter(|row| row["kind"] == "synthetic_failure") {
        let (agent, tasks) = agent();
        let mut model = support::model();
        model.api = row["input"]["model"]["api"].as_str().unwrap().into();
        model.provider = row["input"]["model"]["provider"].as_str().unwrap().into();
        model.id = row["input"]["model"]["id"].as_str().unwrap().into();
        agent.state().write().unwrap().model = Arc::new(std::sync::RwLock::new(model));
        let text = row["input"]["message"].as_str().unwrap().to_owned();
        let aborted = row["input"]["aborted"].as_bool().unwrap();
        *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, _, _, signal| {
            let error = failure(&text);
            if aborted {
                signal.common.signal.as_ref().unwrap().abort();
            }
            Box::pin(async move { Err(error) })
        }));
        let before = maestro_models::timestamp_now();
        let observer = agent.prompt(input("hello"));
        tasks.take().await;
        observer.await.unwrap();
        let state = agent.state().read().unwrap();
        let history = state.messages().read().unwrap();
        let AgentMessage::Assistant(output) = history.last().unwrap() else {
            panic!("missing synthetic message")
        };
        let mut actual = output.read().unwrap().clone();
        assert!((before..=maestro_models::timestamp_now()).contains(&actual.timestamp));
        actual.timestamp = row["input"]["timestamp"].as_f64().unwrap();
        let expected: maestro_models::AssistantMessage =
            serde_json::from_value(row["expected"].clone()).unwrap();
        assert_eq!(actual, expected);
    }
}

/// Retained invocation owner for the controlled provider.
struct OwnedDispatch;
impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    > for OwnedDispatch
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(
        &self,
        call: maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    ) -> Self::Return {
        let maestro_models::ProviderCall {
            model,
            context,
            options,
            ..
        } = call;

        assert_eq!(model.id, "fixture");
        assert_eq!(context.messages.len(), 1);
        assert_eq!(
            options.unwrap().common.transport,
            Some(maestro_models::Transport::Auto)
        );
        let stream = maestro_models::AssistantMessageEventStream::new();
        stream.end(Some(support::assistant("registered")));
        Ok(stream)
    }
}

/// Retained invocation owner for the controlled provider.
struct UnexpectedRaw;
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for UnexpectedRaw
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        panic!("raw dispatch selected")
    }
}
