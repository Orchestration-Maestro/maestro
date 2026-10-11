//! Captured run inputs and shared model identity.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
/// Controlled model and execution fixtures.
pub mod support;
use maestro_models::AssistantMessageEventStream;
use std::sync::{Arc, RwLock};
use tokio::sync::Notify;
/// Credential resolution sees the provider before its await; dispatch sees the later model.
#[test]
fn key_wait_reads_captured_model_again() {
    support::run(key_wait_reads_captured_model_again_case());
}
/// Execute the controlled public operation.
async fn key_wait_reads_captured_model_again_case() {
    let mut config = support::config();
    let captured = config.model.clone();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let (e, r) = (entered.clone(), release.clone());
    config.get_api_key = Some(Arc::new(move |provider| {
        assert_eq!(provider, "fixture");
        let (e, r) = (e.clone(), r.clone());
        Box::pin(async move {
            e.notify_one();
            r.notified().await;
            Ok(Some("key".into()))
        })
    }));
    let options = maestro_agent::AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(|model, _, _, _| {
            assert_eq!(model.id, "changed");
            assert_eq!(model.provider, "changed-provider");
            Box::pin(async {
                let stream = AssistantMessageEventStream::new();
                stream.end(Some(support::assistant("ok")));
                Ok(stream)
            })
        })),
    };
    let run = tokio::spawn(async move {
        maestro_agent::run_agent_loop(
            vec![support::user("hello")],
            support::context(),
            &config,
            Arc::new(|_| Box::pin(async { Ok(()) })),
            options,
        )
        .await
    });
    entered.notified().await;
    captured.write().unwrap().id = "changed".into();
    captured.write().unwrap().provider = "changed-provider".into();
    release.notify_one();
    run.await.unwrap().unwrap();
}
/// Slot replacement does not retarget the request; failures use the live state model.
#[test]
fn captured_model_edits_survive_slot_replacement() {
    support::run(captured_model_edits_survive_slot_replacement_case());
}
/// Execute the controlled public operation.
async fn captured_model_edits_survive_slot_replacement_case() {
    let (agent, tasks) = support::owned::agent();
    let captured = agent.state().read().unwrap().model.clone();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(|model, _, _, _| {
        assert_eq!(model.id, "captured-edited");
        support::ready(Err(support::owned::failure("original\nerror")))
    }));
    let before = maestro_models::timestamp_now();
    let run = agent.prompt(support::owned::input("bind"));
    let mut live = support::model();
    live.id = "failure-model".into();
    agent.state().write().unwrap().model = Arc::new(RwLock::new(live));
    captured.write().unwrap().id = "captured-edited".into();
    tasks.take().await;
    run.await.unwrap();
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    let maestro_agent::AgentMessage::Assistant(failure) = messages.last().unwrap() else {
        panic!("expected failure")
    };
    assert_eq!(failure.read().unwrap().model, "failure-model");
    let timestamp = failure.read().unwrap().timestamp;
    assert!(timestamp >= before && timestamp <= maestro_models::timestamp_now());
    assert_eq!(
        failure.read().unwrap().error_message.as_deref(),
        Some("original\nerror")
    );
}

/// Outer collections and callback selection bind at admission; retained entries stay live.
#[test]
fn prompt_captures_collections_and_stream_at_admission() {
    support::run(async {
        use maestro_agent::ThinkingLevel;
        for (level, expected) in [
            (ThinkingLevel::Off, None),
            (
                ThinkingLevel::Minimal,
                Some(maestro_models::ThinkingLevel::Minimal),
            ),
            (ThinkingLevel::Low, Some(maestro_models::ThinkingLevel::Low)),
            (
                ThinkingLevel::Medium,
                Some(maestro_models::ThinkingLevel::Medium),
            ),
            (
                ThinkingLevel::High,
                Some(maestro_models::ThinkingLevel::High),
            ),
            (
                ThinkingLevel::Xhigh,
                Some(maestro_models::ThinkingLevel::Xhigh),
            ),
        ] {
            captured_collections(level, expected).await;
        }
    });
}

