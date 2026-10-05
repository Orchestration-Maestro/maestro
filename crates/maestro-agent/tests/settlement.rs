mod support;
use maestro_agent::*;
use maestro_models::*;
use std::{
    future::Future,
    sync::{Arc, Mutex},
};
use support::*;

#[tokio::test]
async fn subscribers_are_awaited_in_registration_order() {
    let (agent, _) = setup(vec![Script::Steps(steps("reply"))], AgentOptions::default());
    let (wait, entered, release) = gate();
    let held = Mutex::new(Some(wait));
    let order = Arc::new(Mutex::new(vec![]));
    let first = order.clone();
    let owner = agent.clone();
    agent.subscribe(Arc::new(move |event, _| {
        assert!(owner.state().is_running);
        if let AgentEvent::MessageEnd { message } = &event {
            assert_eq!(owner.state().context.messages.last(), Some(message));
        }
        let wait = held.lock().unwrap().take();
        let first = first.clone();
        Box::pin(async move {
            first.lock().unwrap().push((label(&event), 1));
            if let Some(ScriptStep::Wait(wait)) = wait {
                wait.await;
            }
        })
    }));
    let second = order.clone();
    let owner = agent.clone();
    agent.subscribe(Arc::new(move |event, _| {
        assert!(owner.state().is_running);
        if matches!(event, AgentEvent::AgentEnd { .. }) {
            owner.follow_up(user("accepted"));
        }
        second.lock().unwrap().push((label(&event), 2));
        Box::pin(async {})
    }));
    let handle = agent.prompt(user("input"), options()).unwrap();
    entered.await.unwrap();
    assert_eq!(*order.lock().unwrap(), vec![("agent_start", 1)]);
    release.send(()).unwrap();
    handle.await.unwrap();
    let delivered = order.lock().unwrap();
    let (pairs, remainder) = delivered.as_chunks::<2>();
    assert!(remainder.is_empty());
    for pair in pairs {
        assert_eq!(pair[0].0, pair[1].0);
        assert_eq!((pair[0].1, pair[1].1), (1, 2));
    }
    assert_eq!(agent.queue(Queue::FollowUp), vec![user("accepted")]);
}

#[tokio::test]
async fn agent_end_handlers_and_accepted_work_precede_idle_and_completion() {
    let (agent, _) = setup(vec![Script::Steps(steps("reply"))], AgentOptions::default());
    let (handler, entered, release) = gate();
    let (child, child_entered, child_release) = gate();
    let accepted = Mutex::new(Some((handler, child)));
    agent.subscribe(Arc::new(move |event, _| {
        let work = if matches!(event, AgentEvent::AgentEnd { .. }) {
            accepted.lock().unwrap().take()
        } else {
            None
        };
        Box::pin(async move {
            if let Some((ScriptStep::Wait(handler), ScriptStep::Wait(child))) = work {
                handler.await;
                tokio::spawn(child).await.unwrap();
            }
        })
    }));
    let mut handle = agent.prompt(user("input"), options()).unwrap();
    entered.await.unwrap();
    let mut idle1 = Box::pin(agent.wait_for_idle());
    let mut idle2 = Box::pin(agent.wait_for_idle());
    pending(&mut handle).await;
    pending(&mut idle1).await;
    pending(&mut idle2).await;
    assert!(agent.state().is_running);
    assert!(matches!(
        agent.continue_run(options()),
        Err(AgentError::Busy)
    ));
    release.send(()).unwrap();
    child_entered.await.unwrap();
    pending(&mut handle).await;
    pending(&mut idle1).await;
    pending(&mut idle2).await;
    assert!(agent.state().is_running);
    child_release.send(()).unwrap();
    assert_eq!(handle.await.unwrap().len(), 2);
    idle1.await.unwrap();
    idle2.await.unwrap();
    assert!(!agent.state().is_running);
    agent.wait_for_idle().await.unwrap();
}

