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
    let config = AgentLoopConfig {
        model: Model {
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
        },
        options: SimpleStreamOptions::default(),
        extra: JsonObject::new(),
        objects: ProviderObjects::default(),
        transform_context: None,
        get_api_key: None,
        convert_to_llm: Arc::new(move |_| {
            let local = Rc::clone(&observed);
            Box::pin(async move {
                drop(local);
                Ok(vec![])
            })
        }),
    };
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