/// Captured entries survive removal of their slots and external aliases, then retire at settlement.
#[test]
fn captured_entries_survive_replacement_until_settlement() {
    support::run(async {
        let (agent, tasks) = support::owned::agent();
        let history = support::user("retained");
        let maestro_agent::AgentMessage::User(value) = &history else {
            unreachable!()
        };
        let weak = Arc::downgrade(value);
        let inventory =
            support::with_execute(Arc::new(|_, _, _, _| support::ready(Ok(support::result()))))
                .tools
                .unwrap();
        let tool = inventory.write().unwrap().pop().unwrap();
        let weak_tool = Arc::downgrade(&tool);
        agent.state().write().unwrap().set_tools(vec![tool.clone()]);
        agent.state().write().unwrap().set_messages(vec![history]);
        *agent.stream_fn.write().unwrap() = Some(Arc::new(|_, context, _, _| {
            assert_eq!(context.tools.as_ref().unwrap()[0].name, "retained edit");
            assert!(
                matches!(&context.messages[0], maestro_models::Message::User(value) if matches!(&value.content, maestro_models::UserContent::Text(text) if text == "retained"))
            );
            let stream = AssistantMessageEventStream::new();
            stream.end(Some(support::assistant("completed")));
            support::ready(Ok(stream))
        }));
        let run = agent.prompt(support::owned::input("new"));
        agent.state().write().unwrap().set_messages(vec![]);
        agent.state().write().unwrap().set_tools(vec![]);
        tool.write().unwrap().definition.name = "retained edit".into();
        assert!(Arc::ptr_eq(&weak_tool.upgrade().unwrap(), &tool));
        drop(tool);
        drop(inventory);
        assert!(weak_tool.upgrade().is_some());
        assert!(weak.upgrade().is_some());
        tasks.take().await;
        run.await.unwrap();
        assert!(weak.upgrade().is_none());
        assert!(weak_tool.upgrade().is_none());
        assert!(agent.signal().is_none());
    });
}

/// Drive one admitted thinking selection before replacing live slots.
async fn captured_collections(
    level: maestro_agent::ThinkingLevel,
    expected: Option<maestro_models::ThinkingLevel>,
) {
    use maestro_agent::ThinkingLevel;
    let (agent, tasks) = support::owned::agent();
    let inventory =
        support::with_execute(Arc::new(|_, _, _, _| support::ready(Ok(support::result()))))
            .tools
            .unwrap();
    let tool = inventory.read().unwrap()[0].clone();
    let history = support::user("old");
    let maestro_agent::AgentMessage::User(entry) = &history else {
        unreachable!()
    };
    {
        let mut state = agent.state().write().unwrap();
        state.system_prompt = "captured instructions".into();
        state.thinking_level = level;
        state.set_messages(vec![history.clone()]);
        state.set_tools(vec![tool.clone()]);
    }
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, context, options, _| {
        assert_eq!(
            context.system_prompt.as_deref(),
            Some("captured instructions")
        );
        assert_eq!(context.messages.len(), 2);
        let tools = context.tools.as_ref().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "edited tool");
        assert!(
            matches!(&context.messages[0], maestro_models::Message::User(value) if matches!(&value.content, maestro_models::UserContent::Text(text) if text == "inner edit"))
        );
        assert_eq!(options.reasoning, expected);
        assert_eq!(
            options.common.transport,
            Some(maestro_models::Transport::Auto)
        );
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(support::assistant("captured stream")));
        support::ready(Ok(stream))
    }));
    let run = agent.prompt(support::owned::input("new"));
    {
        let mut state = agent.state().write().unwrap();
        state.system_prompt = "replacement".into();
        state.thinking_level = ThinkingLevel::Off;
        state.set_messages(vec![support::user("replacement history")]);
        state.set_tools(vec![]);
    }
    tool.write().unwrap().definition.name = "edited tool".into();
    entry.write().unwrap().content = maestro_models::UserContent::Text("inner edit".into());
    install_next_stream(&agent);
    tasks.take().await;
    run.await.unwrap();
    let next = agent.prompt(support::owned::input("next"));
    tasks.take().await;
    next.await.unwrap();
}

/// The subsequent admission observes replacement configuration.
fn install_next_stream(agent: &maestro_agent::Agent) {
    *agent.stream_fn.write().unwrap() = Some(Arc::new(|_, context, options, _| {
        assert_eq!(context.system_prompt.as_deref(), Some("replacement"));
        assert_eq!(options.reasoning, None);
        assert!(context.tools.as_ref().unwrap().is_empty());
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(support::assistant("next stream")));
        support::ready(Ok(stream))
    }));
}