#[tokio::test]
async fn abort_signals_current_work_without_clearing_queues() {
    let seen = Arc::new(Mutex::new(None));
    let transformed = seen.clone();
    let config = AgentOptions {
        transform_context: Some(Arc::new(move |records, signal| {
            *transformed.lock().unwrap() = Some(signal);
            Box::pin(async move { records })
        })),
        ..Default::default()
    };
    let (wait, entered, _release) = gate();
    let mut response = steps("partial");
    response.insert(2, wait);
    let (agent, provider) = setup(
        vec![Script::Steps(response), Script::Steps(steps("fresh"))],
        config,
    );
    let events = capture(&agent);
    let signals = Arc::new(Mutex::new(vec![]));
    let observed = signals.clone();
    agent.subscribe(Arc::new(move |_, signal| {
        observed.lock().unwrap().push(signal);
        Box::pin(async {})
    }));
    agent.abort();
    let handle = agent.prompt(user("input"), options()).unwrap();
    entered.await.unwrap();
    agent.steer(user("s"));
    agent.follow_up(user("f"));
    agent.abort();
    let result = handle.await.unwrap();
    assert!(seen.lock().unwrap().as_ref().unwrap().is_cancelled());
    assert!(provider.calls()[0].options.cancellation.is_cancelled());
    assert!(
        signals
            .lock()
            .unwrap()
            .iter()
            .all(Cancellation::is_cancelled)
    );
    let AgentMessage::Model(Message::Assistant(terminal)) = &result[1] else {
        panic!()
    };
    assert_eq!(terminal.stop_reason, Some(StopReason::Aborted));
    assert_eq!(
        terminal.content,
        vec![AssistantContent::Text(TextContent {
            text: "partial".into(),
            replay_metadata: None
        })]
    );
    assert_eq!(agent.queue(Queue::Steering), vec![user("s")]);
    assert_eq!(agent.queue(Queue::FollowUp), vec![user("f")]);
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, AgentEvent::TurnEnd { .. }))
            .count(),
        1
    );
    agent.abort();
    agent.clear_queue(Queue::Steering);
    agent.clear_queue(Queue::FollowUp);
    agent
        .prompt(user("again"), options())
        .unwrap()
        .await
        .unwrap();
    assert!(!provider.calls()[1].options.cancellation.is_cancelled());
}

#[tokio::test]
async fn abort_waits_for_accepted_subscribers() {
    let (agent, provider) = setup(vec![Script::Steps(steps("reply"))], AgentOptions::default());
    let (wait, entered, release) = gate();
    let work = Mutex::new(Some(wait));
    let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel();
    let cancelled_tx = Mutex::new(Some(cancelled_tx));
    agent.subscribe(Arc::new(move |event, signal| {
        let accepted = if matches!(
            event,
            AgentEvent::MessageStart {
                message: AgentMessage::Model(Message::Assistant(_))
            }
        ) {
            work.lock().unwrap().take()
        } else {
            None
        };
        let notify = if accepted.is_some() {
            cancelled_tx.lock().unwrap().take()
        } else {
            None
        };
        Box::pin(async move {
            if let Some(ScriptStep::Wait(cleanup)) = accepted {
                let child = tokio::spawn(cleanup);
                signal.cancelled().await;
                notify.unwrap().send(()).unwrap();
                child.await.unwrap();
            }
        })
    }));
    let events = capture(&agent);
    let mut handle = agent.prompt(user("input"), options()).unwrap();
    entered.await.unwrap();
    let count = events.lock().unwrap().len();
    agent.abort();
    cancelled_rx.await.unwrap();
    let mut idle = Box::pin(agent.wait_for_idle());
    pending(&mut handle).await;
    pending(&mut idle).await;
    assert_eq!(events.lock().unwrap().len(), count);
    assert!(agent.state().is_running);
    assert!(provider.calls()[0].options.cancellation.is_cancelled());
    release.send(()).unwrap();
    handle.await.unwrap();
    idle.await.unwrap();
    assert_eq!(events.lock().unwrap().last().map(label), Some("agent_end"));
}

