//! Browser-owned execution accepts local callback and payload captures.
#![cfg(test)]
#![cfg(target_arch = "wasm32")]
use futures_util::FutureExt;
use maestro_agent::{Agent, AgentMessage, AgentOptions, CustomAgentMessages, PromptInput};
use maestro_models::{AssistantMessageEventStream, BoxFuture, StopReason};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// A local message payload.
struct Notice(Rc<()>);
impl CustomAgentMessages for Notice {
    fn role(&self) -> &str {
        "notice"
    }
}
/// Local task ownership remains independent of dropped prompt observation.
#[test]
fn browser_task_admission_accepts_local_captures() {
    let tasks = Rc::new(RefCell::new(Vec::<BoxFuture<()>>::new()));
    let captured = Rc::clone(&tasks);
    let seen = Rc::new(RefCell::new(0));
    let counted = Rc::clone(&seen);
    let payload = Rc::new(());
    let agent: Arc<Agent<Notice>> = Arc::new(Agent::new(AgentOptions {
        spawn: Some(Arc::new(move |task| captured.borrow_mut().push(task))),
        stream_fn: Some(Arc::new(move |_, _, _, _| {
            *counted.borrow_mut() += 1;
            let stream = AssistantMessageEventStream::new();
            let message = serde_json::from_value(serde_json::json!({
                "role":"assistant", "content":[], "api":"local", "provider":"local", "model":"local",
                "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                    "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
                "stopReason":StopReason::Stop, "timestamp":1
            })).unwrap();
            stream.end(Some(Arc::new(RwLock::new(message))));
            Box::pin(std::future::ready(Ok(stream)))
        })),
        ..Default::default()
    }));
    let counted = Rc::clone(&seen);
    agent.subscribe(Arc::new(move |event, _| {
        if let maestro_agent::AgentEvent::MessageEnd {
            message: AgentMessage::Custom(message),
        } = &*event.read().unwrap()
        {
            assert!(Rc::strong_count(&message.read().unwrap().0) > 0);
            *counted.borrow_mut() += 1;
        }
        Box::pin(std::future::ready(Ok(())))
    }));
    drop(
        agent.prompt(PromptInput::Message(AgentMessage::Custom(Arc::new(
            RwLock::new(Notice(payload)),
        )))),
    );
    assert_eq!(tasks.borrow().len(), 1);
    let task = tasks.borrow_mut().pop().unwrap();
    assert!(task.now_or_never().is_some());
    assert_eq!(*seen.borrow(), 2);
    assert!(agent.signal().is_none());
    assert!(agent.wait_for_idle().now_or_never().is_some());
}

/// Continuation configuration retains local captures without native thread bounds.
#[test]
fn browser_continuation_accepts_local_configuration() {
    let local = Rc::new(RefCell::new(0));
    let tasks = Rc::new(RefCell::new(Vec::<BoxFuture<()>>::new()));
    let submitted = tasks.clone();
    let converted = local.clone();
    let transformed = local.clone();
    let keyed = local.clone();
    let payload = local.clone();
    let response = local.clone();
    let before = local.clone();
    let after = local.clone();
    let agent: Arc<Agent<Notice>> = Arc::new(Agent::new(AgentOptions {
        initial_state: maestro_agent::AgentInitialState {
            messages: vec![AgentMessage::Custom(Arc::new(RwLock::new(Notice(
                Rc::new(()),
            ))))],
            ..Default::default()
        },
        spawn: Some(Arc::new(move |task| submitted.borrow_mut().push(task))),
        convert_to_llm: Some(Arc::new(move |_| {
            *converted.borrow_mut() += 1;
            Box::pin(std::future::ready(Ok(vec![])))
        })),
        transform_context: Some(Arc::new(move |history, _| {
            *transformed.borrow_mut() += 1;
            Box::pin(std::future::ready(Ok(history)))
        })),
        get_api_key: Some(Arc::new(move |_| {
            *keyed.borrow_mut() += 1;
            Box::pin(std::future::ready(Ok(None)))
        })),
        on_payload: Some(Arc::new(move |value, _| {
            *payload.borrow_mut() += 1;
            Box::pin(std::future::ready(Ok(value)))
        })),
        on_response: Some(Arc::new(move |_, _| {
            *response.borrow_mut() += 1;
            Box::pin(std::future::ready(Ok(())))
        })),
        before_tool_call: Some(Arc::new(move |_, _| {
            *before.borrow_mut() += 1;
            Box::pin(std::future::ready(Ok(None)))
        })),
        after_tool_call: Some(Arc::new(move |_, _| {
            *after.borrow_mut() += 1;
            Box::pin(std::future::ready(Ok(None)))
        })),
        ..Default::default()
    }));
    let continuing = agent.r#continue();
    assert_eq!(tasks.borrow().len(), 1);
    drop(continuing);
    let task = tasks.borrow_mut().pop().unwrap();
    assert!(task.now_or_never().is_some());
    let prompting = agent.prompt(PromptInput::Messages(vec![]));
    assert_eq!(tasks.borrow().len(), 1);
    drop(prompting);
    let task = tasks.borrow_mut().pop().unwrap();
    assert!(task.now_or_never().is_some());
    assert_eq!(*local.borrow(), 6);
}
