//! Continuation through the public conversation owner.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{Agent, AgentMessage};
use std::sync::Arc;

#[test]
fn continuation_reports_busy_empty_and_assistant_tail() {
    support::run(async {
        let empty: Arc<Agent> = Arc::default();
        assert_eq!(
            empty.r#continue().await.unwrap_err().message,
            "No messages to continue from"
        );
        empty
            .state()
            .write()
            .unwrap()
            .set_messages(vec![AgentMessage::Assistant(support::assistant("old"))]);
        assert_eq!(
            empty.r#continue().await.unwrap_err().message,
            "Cannot continue from message role: assistant"
        );
        empty.steer(support::user("queued"));
        assert_eq!(
            empty.r#continue().await.unwrap_err().message,
            "Agent requires a task spawner before continuing."
        );
        assert!(empty.has_queued_messages());
        assert!(!empty.state().read().unwrap().is_streaming());
        let (agent, tasks) = support::owned::agent();
        let observation = agent.prompt(support::owned::input("start"));
        agent.reset();
        assert_eq!(
            agent.r#continue().await.unwrap_err().message,
            "Agent is already processing. Wait for completion before continuing."
        );
        tasks.take().await;
        observation.await.unwrap();
        assert!(agent.signal().is_none());
        support::continuation_corpus::replay("C1").await;
        support::binding_corpus::controls("busy_continue").await;
    });
}

#[test]
fn assistant_tail_steers_one_turn_at_a_time() {
    support::run(async {
        let (agent, tasks) = support::owned::agent();
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = requests.clone();
        *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, context, _, _| {
            captured.lock().unwrap().push(context.messages);
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.end(Some(support::assistant("done")));
            support::ready(Ok(stream))
        }));
        agent
            .state()
            .write()
            .unwrap()
            .set_messages(vec![AgentMessage::Assistant(support::assistant("old"))]);
        agent.steer(support::user("first"));
        agent.steer(support::user("second"));
        let observe = agent.r#continue();
        tasks.take().await;
        observe.await.unwrap();
        {
            let requests = requests.lock().unwrap();
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[0].len(), 2);
            assert_eq!(requests[1].len(), 4);
            assert!(!agent.has_queued_messages());
        }
        support::continuation_corpus::replay("C4").await;
    });
}

#[test]
fn continuation_accepts_nonassistant_tails_without_replay() {
    support::run(support::continuation_corpus::replay("C2"));
}

/// Recorded continuation branch with complete observations.
#[test]
fn assistant_tail_prefers_steering_over_followups() {
    support::run(support::continuation_corpus::replay("C3"));
}

/// Recorded continuation branch with complete observations.
#[test]
fn followup_restart_polls_entry_steering() {
    support::run(support::continuation_corpus::replay("C5"));
}

/// Dropped observation leaves the submitted continuation and end listener owned.
#[test]
fn continuation_owns_work_after_observation_drop() {
    support::run(async {
        dropped_continuation().await;
        let (agent, tasks) = support::owned::agent();
        agent
            .state()
            .write()
            .unwrap()
            .set_messages(vec![support::user("old")]);
        let (entered, entry) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        let gates = std::sync::Mutex::new(Some((entered, released)));
        agent.subscribe(Arc::new(move |value, _| {
            let gate = if matches!(
                &*value.read().unwrap(),
                maestro_agent::AgentEvent::AgentEnd { .. }
            ) {
                gates.lock().unwrap().take()
            } else {
                None
            };
            Box::pin(async move {
                wait_end_gate(gate).await;
                Ok(())
            })
        }));
        drop(agent.r#continue());
        let task = tasks.take();
        let joined = tokio::spawn(task);
        entry.await.unwrap();
        assert!(agent.signal().is_some());
        assert_eq!(
            agent.r#continue().await.unwrap_err().message,
            "Agent is already processing. Wait for completion before continuing."
        );
        release.send(()).unwrap();
        joined.await.unwrap();
        agent.wait_for_idle().await;
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
        assert!(agent.signal().is_none());
    });
}

/// Outer replacement does not retarget captured continuation inputs.
#[test]
fn continuation_captures_history_tools_model_and_stream() {
    support::run(async {
        let (agent, tasks) = support::owned::agent();
        let message = support::user("old");
        let tools =
            support::with_execute(Arc::new(|_, _, _, _| support::ready(Ok(support::result()))))
                .tools
                .unwrap();
        let tool = tools.read().unwrap()[0].clone();
        let model = Arc::new(std::sync::RwLock::new(support::model()));
        {
            let mut state = agent.state().write().unwrap();
            state.set_messages(vec![message.clone()]);
            state.set_tools(vec![tool.clone()]);
            state.model = model.clone();
        }
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        capture_continuation(&agent, &seen);
        let observe = agent.r#continue();
        {
            let mut state = agent.state().write().unwrap();
            state.set_messages(vec![support::user("replacement")]);
            state.set_tools(vec![]);
            state.model = Arc::new(std::sync::RwLock::new(support::model()));
        }
        if let AgentMessage::User(value) = &message {
            value.write().unwrap().content = maestro_models::UserContent::Text("edited".into());
        }
        model.write().unwrap().id = "edited-model".into();
        tool.write().unwrap().definition.name = "edited-tool".into();
        *agent.stream_fn.write().unwrap() =
            support::responses(vec![support::assistant("next")]).stream_fn;
        tasks.take().await;
        observe.await.unwrap();
        {
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].0, "edited-model");
            assert!(
                matches!(&seen[0].1[0], maestro_models::Message::User(v) if v.content == maestro_models::UserContent::Text("edited".into()))
            );
            assert_eq!(seen[0].2[0].name, "edited-tool");
        }
        let observe = agent.prompt(support::owned::input("next"));
        tasks.take().await;
        observe.await.unwrap();
        assert_eq!(
            agent
                .state()
                .read()
                .unwrap()
                .messages()
                .read()
                .unwrap()
                .len(),
            4
        );
    });
}

/// Acknowledgment is emitted before the pending listener effect completes.
async fn wait_end_gate(
    gate: Option<(
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Receiver<()>,
    )>,
) {
    if let Some((entered, released)) = gate {
        entered.send(()).unwrap();
        released.await.unwrap();
    }
}

/// Captured request operands from one controlled continuation.
type CapturedInputs = Arc<
    std::sync::Mutex<
        Vec<(
            String,
            Vec<maestro_models::Message>,
            Vec<maestro_models::Tool>,
        )>,
    >,
>;
/// Observe the actual captured model, history and tool descriptors.
fn capture_continuation(agent: &Agent, seen: &CapturedInputs) {
    let captured = seen.clone();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |model, context, _, _| {
        captured
            .lock()
            .unwrap()
            .push((model.id, context.messages, context.tools.unwrap()));
        let stream = maestro_models::AssistantMessageEventStream::new();
        stream.end(Some(support::assistant("done")));
        support::ready(Ok(stream))
    }));
}

/// Dropping an observer before driving its task cannot discard continuation work.
async fn dropped_continuation() {
    let (agent, tasks) = support::owned::agent();
    agent
        .state()
        .write()
        .unwrap()
        .set_messages(vec![support::user("before-drive")]);
    drop(agent.r#continue());
    tasks.take().await;
    assert!(
        agent.signal().is_none(),
        "dropped observer left the owned task unfinished"
    );
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
}
