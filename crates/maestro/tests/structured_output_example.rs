//! A terminating structured answer through the built component and native agent.
use maestro_app::{AssistantMessageEvent, AssistantMessageEventStream, PromptOptions};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Literal final answer, including authored whitespace and Unicode.
fn details() -> Value {
    json!({"headline":" Ship 雪 ","summary":"One paragraph.","actionItems":["Review", " Ship "]})
}

/// The actual agent consumes one tool call and does not request a follow-up completion.
#[test]
fn structured_output_ends_the_run_without_another_completion() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = super::support::Root::new();
        let session = super::support::session::component_session(
            &root, local.clone(),
            std::env::var("MAESTRO_STRUCTURED_OUTPUT_COMPONENT").expect("nextest component setup"),
            Some(maestro_app::NoTools::Builtin), super::support::widgets(),
        ).await;
        session.model_registry().auth_storage().set_runtime_api_key("compiled-stream", "controlled");
        let tools = session.extension_runner().get_all_registered_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].definition.name(), "structured_output");
        let requests = Rc::new(Cell::new(0));
        let counted = requests.clone();
        *session.agent().stream_fn.write().unwrap() = Some(Rc::new(move |_, context, _, _| {
            counted.set(counted.get() + 1);
            assert_eq!(counted.get(), 1, "no extra completion request");
            assert_eq!(context.tools.as_ref().unwrap()[0].name, "structured_output");
            let message = Arc::new(RwLock::new(serde_json::from_value(json!({
                "role":"assistant", "content":[{"type":"toolCall","id":"final","name":"structured_output","arguments":details()}],
                "api":"guest-owned-stream","provider":"compiled-stream","model":"registered-case",
                "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                    "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
                "stopReason":"toolUse","timestamp":0
            })).unwrap()));
            let stream = AssistantMessageEventStream::new();
            stream.push(AssistantMessageEvent::Done { reason: serde_json::from_value(json!("toolUse")).unwrap(), message });
            Box::pin(async move { Ok(stream) })
        }));
        session.prompt("Return a structured answer", PromptOptions::default()).await.unwrap();
        assert_eq!(requests.get(), 1);
        let state = session.agent().state().read().unwrap();
        let messages: Vec<Value> = state.messages().read().unwrap().iter().map(|message| serde_json::to_value(message).unwrap()).collect();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2]["role"], "toolResult");
        assert_eq!(messages[2]["content"], json!([{"type":"text","text":"Structured answer ready:  Ship 雪 "}]));
        assert_eq!(messages[2]["details"], details());
        assert_eq!(messages[2]["isError"], false);
        drop(state);
        session.dispose();
    });
}