#[tokio::test]
async fn model_error_and_abort_end_without_retry_or_queue_drain() {
    for (script, failure, content) in [
        (
            Script::SetupFailure(Failure::AdapterFailed),
            Failure::AdapterFailed,
            vec![],
        ),
        (
            Script::Steps(vec![
                ScriptStep::Update(ProviderUpdate::TextStart { content_index: 0 }),
                ScriptStep::Update(ProviderUpdate::TextDelta {
                    content_index: 0,
                    delta: "partial".into(),
                }),
                ScriptStep::Update(ProviderUpdate::Error {
                    failure: Failure::Transport,
                }),
            ]),
            Failure::Transport,
            vec![AssistantContent::Text(TextContent {
                text: "partial".into(),
                replay_metadata: None,
            })],
        ),
        (
            Script::SetupFailure(Failure::Cancelled),
            Failure::Cancelled,
            vec![],
        ),
    ] {
        let config = AgentOptions {
            stop_after_turn: Some(Arc::new(|_| panic!("failure must not invoke stop hook"))),
            ..Default::default()
        };
        let (agent, provider) = setup(vec![script], config);
        let owner = agent.clone();
        agent.subscribe(Arc::new(move |event, _| {
            if matches!(event, AgentEvent::TurnEnd { .. }) {
                owner.steer(user("s"));
                owner.follow_up(user("f"));
            }
            Box::pin(async {})
        }));
        let events = capture(&agent);
        let result = agent
            .prompt(user("input"), options())
            .unwrap()
            .await
            .unwrap();
        let AgentMessage::Model(Message::Assistant(terminal)) = &result[1] else {
            panic!()
        };
        assert_eq!(terminal.failure, Some(failure));
        assert_eq!(terminal.content, content);
        assert_eq!(
            terminal.stop_reason,
            Some(if failure == Failure::Cancelled {
                StopReason::Aborted
            } else {
                StopReason::Error
            })
        );
        assert_eq!(provider.calls().len(), 1);
        assert_eq!(agent.queue(Queue::Steering), vec![user("s")]);
        assert_eq!(agent.queue(Queue::FollowUp), vec![user("f")]);
        let events = events.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, AgentEvent::MessageStart { .. }))
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, AgentEvent::MessageEnd { .. }))
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, AgentEvent::TurnEnd { .. }))
                .count(),
            1
        );
        assert_eq!(
            events.last(),
            Some(&AgentEvent::AgentEnd {
                messages: result.clone()
            })
        );
        assert_eq!(agent.state().context.messages, result);
    }
}

#[tokio::test]
async fn dropping_prompt_handle_does_not_cancel_owned_run() {
    let (wait, entered, release) = gate();
    let mut response = vec![wait];
    response.extend(steps("survived"));
    let (agent, provider) = setup(vec![Script::Steps(response)], AgentOptions::default());
    let events = capture(&agent);
    let handle = agent.prompt(user("input"), options()).unwrap();
    entered.await.unwrap();
    drop(handle);
    let mut idle = Box::pin(agent.wait_for_idle());
    pending(&mut idle).await;
    drop(idle);
    assert!(agent.state().is_running);
    assert!(!provider.calls()[0].options.cancellation.is_cancelled());
    release.send(()).unwrap();
    agent.wait_for_idle().await.unwrap();
    assert_eq!(agent.state().context.messages.len(), 2);
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, AgentEvent::AgentEnd { .. }))
            .count(),
        1
    );
}

#[tokio::test]
async fn unsubscribe_stops_future_acceptance_without_dropping_accepted_delivery() {
    let (agent, _) = setup(vec![Script::Steps(steps("reply"))], AgentOptions::default());
    let (first, entered, release) = gate();
    let held = Mutex::new(Some(first));
    agent.subscribe(Arc::new(move |_, _| {
        let work = held.lock().unwrap().take();
        Box::pin(async move {
            if let Some(ScriptStep::Wait(wait)) = work {
                wait.await;
            }
        })
    }));
    let accepted = Arc::new(Mutex::new(vec![]));
    let observed = accepted.clone();
    let (second, second_entered, second_release) = gate();
    let work = Mutex::new(Some(second));
    let subscription = agent.subscribe(Arc::new(move |event, _| {
        observed.lock().unwrap().push(event);
        let work = work.lock().unwrap().take();
        Box::pin(async move {
            if let Some(ScriptStep::Wait(wait)) = work {
                wait.await;
            }
        })
    }));
    assert!(accepted.lock().unwrap().is_empty());
    let mut handle = agent.prompt(user("input"), options()).unwrap();
    entered.await.unwrap();
    agent.unsubscribe(subscription);
    agent.unsubscribe(subscription);
    release.send(()).unwrap();
    second_entered.await.unwrap();
    pending(&mut handle).await;
    assert!(agent.state().is_running);
    assert_eq!(*accepted.lock().unwrap(), vec![AgentEvent::AgentStart]);
    second_release.send(()).unwrap();
    handle.await.unwrap();
    assert_eq!(*accepted.lock().unwrap(), vec![AgentEvent::AgentStart]);
}

