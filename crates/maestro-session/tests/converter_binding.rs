//! Constructor-installed projection at the model boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use maestro_agent::{Agent, AgentMessage, AgentOptions, PromptInput};
use maestro_models::{AssistantMessageEventStream, BoxFuture, UserContent};
use maestro_request::presence::Presence;
use maestro_session::messages::{BashExecutionMessage, CustomAgentMessages, create_custom_message};
use std::sync::{Arc, Mutex, RwLock};

/// An unrecognized caller role.
struct Unknown;
impl maestro_agent::CustomAgentMessages for Unknown {
    fn role(&self) -> &'static str {
        "unrecognized"
    }
}

#[test]
fn configured_converter_reaches_model_request() {
    let task = Arc::new(Mutex::new(None::<BoxFuture<()>>));
    let submitted = Arc::clone(&task);
    let observed = Arc::new(Mutex::new(None));
    let delivered = Arc::clone(&observed);
    let agent = Arc::new(Agent::new(AgentOptions::<
        CustomAgentMessages<serde_json::Value, Unknown>,
    > {
        convert_to_llm: Some(Arc::new(|history| {
            let messages = maestro_session::convert_to_llm(&history.read().unwrap());
            Box::pin(async { Ok(messages) })
        })),
        spawn: Some(Arc::new(move |future| {
            *submitted.lock().unwrap() = Some(future);
        })),
        stream_fn: Some(Arc::new(move |_, context, _, _| {
            *delivered.lock().unwrap() = Some(context.messages);
            Box::pin(async {
                let stream = AssistantMessageEventStream::new();
                let response = serde_json::from_value(serde_json::json!({
                    "role":"assistant","content":[],"api":"fixture","provider":"fixture","model":"fixture",
                    "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
                    "stopReason":"stop","timestamp":1
                })).unwrap();
                stream.end(Some(Arc::new(RwLock::new(response))));
                Ok(stream)
            })
        })),
        ..Default::default()
    }));
    let entries = entries();
    let completion = agent.prompt(PromptInput::Messages(entries));
    let future = task.lock().unwrap().take().unwrap();
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            future.await;
            completion.await.unwrap();
        });
    let messages = observed
        .lock()
        .unwrap()
        .take()
        .expect("stream callback must run");
    assert_eq!(
        serde_json::to_value(messages).unwrap(),
        serde_json::json!([
            {"role":"user","content":[{"type":"text","text":"hidden but included"}],"timestamp":0.0}
        ])
    );
}

/// Distinguishable admitted and omitted custom input.
fn entries() -> Vec<AgentMessage<CustomAgentMessages<serde_json::Value, Unknown>>> {
    vec![
        CustomAgentMessages::Custom(create_custom_message(
            "notice".into(),
            UserContent::Text("hidden but included".into()),
            false,
            Presence::Missing,
            "1970-01-01",
        )),
        CustomAgentMessages::BashExecution(BashExecutionMessage {
            command: "ignored".into(),
            output: "ignored".into(),
            cancelled: false,
            truncated: false,
            exit_code: Presence::Missing,
            full_output_path: Presence::Missing,
            timestamp: 0.0,
            exclude_from_context: Presence::Present(true),
        }),
        CustomAgentMessages::Other(Unknown),
    ]
    .into_iter()
    .map(|message| AgentMessage::Custom(Arc::new(RwLock::new(message))))
    .collect()
}
