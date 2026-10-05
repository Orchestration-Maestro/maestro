mod support;
use maestro_agent::*;
use maestro_models::*;
use support::*;

#[tokio::test]
async fn queue_modes_default_independently_and_preserve_fifo() {
    for steering in [QueueMode::OneAtATime, QueueMode::All] {
        for follow_up in [QueueMode::OneAtATime, QueueMode::All] {
            let (agent, provider) = setup(
                (0..5).map(|_| Script::Steps(steps("answer"))).collect(),
                AgentOptions::default(),
            );
            assert_eq!(agent.queue_mode(Queue::Steering), QueueMode::OneAtATime);
            assert_eq!(agent.queue_mode(Queue::FollowUp), QueueMode::OneAtATime);
            agent.set_queue_mode(Queue::Steering, steering);
            assert_eq!(agent.queue_mode(Queue::FollowUp), QueueMode::OneAtATime);
            agent.set_queue_mode(Queue::FollowUp, follow_up);
            agent.steer(user("s1"));
            agent.steer(user("s2"));
            agent.follow_up(user("f1"));
            agent.follow_up(user("f2"));
            let result = agent
                .prompt(user("initial"), options())
                .unwrap()
                .await
                .unwrap();
            let inputs: Vec<_> = result
                .iter()
                .filter(|m| matches!(m, AgentMessage::Model(Message::User(_))))
                .cloned()
                .collect();
            assert_eq!(
                inputs,
                vec![
                    user("initial"),
                    user("s1"),
                    user("s2"),
                    user("f1"),
                    user("f2")
                ]
            );
            let calls = provider.calls();
            assert_eq!(
                calls.len(),
                (if steering == QueueMode::All { 1 } else { 2 })
                    + (if follow_up == QueueMode::All { 1 } else { 2 })
            );
            assert_eq!(
                calls[0].context.messages.len(),
                if steering == QueueMode::All { 3 } else { 2 }
            );
            assert!(agent.queue(Queue::Steering).is_empty());
            assert!(agent.queue(Queue::FollowUp).is_empty());
        }
    }
}

#[test]
fn queue_inspection_and_clear_are_detached_and_ordered() {
    let (agent, provider) = setup(vec![], AgentOptions::default());
    agent.steer(user("s1"));
    agent.steer(user("s2"));
    agent.follow_up(user("f1"));
    assert!(agent.state().context.messages.is_empty());
    assert!(!agent.state().is_running);
    assert!(provider.calls().is_empty());
    let mut copy = agent.queue(Queue::Steering);
    copy.clear();
    assert_eq!(agent.queue(Queue::Steering), vec![user("s1"), user("s2")]);
    assert_eq!(
        agent.clear_queue(Queue::Steering),
        vec![user("s1"), user("s2")]
    );
    assert!(agent.clear_queue(Queue::Steering).is_empty());
    assert_eq!(agent.queue(Queue::FollowUp), vec![user("f1")]);
    assert_eq!(agent.clear_queue(Queue::FollowUp), vec![user("f1")]);
    assert!(agent.clear_queue(Queue::FollowUp).is_empty());
}

#[tokio::test]
async fn steering_is_polled_at_entry_and_after_complete_turn() {
    use std::sync::{Arc, Mutex};
    let (wait, entered, release) = gate();
    let mut response = steps("first");
    response.insert(2, wait);
    let (agent, provider) = setup(
        vec![Script::Steps(response), Script::Steps(steps("second"))],
        AgentOptions::default(),
    );
    agent.steer(user("entry"));
    let (end_wait, end_entered, end_release) = gate();
    let held = Arc::new(Mutex::new(Some(end_wait)));
    agent.subscribe(Arc::new(move |event, _| {
        let wait = if matches!(event, AgentEvent::TurnEnd { .. }) {
            held.lock().unwrap().take()
        } else {
            None
        };
        Box::pin(async move {
            if let Some(ScriptStep::Wait(wait)) = wait {
                wait.await;
            }
        })
    }));
    let handle = agent.prompt(user("initial"), options()).unwrap();
    entered.await.unwrap();
    agent.steer(user("boundary"));
    assert_eq!(provider.calls()[0].context.messages.len(), 2);
    assert_eq!(agent.queue(Queue::Steering), vec![user("boundary")]);
    release.send(()).unwrap();
    end_entered.await.unwrap();
    assert_eq!(provider.calls().len(), 1);
    assert_eq!(agent.queue(Queue::Steering), vec![user("boundary")]);
    end_release.send(()).unwrap();
    handle.await.unwrap();
    assert_eq!(provider.calls().len(), 2);
    assert_eq!(provider.calls()[1].context.messages.len(), 4);
}

