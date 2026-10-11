//! Browser callbacks and custom messages need no native thread bounds.
#![cfg(test)]
#![cfg(target_arch = "wasm32")]
use maestro_agent::{
    AgentContext, AgentEventSink, AgentLoopConfig, AgentLoopOptions, AgentMessage,
    CustomAgentMessages, run_agent_loop,
};
use maestro_models::{JsonObject, Model, ModelCost, ProviderObjects, SimpleStreamOptions};
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};
/// A browser-local caller payload.
struct Notice(Rc<()>);
impl CustomAgentMessages for Notice {
    fn role(&self) -> &'static str {
        "notice"
    }
}
/// Compile the complete browser operation with non-Send captures.
#[test]
fn browser_callbacks_admit_local_payloads() {
    let local = Rc::new(());
    let observed = Rc::clone(&local);
    let config = local_config(observed, &local);
    let emit: AgentEventSink<Notice> = Arc::new(move |event| {
        if let maestro_agent::AgentEvent::MessageStart {
            message: AgentMessage::Custom(value),
        } = event
        {
            assert!(Rc::strong_count(&value.read().unwrap().0) >= 1);
        }
        Box::pin(async { Ok(()) })
    });
    let context = AgentContext {
        system_prompt: String::new(),
        messages: Arc::new(RwLock::new(vec![])),
        tools: None,
    };
    let operation = run_agent_loop(
        vec![AgentMessage::Custom(Arc::new(RwLock::new(Notice(local))))],
        context,
        &config,
        emit,
        AgentLoopOptions::default(),
    );
    drop(operation);
}

/// Construct a local queue carrying the same non-Send custom payload type.
fn local_queue(local: Rc<()>) -> maestro_agent::GetSteeringMessages<Notice> {
    Arc::new(move || {
        let local = Rc::clone(&local);
        Box::pin(async move {
            Ok(vec![AgentMessage::Custom(Arc::new(RwLock::new(Notice(
                local,
            ))))])
        })
    })
}

/// Build a local before tool call callback.
fn local_before_tool_call(local: Rc<()>) -> maestro_agent::BeforeToolCall<Notice> {
    Arc::new(move |c, _| {
        let captured = Rc::clone(&local);
        Box::pin(async move {
            c.args["edited"] = serde_json::json!(Rc::strong_count(&captured));
            Ok(None)
        })
    })
}

/// Build a local after tool call callback.
fn local_after_tool_call(local: Rc<()>) -> maestro_agent::AfterToolCall<Notice> {
    Arc::new(move |c, _| {
        let captured = Rc::clone(&local);
        Box::pin(async move {
            assert!(Rc::strong_count(&captured) >= 1);
            assert!(c.context.messages.try_write().is_ok());
            Ok(None)
        })
    })
}

/// Build a local should stop after turn callback.
fn local_should_stop_after_turn(local: Rc<()>) -> maestro_agent::ShouldStopAfterTurn<Notice> {
    Arc::new(move |c| {
        let captured = Rc::clone(&local);
        Box::pin(async move {
            assert!(Rc::strong_count(&captured) >= 1);
            assert!(c.context.messages.try_write().is_ok());
            Ok(false)
        })
    })
}

/// Construct a complete configuration with local callback captures.
fn local_config(observed: Rc<()>, local: &Rc<()>) -> AgentLoopConfig<Notice> {
    AgentLoopConfig {
        thinking_budgets: None,
        model: Arc::new(RwLock::new(local_model())),
        options: SimpleStreamOptions::default(),
        extra: JsonObject::new(),
        objects: ProviderObjects::default(),
        transform_context: None,
        get_api_key: None,
        before_tool_call: Some(local_before_tool_call(Rc::clone(local))),

        after_tool_call: Some(local_after_tool_call(Rc::clone(local))),

        should_stop_after_turn: Some(local_should_stop_after_turn(Rc::clone(local))),

        get_steering_messages: Some(local_queue(Rc::clone(local))),
        get_follow_up_messages: Some(local_queue(Rc::clone(local))),
        tool_execution: None,
        convert_to_llm: Arc::new(move |_| {
            let local = Rc::clone(&observed);
            Box::pin(async move {
                drop(local);
                Ok(vec![])
            })
        }),
    }
}

/// Supply the browser fixture model descriptor.
fn local_model() -> Model {
    Model {
        id: "local".into(),
        name: "Local".into(),
        api: "local".into(),
        provider: "local".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![],
        cost: ModelCost::default(),
        context_window: 1.0,
        max_tokens: 1.0,
        headers: None,
        compat: None,
    }
}

/// Compile both observable wrappers with local payloads and task submission.
#[test]
fn maestro_browser_raw_wrappers_accept_local_captures() {
    let local = Rc::new(());
    let config = Arc::new(local_config(Rc::clone(&local), &local));
    let tasks = Rc::new(std::cell::RefCell::new(Vec::new()));
    let captured = Rc::clone(&tasks);
    let spawn: maestro_agent::AgentSpawner = Arc::new(move |task| captured.borrow_mut().push(task));
    let context = AgentContext {
        system_prompt: String::new(),
        messages: Arc::new(RwLock::new(vec![AgentMessage::Custom(Arc::new(
            RwLock::new(Notice(local)),
        ))])),
        tools: None,
    };
    let prompt = maestro_agent::agent_loop(
        vec![],
        context.clone(),
        Arc::clone(&config),
        AgentLoopOptions::default(),
        Arc::clone(&spawn),
    );
    let continuation =
        maestro_agent::agent_loop_continue(context, config, AgentLoopOptions::default(), spawn)
            .unwrap();
    assert_eq!(tasks.borrow().len(), 2);
    drop((prompt, continuation));
    tasks.borrow_mut().clear();
}
