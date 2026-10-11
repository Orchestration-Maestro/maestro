//! Live identity-bearing listener membership through public operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
/// Controlled model and execution fixtures.
pub mod support;
use maestro_agent::{AgentEvent, AgentListener, AgentMessage, Subscription};
use std::sync::{Arc, Mutex};
use support::owned::{agent, input};

/// Callback identity deduplicates, while removal handles are explicit and inert on drop.
#[test]
fn subscriptions_are_inert_deduplicated_and_explicitly_removed() {
    support::run(subscriptions_are_inert_deduplicated_and_explicitly_removed_case());
}
/// Execute the controlled public operation.
async fn subscriptions_are_inert_deduplicated_and_explicitly_removed_case() {
    let (agent, tasks) = agent();
    let log = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&log);
    let a: AgentListener = Arc::new(move |event, _| {
        if matches!(&*event.read().unwrap(), AgentEvent::AgentStart) {
            seen.lock().unwrap().push("a");
        }
        support::ready(Ok(()))
    });
    let first = agent.subscribe(Arc::clone(&a));
    let second = agent.subscribe(a);
    let seen = Arc::clone(&log);
    drop(agent.subscribe(Arc::new(move |event, _| {
        if matches!(&*event.read().unwrap(), AgentEvent::AgentStart) {
            seen.lock().unwrap().push("b");
        }
        support::ready(Ok(()))
    })));
    agent.reset();
    assert!(log.lock().unwrap().is_empty());
    let run = agent.prompt(input("one"));
    tasks.take().await;
    run.await.unwrap();
    assert_eq!(*log.lock().unwrap(), ["a", "b"]);
    second.unsubscribe();
    first.unsubscribe();
    second.unsubscribe();
    *agent.stream_fn.write().unwrap() =
        support::responses(vec![support::assistant("again")]).stream_fn;
    let run = agent.prompt(input("two"));
    tasks.take().await;
    run.await.unwrap();
    assert_eq!(*log.lock().unwrap(), ["a", "b", "b"]);
}
/// Dispatch reads live membership after each awaited callback.
#[test]
fn dispatch_observes_live_add_remove_and_readd() {
    support::run(dispatch_observes_live_add_remove_and_readd_case());
}
/// Execute the controlled public operation.
async fn dispatch_observes_live_add_remove_and_readd_case() {
    let (agent, tasks) = agent();
    let log = Arc::new(Mutex::new(Vec::new()));
    let removals = Arc::new(Mutex::new(None::<(Subscription, Subscription)>));
    let self_callback = Arc::new(Mutex::new(None::<AgentListener>));
    let seen = Arc::clone(&log);
    let c: AgentListener = Arc::new(move |event, _| {
        if matches!(&*event.read().unwrap(), AgentEvent::AgentStart) {
            seen.lock().unwrap().push("c");
        }
        support::ready(Ok(()))
    });
    let seen = Arc::clone(&log);
    let remove = Arc::clone(&removals);
    let owner = Arc::downgrade(&agent);
    let again = Arc::clone(&self_callback);
    let a: AgentListener = Arc::new(move |event, _| {
        if matches!(&*event.read().unwrap(), AgentEvent::AgentStart) {
            let first = {
                let mut log = seen.lock().unwrap();
                log.push("a");
                log.len() == 1
            };
            if first {
                let removals = remove.lock().unwrap().take().unwrap();
                removals.0.unsubscribe();
                removals.1.unsubscribe();
                let owner = owner.upgrade().unwrap();
                owner.subscribe(Arc::clone(&c));
                owner.subscribe(again.lock().unwrap().as_ref().unwrap().clone());
            }
        }
        support::ready(Ok(()))
    });
    *self_callback.lock().unwrap() = Some(Arc::clone(&a));
    let old = agent.subscribe(Arc::clone(&a));
    let a_remove = agent.subscribe(a);
    let b = agent.subscribe(Arc::new(|_, _| panic!("removed callback ran")));
    *removals.lock().unwrap() = Some((a_remove, b));
    let run = agent.prompt(input("first"));
    tasks.take().await;
    run.await.unwrap();
    assert_eq!(*log.lock().unwrap(), ["a", "c", "a"]);
    old.unsubscribe();
    *agent.stream_fn.write().unwrap() =
        support::responses(vec![support::assistant("again")]).stream_fn;
    let run = agent.prompt(input("second"));
    tasks.take().await;
    run.await.unwrap();
    assert_eq!(*log.lock().unwrap(), ["a", "c", "a", "c"]);
    self_callback.lock().unwrap().take();
    removal_during_await().await;
}
/// Event field replacement stays live without retargeting already-published slots.
#[test]
fn listeners_share_mutable_event_after_state_reduction() {
    support::run(listeners_share_mutable_event_after_state_reduction_case());
}
/// Execute the controlled public operation.
async fn listeners_share_mutable_event_after_state_reduction_case() {
    let (agent, tasks) = agent();
    let source = support::user("original");
    let AgentMessage::User(original) = &source else {
        panic!("expected user")
    };
    let original = Arc::clone(original);
    agent.subscribe(Arc::new(|event, _| {
        match &mut *event.write().unwrap() {
            AgentEvent::MessageStart { message } | AgentEvent::MessageEnd { message }
                if matches!(message, AgentMessage::User(_)) =>
            {
                *message = support::user("edited event");
            }
            _ => (),
        }
        support::ready(Ok(()))
    }));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&seen);
    let owner = Arc::downgrade(&agent);
    let retained = Arc::clone(&original);
    agent.subscribe(Arc::new(move |event, _| {
        let event = event.read().unwrap();
        let (message, start) = match &*event {
            AgentEvent::MessageStart { message: AgentMessage::User(message) } => (message, true),
            AgentEvent::MessageEnd { message: AgentMessage::User(message) } => (message, false),
            _ => return support::ready(Ok(())),
        };
        assert!(matches!(&message.read().unwrap().content, maestro_models::UserContent::Text(text) if text == "edited event"));
        let owner = owner.upgrade().unwrap();
        let state = owner.state().read().unwrap();
        let stored = if start {
            state.streaming_message().unwrap().clone()
        } else {
            assert!(state.streaming_message().is_none());
            state.messages().read().unwrap().last().unwrap().clone()
        };
        let AgentMessage::User(stored) = stored else { panic!("expected original user") };
        assert!(Arc::ptr_eq(&stored, &retained));
        retained.write().unwrap().content = maestro_models::UserContent::Text("inner edit".into());
        observed.lock().unwrap().push(start);
        support::ready(Ok(()))
    }));
    let run = agent.prompt(maestro_agent::PromptInput::Message(source));
    tasks.take().await;
    run.await.unwrap();
    assert_eq!(*seen.lock().unwrap(), [true, false]);
    assert!(
        matches!(&original.read().unwrap().content, maestro_models::UserContent::Text(text) if text == "inner edit")
    );
}
/// Explicit removal releases strong captures; weak captures introduce no owner cycle.
#[test]
fn listener_captures_release_after_deregistration() {
    support::run(listener_captures_release_after_deregistration_case());
}
/// Execute the controlled public operation.
async fn listener_captures_release_after_deregistration_case() {
    let (agent, tasks) = agent();
    let weak = Arc::downgrade(&agent);
    let captured = Arc::clone(&agent);
    let released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let payload = Arc::new(ReentrantDrop {
        owner: Arc::downgrade(&agent),
        effect: released.clone(),
    });
    let observed_payload = Arc::downgrade(&payload);
    let subscription = agent.subscribe(Arc::new(move |_, _| {
        assert!(captured.state().try_write().is_ok());
        assert!(Arc::strong_count(&payload) > 0);
        support::ready(Ok(()))
    }));
    let run = agent.prompt(input("strong"));
    tasks.take().await;
    run.await.unwrap();
    subscription.unsubscribe();
    subscription.unsubscribe();
    assert!(observed_payload.upgrade().is_none());
    assert!(released.load(std::sync::atomic::Ordering::SeqCst));
    drop(agent);
    assert!(weak.upgrade().is_none());
    let (agent, tasks) = support::owned::agent();
    let weak = Arc::downgrade(&agent);
    let owner = weak.clone();
    agent.subscribe(Arc::new(move |_, _| {
        assert!(owner.upgrade().unwrap().state().try_write().is_ok());
        support::ready(Ok(()))
    }));
    let run = agent.prompt(input("weak"));
    tasks.take().await;
    run.await.unwrap();
    drop(agent);
    assert!(weak.upgrade().is_none());
}