#[tokio::test]
async fn follow_up_waits_until_steering_is_exhausted() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (agent, provider) = setup(
        (0..4).map(|_| Script::Steps(steps("reply"))).collect(),
        AgentOptions::default(),
    );
    agent.follow_up(user("later"));
    agent.steer(user("entry"));
    let owner = agent.clone();
    let once = AtomicBool::new(false);
    agent.subscribe(Arc::new(move |event, _| {
        if matches!(event, AgentEvent::TurnEnd { .. }) && !once.swap(true, Ordering::SeqCst) {
            owner.steer(user("injected"));
        }
        Box::pin(async {})
    }));
    let result = agent
        .prompt(user("initial"), options())
        .unwrap()
        .await
        .unwrap();
    assert_eq!(provider.calls().len(), 3);
    let inputs: Vec<_> = result
        .into_iter()
        .filter(|m| matches!(m, AgentMessage::Model(Message::User(_))))
        .collect();
    assert_eq!(
        inputs,
        vec![
            user("initial"),
            user("entry"),
            user("injected"),
            user("later")
        ]
    );
    assert_eq!(provider.calls()[0].context.messages.len(), 2);
    assert_eq!(provider.calls()[1].context.messages.len(), 4);
    assert_eq!(provider.calls()[2].context.messages.len(), 6);
}

#[tokio::test]
async fn assistant_tail_restart_prioritizes_steering_without_double_drain() {
    for mode in [QueueMode::OneAtATime, QueueMode::All] {
        for steering in [true, false] {
            let (agent, provider) = setup(
                (0..5).map(|_| Script::Steps(steps("reply"))).collect(),
                AgentOptions::default(),
            );
            agent.prompt(user("old"), options()).unwrap().await.unwrap();
            agent.set_queue_mode(Queue::Steering, mode);
            agent.set_queue_mode(Queue::FollowUp, mode);
            if steering {
                agent.steer(user("s1"));
                agent.steer(user("s2"));
            }
            agent.follow_up(user("f1"));
            agent.follow_up(user("f2"));
            let result = agent.continue_run(options()).unwrap().await.unwrap();
            assert_eq!(result[0], if steering { user("s1") } else { user("f1") });
            let first = &provider.calls()[1].context.messages;
            assert_eq!(first.len(), if mode == QueueMode::All { 4 } else { 3 });
            let inputs: Vec<_> = result
                .into_iter()
                .filter(|m| matches!(m, AgentMessage::Model(Message::User(_))))
                .collect();
            assert_eq!(
                inputs,
                if steering {
                    vec![user("s1"), user("s2"), user("f1"), user("f2")]
                } else {
                    vec![user("f1"), user("f2")]
                }
            );
        }
    }
}

#[tokio::test]
async fn stop_after_turn_true_leaves_both_queues_untouched() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let finished = Arc::new(AtomicBool::new(false));
    let observed = finished.clone();
    let config = AgentOptions {
        stop_after_turn: Some(Arc::new(move |context| {
            assert!(observed.load(Ordering::SeqCst));
            assert_eq!(context.new_messages, context.context.messages);
            assert_eq!(context.new_messages.len(), 2);
            assert_eq!(
                context.new_messages[1],
                AgentMessage::Model(Message::Assistant(context.message.clone()))
            );
            assert!(context.tool_results.is_empty());
            Box::pin(async { true })
        })),
        ..Default::default()
    };
    let (agent, provider) = setup(vec![Script::Steps(steps("done"))], config);
    let events = capture(&agent);
    let (wait, entered, release) = gate();
    let wait = Mutex::new(Some(wait));
    let owner = agent.clone();
    agent.subscribe(Arc::new(move |event, _| {
        let accepted = if matches!(event, AgentEvent::TurnEnd { .. }) {
            owner.steer(user("s"));
            owner.follow_up(user("f"));
            wait.lock().unwrap().take()
        } else {
            None
        };
        let finished = finished.clone();
        Box::pin(async move {
            if let Some(ScriptStep::Wait(wait)) = accepted {
                wait.await;
                finished.store(true, Ordering::SeqCst);
            }
        })
    }));
    let handle = agent.prompt(user("input"), options()).unwrap();
    entered.await.unwrap();
    assert_eq!(provider.calls().len(), 1);
    release.send(()).unwrap();
    handle.await.unwrap();
    assert_eq!(agent.queue(Queue::Steering), vec![user("s")]);
    assert_eq!(agent.queue(Queue::FollowUp), vec![user("f")]);
    assert_eq!(provider.calls().len(), 1);
    let events = events.lock().unwrap();
    assert_eq!(
        events.iter().rev().take(2).map(label).collect::<Vec<_>>(),
        vec!["agent_end", "turn_end"]
    );
}

#[tokio::test]
async fn stop_after_turn_false_does_not_invent_continuation() {
    use std::sync::Arc;
    for hook in [None, Some(Arc::new(|_: StopAfterTurnContext| -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>> { Box::pin(async { false }) }) as StopAfterTurn)] {
        for queued in [false, true] {
            let config = AgentOptions { stop_after_turn: hook.clone(), ..Default::default() };
            let (agent, provider) = setup((0..3).map(|_| Script::Steps(steps("reply"))).collect(), config);
            if queued { agent.follow_up(user("later")); }
            let result = agent.prompt(user("initial"), options()).unwrap().await.unwrap();
            assert_eq!(provider.calls().len(), if queued {2} else {1});
            assert_eq!(result.len(), if queued {4} else {2});
        }
    }
}