#[test]
fn starting_without_runtime_fails_before_mutation() {
    let (agent, provider) = setup(
        vec![],
        AgentOptions {
            context: AgentContext {
                messages: vec![user("history")],
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let events = capture(&agent);
    agent.steer(user("s"));
    agent.follow_up(user("f"));
    let state = agent.state();
    assert!(matches!(
        agent.prompt(user("input"), options()),
        Err(AgentError::RuntimeUnavailable)
    ));
    assert!(matches!(
        agent.continue_run(options()),
        Err(AgentError::RuntimeUnavailable)
    ));
    assert_eq!(agent.state(), state);
    assert_eq!(agent.queue(Queue::Steering), vec![user("s")]);
    assert_eq!(agent.queue(Queue::FollowUp), vec![user("f")]);
    assert!(provider.calls().is_empty());
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn owned_task_failure_is_not_reported_as_successful_idle() {
    let (agent, _) = setup(vec![Script::Steps(steps("reply"))], AgentOptions::default());
    let (wait, entered, release) = gate();
    let held = Mutex::new(Some(wait));
    agent.subscribe(Arc::new(move |_, _| {
        let work = held.lock().unwrap().take();
        Box::pin(async move {
            if let Some(ScriptStep::Wait(wait)) = work {
                wait.await;
                panic!("intentional callback contract violation");
            }
        })
    }));
    let mut handle = agent.prompt(user("input"), options()).unwrap();
    entered.await.unwrap();
    let mut idle1 = Box::pin(agent.wait_for_idle());
    let mut idle2 = Box::pin(agent.wait_for_idle());
    pending(&mut handle).await;
    pending(&mut idle1).await;
    pending(&mut idle2).await;
    release.send(()).unwrap();
    assert_eq!(handle.await, Err(AgentError::RunFailed));
    assert_eq!(idle1.await, Err(AgentError::RunFailed));
    assert_eq!(idle2.await, Err(AgentError::RunFailed));
    assert!(!agent.state().is_running);
    assert!(agent.state().streaming_message.is_none());
    assert_eq!(agent.wait_for_idle().await, Err(AgentError::RunFailed));
}

#[test]
fn completion_wake_can_read_state() {
    struct StateWake {
        agent: Agent,
        observed: Mutex<Option<tokio::sync::oneshot::Sender<AgentState>>>,
    }
    impl std::task::Wake for StateWake {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            let state = self.agent.state();
            if let Some(observed) = self.observed.lock().unwrap().take() {
                observed.send(state).unwrap();
            }
        }
    }
    let (completed, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let (wait, entered, release) = gate();
            let mut response = vec![wait];
            response.extend(steps("reply"));
            let (agent, _) = setup(vec![Script::Steps(response)], AgentOptions::default());
            let mut handle = agent.prompt(user("input"), options()).unwrap();
            entered.await.unwrap();
            let (observed, observation) = tokio::sync::oneshot::channel();
            let observer = Arc::new(StateWake {
                agent: agent.clone(),
                observed: Mutex::new(Some(observed)),
            });
            let waker = std::task::Waker::from(observer.clone());
            let mut context = std::task::Context::from_waker(&waker);
            assert!(
                std::pin::Pin::new(&mut handle)
                    .poll(&mut context)
                    .is_pending()
            );
            release.send(()).unwrap();
            let observed = observation.await.unwrap();
            let result = handle.await.unwrap();
            completed.send((result, observed, agent.state())).unwrap();
        });
    });
    let (result, observed, settled) = completion
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("completion wake must finish reentrant state observation");
    assert_eq!(result.len(), 2);
    assert!(!observed.is_running);
    assert!(observed.streaming_message.is_none());
    assert_eq!(observed.context.messages, result);
    assert_eq!(observed, settled);
    worker.join().unwrap();
}

#[test]
fn idle_unsubscribe_allows_listener_cleanup_to_read_state() {
    struct Cleanup(Agent);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            assert!(!self.0.state().is_running);
        }
    }
    let (agent, _) = setup(vec![], AgentOptions::default());
    let cleanup = Cleanup(agent.clone());
    let subscription = agent.subscribe(Arc::new(move |_, _| {
        let _ = &cleanup;
        Box::pin(async {})
    }));
    let (completed, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        agent.unsubscribe(subscription);
        completed.send(agent.state()).unwrap();
    });
    let state = completion
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("idle unsubscribe must finish reentrant listener cleanup");
    assert!(!state.is_running);
    assert!(state.context.messages.is_empty());
    worker.join().unwrap();
}