/// Removing an entered listener revokes future delivery, not its awaited effect.
async fn removal_during_await() {
    use tokio::sync::Notify;
    let (agent, tasks) = agent();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let effect = Arc::new(Mutex::new(false));
    let (e, r, f) = (entered.clone(), release.clone(), effect.clone());
    let subscription = agent.subscribe(Arc::new(move |event, _| {
        let (e, r, f) = (e.clone(), r.clone(), f.clone());
        Box::pin(async move {
            if matches!(&*event.read().unwrap(), AgentEvent::AgentStart) {
                e.notify_one();
                r.notified().await;
                *f.lock().unwrap() = true;
            }
            Ok(())
        })
    }));
    let run = agent.prompt(input("entered"));
    let task = tokio::spawn(tasks.take());
    entered.notified().await;
    subscription.unsubscribe();
    assert!(!*effect.lock().unwrap());
    release.notify_one();
    run.await.unwrap();
    task.await.unwrap();
    assert!(*effect.lock().unwrap());
}
/// A callback-owned destructor reenters both state and membership boundaries.
struct ReentrantDrop {
    /// Non-owning registration owner.
    owner: std::sync::Weak<maestro_agent::Agent>,
    /// Witness after unlocked destruction.
    effect: Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for ReentrantDrop {
    fn drop(&mut self) {
        let owner = self.owner.upgrade().unwrap();
        assert!(owner.state().try_write().is_ok());
        owner
            .subscribe(Arc::new(|_, _| support::ready(Ok(()))))
            .unsubscribe();
        self.effect.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
